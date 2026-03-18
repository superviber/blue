//! Dialogue tool handlers
//!
//! Handles dialogue document creation, storage, and extraction.
//! Dialogues capture agent conversations and link them to RFCs.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::alignment_db::{
    self,
    can_dialogue_converge,
    create_round_with_metrics,
    display_id,
    get_claims,
    get_dialogue,
    get_evidence,
    get_experts,
    get_perspectives,
    get_recommendations,
    get_scoreboard,
    get_tensions,
    get_verdicts,
    parse_display_id,
    register_claim,
    register_evidence,
    register_expert,
    register_perspective,
    register_recommendation,
    register_ref,
    register_tension,
    register_verdict,
    update_expert_score,
    update_tension_status,
    validate_display_id,
    validate_ref_semantics,
    ConvergenceMetrics,
    EntityType,
    ExpertSource as DbExpertSource,
    ExpertTier as DbExpertTier,
    RefType,
    // RFC 0057: Convergence discipline
    ScoreComponents,
    ValidationCollector,
    ValidationError,
    Verdict,
    VerdictType,
};
use crate::{title_to_slug, DocType, Document, LinkType, ProjectState};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::handler_error::HandlerError;

/// Coerce a JSON value to bool, accepting both `true` and `"true"`.
/// MCP clients sometimes send booleans as strings.
fn coerce_bool(v: &Value) -> Option<bool> {
    v.as_bool().or_else(|| match v.as_str() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    })
}

// ==================== Alignment Mode Types ====================

/// Expert tier for pool-based sampling (RFC 0048)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExpertTier {
    Core,
    Adjacent,
    Wildcard,
}

impl std::fmt::Display for ExpertTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpertTier::Core => write!(f, "Core"),
            ExpertTier::Adjacent => write!(f, "Adjacent"),
            ExpertTier::Wildcard => write!(f, "Wildcard"),
        }
    }
}

/// Rotation mode for expert panel sampling (RFC 0048, RFC 0050)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RotationMode {
    None,
    Wildcards,
    Full,
    /// RFC 0050: Judge-driven panel evolution with expert creation (default)
    #[default]
    Graduated,
}

/// A single expert in the pool (RFC 0048)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolExpert {
    pub role: String,
    pub tier: ExpertTier,
    pub relevance: f64,
}

/// Expert pool with tiered structure (RFC 0048)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertPool {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    pub experts: Vec<PoolExpert>,
}

/// A pastry-themed expert agent for alignment dialogues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PastryAgent {
    pub name: String,
    pub role: String,
    pub emoji: String,
    pub tier: String,
    pub relevance: f64,
    /// RFC 0050: Optional focus area for created experts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

// ==================== RFC 0050: Graduated Panel Rotation Types ====================

/// Source of an expert in a panel (RFC 0050)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExpertSource {
    /// Retained from previous round
    Retained,
    /// Pulled from the original pool
    Pool,
    /// Created on-demand by the Judge
    Created,
}

/// Panel expert specification for graduated rotation (RFC 0050)
/// Used by Judge to specify panel composition in round_prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelExpertSpec {
    /// Pastry name (existing or new)
    pub name: String,
    /// Expert role
    pub role: String,
    /// How the expert joined this panel
    pub source: ExpertSource,
    /// Tier (required for created experts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// Focus area (optional, useful for created experts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
}

/// Panel history entry for a single round (RFC 0050)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelHistory {
    pub round: usize,
    pub panel_size: usize,
    pub retained_count: usize,
    pub from_pool_count: usize,
    pub created_count: usize,
    pub experts: Vec<PanelExpertSpec>,
}

/// Pastry names for alignment agents (ADR 0014)
const PASTRY_NAMES: &[&str] = &[
    "Muffin",
    "Cupcake",
    "Scone",
    "Eclair",
    "Donut",
    "Brioche",
    "Croissant",
    "Macaron",
    "Cannoli",
    "Strudel",
    "Beignet",
    "Churro",
    "Profiterole",
    "Tartlet",
    "Galette",
    "Palmier",
    "Kouign",
    "Sfogliatella",
    "Financier",
    "Religieuse",
];

/// Validate that a name is a valid pastry name (ADR 0014)
fn is_valid_pastry_name(name: &str) -> bool {
    PASTRY_NAMES.contains(&name)
        || (name.starts_with("Pastry") && name[6..].chars().all(|c| c.is_ascii_digit()) && name.len() > 6)
}

/// Extraction status
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Complete,
    Truncated,
    PartialError,
}

/// Extraction result
#[derive(Debug, Serialize)]
pub struct ExtractionResult {
    pub text: String,
    pub status: ExtractionStatus,
    pub source_file: String,
    pub message_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<String>>,
}

/// Handle blue_extract_dialogue
pub fn handle_extract_dialogue(args: &Value) -> Result<Value, HandlerError> {
    let task_id = args.get("task_id").and_then(|v| v.as_str());
    let file_path_arg = args.get("file_path").and_then(|v| v.as_str());

    // Resolve file path
    let file_path = match (task_id, file_path_arg) {
        (Some(id), _) => resolve_task_output(id)?,
        (None, Some(path)) => PathBuf::from(path),
        (None, None) => {
            return Err(HandlerError::InvalidParams);
        }
    };

    // Verify file exists
    if !file_path.exists() {
        return Err(HandlerError::CommandFailed(format!(
            "JSONL file not found: {}",
            file_path.display()
        )));
    }

    // Try jq first, fall back to pure Rust
    let result = if jq_available() {
        extract_with_jq(&file_path)?
    } else {
        extract_with_rust(&file_path)?
    };

    let hint = match result.status {
        ExtractionStatus::Complete => format!(
            "Extracted {} assistant message(s) from {}",
            result.message_count,
            file_path.file_name().unwrap_or_default().to_string_lossy()
        ),
        ExtractionStatus::Truncated => format!(
            "Extracted {} assistant message(s), output truncated",
            result.message_count
        ),
        ExtractionStatus::PartialError => format!(
            "Extracted {} message(s) with {} error(s)",
            result.message_count,
            result.errors.as_ref().map(|e| e.len()).unwrap_or(0)
        ),
    };

    Ok(serde_json::json!({
        "status": "success",
        "message": crate::voice::info(
            &format!("Extracted {} messages", result.message_count),
            Some(&hint)
        ),
        "text": result.text,
        "extraction_status": format!("{:?}", result.status).to_lowercase(),
        "source_file": result.source_file,
        "message_count": result.message_count,
        "errors": result.errors
    }))
}

/// Resolve file path from task_id
fn resolve_task_output(task_id: &str) -> Result<PathBuf, HandlerError> {
    // Look for task output symlink in /tmp/claude/.../tasks/
    let tmp_claude = PathBuf::from("/tmp/claude");
    if !tmp_claude.exists() {
        return Err(HandlerError::CommandFailed(
            "No /tmp/claude directory found. Is Claude Code running?".to_string(),
        ));
    }

    // Search for task output file
    for entry in fs::read_dir(&tmp_claude)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to read /tmp/claude: {}", e)))?
    {
        let entry = entry.map_err(|e| {
            HandlerError::CommandFailed(format!("Failed to read directory entry: {}", e))
        })?;
        let tasks_dir = entry.path().join("tasks");
        if tasks_dir.exists() {
            let output_file = tasks_dir.join(format!("{}.output", task_id));
            if output_file.exists() {
                // Follow symlink to get actual file
                let resolved = fs::read_link(&output_file).unwrap_or(output_file.clone());
                return Ok(resolved);
            }
        }
    }

    Err(HandlerError::CommandFailed(format!(
        "Task output not found for task_id: {}",
        task_id
    )))
}

/// Check if jq is available
fn jq_available() -> bool {
    Command::new("jq")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract dialogue using jq (faster for large files)
fn extract_with_jq(file_path: &Path) -> Result<ExtractionResult, HandlerError> {
    let output = Command::new("jq")
        .arg("-r")
        .arg(r#"select(.type == "assistant") | .message.content[]? | select(.type == "text") | .text"#)
        .arg(file_path)
        .output()
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to run jq: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(HandlerError::CommandFailed(format!("jq failed: {}", stderr)));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();

    // Count messages by counting non-empty segments
    let message_count = text.split("\n\n").filter(|s| !s.trim().is_empty()).count();

    // Check for truncation (arbitrary limit: 500KB)
    let status = if text.len() > 500_000 {
        ExtractionStatus::Truncated
    } else {
        ExtractionStatus::Complete
    };

    Ok(ExtractionResult {
        text,
        status,
        source_file: file_path.to_string_lossy().to_string(),
        message_count,
        errors: None,
    })
}

/// Extract dialogue using pure Rust (fallback)
fn extract_with_rust(file_path: &Path) -> Result<ExtractionResult, HandlerError> {
    let file = File::open(file_path)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to open file: {}", e)))?;

    let reader = BufReader::new(file);
    let mut texts = Vec::new();
    let mut errors = Vec::new();
    let mut message_count = 0;

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                errors.push(format!("Line {}: read error: {}", line_num + 1, e));
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON line
        let json_value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("Line {}: JSON parse error: {}", line_num + 1, e));
                continue;
            }
        };

        // Check if this is an assistant message
        if json_value.get("type").and_then(|v| v.as_str()) != Some("assistant") {
            continue;
        }

        // Extract text content from message.content array
        if let Some(content_array) = json_value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for content_item in content_array {
                if content_item.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                        texts.push(text.to_string());
                        message_count += 1;
                    }
                }
            }
        }
    }

    let text = texts.join("\n\n");

    // Determine status
    let status = if !errors.is_empty() {
        ExtractionStatus::PartialError
    } else if text.len() > 500_000 {
        ExtractionStatus::Truncated
    } else {
        ExtractionStatus::Complete
    };

    Ok(ExtractionResult {
        text,
        status,
        source_file: file_path.to_string_lossy().to_string(),
        message_count,
        errors: if errors.is_empty() {
            None
        } else {
            Some(errors)
        },
    })
}

// ==================== Dialogue Document Handlers ====================

/// Handle blue_dialogue_create
///
/// Creates a new dialogue document with SQLite metadata.
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let rfc_title = args.get("rfc_title").and_then(|v| v.as_str());
    let summary = args.get("summary").and_then(|v| v.as_str());
    let content = args.get("content").and_then(|v| v.as_str());

    // Alignment mode params
    let alignment = args.get("alignment").and_then(coerce_bool).unwrap_or(false);
    let model = args
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("sonnet");
    let sources: Vec<String> = args
        .get("sources")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // RFC 0048: Expert pool parameters
    let expert_pool: Option<ExpertPool> = args
        .get("expert_pool")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    let panel_size = args
        .get("panel_size")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let rotation: RotationMode = args
        .get("rotation")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "wildcards" => RotationMode::Wildcards,
            "full" => RotationMode::Full,
            "graduated" => RotationMode::Graduated,
            _ => RotationMode::None,
        })
        .unwrap_or_default();

    // RFC 0048: Alignment mode requires expert_pool
    if alignment && expert_pool.is_none() {
        return Err(HandlerError::CommandFailed(
            "Alignment dialogues require expert_pool parameter (RFC 0048)".to_string(),
        ));
    }

    // Validate RFC exists if provided
    let rfc_doc = if let Some(rfc) = rfc_title {
        Some(
            state
                .store
                .find_document(DocType::Rfc, rfc)
                .map_err(|_| HandlerError::NotFound(format!("RFC '{}' not found", rfc)))?,
        )
    } else {
        None
    };

    // Get next dialogue number
    let dialogue_number = state
        .store
        .next_number_with_fs(DocType::Dialogue, &state.home.docs_path)
        .map_err(|e| HandlerError::CommandFailed(e.to_string()))?;

    // Generate file path with ISO 8601 timestamp prefix (RFC 0031)
    let timestamp = crate::utc_timestamp();
    let slug = title_to_slug(title);
    let file_name = format!("{}-{}.dialogue.recorded.md", timestamp, slug);
    let file_path = PathBuf::from("dialogues").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let dialogue_path = docs_path.join(&file_path);

    // Generate markdown content — alignment mode gets a different scaffold
    let (markdown, pastry_agents, pool_for_response) = if alignment {
        // RFC 0048: Use expert pool for alignment mode
        let pool = expert_pool.unwrap(); // Safe: validated above
        let size = panel_size.unwrap_or_else(|| pool.experts.len().min(12));
        let sampled = sample_panel_from_pool(&pool, size);
        let agents = assign_pastry_names(sampled);
        let md = generate_alignment_dialogue_markdown(
            title,
            dialogue_number,
            rfc_title,
            &agents,
            Some(&pool),
        );
        (md, Some(agents), Some(pool))
    } else {
        let md = generate_dialogue_markdown(title, dialogue_number, rfc_title, summary, content);
        (md, None, None)
    };

    // Create dialogues directory if it doesn't exist
    if let Some(parent) = dialogue_path.parent() {
        fs::create_dir_all(parent).map_err(|e| HandlerError::CommandFailed(e.to_string()))?;
    }

    // Overwrite protection (RFC 0031)
    if dialogue_path.exists() {
        return Err(HandlerError::CommandFailed(format!(
            "File already exists: {}",
            dialogue_path.display()
        )));
    }

    // Create document in SQLite store
    let mut doc = Document::new(DocType::Dialogue, title, "recorded");
    doc.number = Some(dialogue_number);
    doc.file_path = Some(file_path.to_string_lossy().to_string());

    let dialogue_id = state
        .store
        .add_document(&doc)
        .map_err(|e| HandlerError::CommandFailed(e.to_string()))?;

    // Link to RFC if provided
    if let Some(ref rfc) = rfc_doc {
        if let (Some(rfc_id), Some(dialogue_id)) = (rfc.id, Some(dialogue_id)) {
            let _ = state
                .store
                .link_documents(dialogue_id, rfc_id, LinkType::DialogueToRfc);
        }
    }

    fs::write(&dialogue_path, &markdown).map_err(|e| HandlerError::CommandFailed(e.to_string()))?;

    // Build response — RFC 0023: inject protocol as prose in message field
    let (message, judge_protocol) = if let Some(ref agents) = pastry_agents {
        // RFC 0057: Create output directory in .blue/dialogues/<ISO>-<name>/
        // Format: .blue/dialogues/2026-02-03T1423Z-topic-name/
        let output_dir = format!(".blue/dialogues/{}-{}", timestamp, slug);
        fs::create_dir_all(&output_dir).map_err(|e| {
            HandlerError::CommandFailed(format!("Failed to create output dir {}: {}", output_dir, e))
        })?;

        // RFC 0048: Persist expert pool to output directory
        if let Some(ref pool) = pool_for_response {
            let pool_path = format!("{}/expert-pool.json", output_dir);
            let pool_json = serde_json::to_string_pretty(pool).map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to serialize pool: {}", e))
            })?;
            fs::write(&pool_path, pool_json)
                .map_err(|e| HandlerError::CommandFailed(format!("Failed to write pool: {}", e)))?;
        }

        let protocol = build_judge_protocol(
            agents,
            &dialogue_path.display().to_string(),
            model,
            &sources,
            &output_dir,
            pool_for_response.as_ref(),
            rotation,
        );
        // Extract instructions as prose so Claude reads them directly
        let instructions = protocol["instructions"].as_str().unwrap_or("");
        let template = protocol["agent_prompt_template"].as_str().unwrap_or("");
        let msg = format!(
            "Alignment dialogue created: {title}\n\
             File: {file}\n\n\
             ## JUDGE PROTOCOL — FOLLOW THESE INSTRUCTIONS\n\n\
             {instructions}\n\n\
             ## AGENT PROMPT TEMPLATE (Reference)\n\n\
             Use blue_dialogue_round_prompt to get fully-substituted prompts for each agent.\n\
             Template shown here for reference only:\n\n\
             {template}",
            title = title,
            file = dialogue_path.display(),
            instructions = instructions,
            template = template,
        );
        (msg, Some(protocol))
    } else {
        let msg = crate::voice::info(
            &format!("Dialogue recorded: {}", title),
            Some(if rfc_title.is_some() {
                "Dialogue recorded and linked to RFC."
            } else {
                "Dialogue recorded. Consider linking to an RFC."
            }),
        );
        (msg, None)
    };

    let mut response = json!({
        "status": "success",
        "message": message,
        "dialogue": {
            "id": dialogue_id,
            "number": dialogue_number,
            "title": title,
            "file": dialogue_path.display().to_string(),
            "linked_rfc": rfc_title,
        },
        "content": markdown,
    });

    // Also attach structured protocol for programmatic use
    if let Some(protocol) = judge_protocol {
        response
            .as_object_mut()
            .unwrap()
            .insert("judge_protocol".to_string(), protocol);
    }

    Ok(response)
}

/// Handle blue_dialogue_get
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Dialogue, title)
        .map_err(|e| HandlerError::StateLoadFailed(e.to_string()))?;

    // Read file content if available
    let content = if let Some(ref rel_path) = doc.file_path {
        let file_path = state.home.docs_path.join(rel_path);
        fs::read_to_string(&file_path).ok()
    } else {
        None
    };

    // Get linked RFC if any
    let linked_rfc = if let Some(doc_id) = doc.id {
        state
            .store
            .get_linked_documents(doc_id, Some(LinkType::DialogueToRfc))
            .ok()
            .and_then(|docs| docs.into_iter().next())
            .map(|d| d.title)
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "message": crate::voice::info(
            &format!("Dialogue: {}", doc.title),
            None
        ),
        "dialogue": {
            "id": doc.id,
            "number": doc.number,
            "title": doc.title,
            "status": doc.status,
            "file_path": doc.file_path,
            "linked_rfc": linked_rfc,
            "created_at": doc.created_at,
        },
        "content": content,
    }))
}

/// Handle blue_dialogue_list
pub fn handle_list(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let rfc_filter = args.get("rfc_title").and_then(|v| v.as_str());

    let all_dialogues = state
        .store
        .list_documents(DocType::Dialogue)
        .map_err(|e| HandlerError::StateLoadFailed(e.to_string()))?;

    // Filter by RFC if specified
    let dialogues: Vec<_> = if let Some(rfc_title) = rfc_filter {
        // First find the RFC
        let rfc_doc = state
            .store
            .find_document(DocType::Rfc, rfc_title)
            .map_err(|e| HandlerError::StateLoadFailed(e.to_string()))?;

        // Find dialogues linked to this RFC
        if let Some(rfc_id) = rfc_doc.id {
            all_dialogues
                .into_iter()
                .filter(|d| {
                    if let Some(doc_id) = d.id {
                        state
                            .store
                            .get_linked_documents(doc_id, Some(LinkType::DialogueToRfc))
                            .map(|linked| linked.iter().any(|l| l.id == Some(rfc_id)))
                            .unwrap_or(false)
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    } else {
        all_dialogues
    };

    let hint = if dialogues.is_empty() {
        if rfc_filter.is_some() {
            "No dialogues for this RFC."
        } else {
            "No dialogues recorded. Create one with blue_dialogue_create."
        }
    } else {
        "Use blue_dialogue_get to view full content."
    };

    Ok(json!({
        "status": "success",
        "message": crate::voice::info(
            &format!("{} dialogue(s)", dialogues.len()),
            Some(hint)
        ),
        "dialogues": dialogues.iter().map(|d| json!({
            "id": d.id,
            "number": d.number,
            "title": d.title,
            "status": d.status,
            "file_path": d.file_path,
            "created_at": d.created_at,
        })).collect::<Vec<_>>(),
        "count": dialogues.len(),
        "rfc_filter": rfc_filter,
    }))
}

/// Handle blue_dialogue_save (extends extract_dialogue to save with metadata)
pub fn handle_save(state: &mut ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let task_id = args.get("task_id").and_then(|v| v.as_str());
    let file_path_arg = args.get("file_path").and_then(|v| v.as_str());
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let rfc_title = args.get("rfc_title").and_then(|v| v.as_str());
    let summary = args.get("summary").and_then(|v| v.as_str());

    // Resolve and extract content from JSONL
    let jsonl_path = match (task_id, file_path_arg) {
        (Some(id), _) => resolve_task_output(id)?,
        (None, Some(path)) => PathBuf::from(path),
        (None, None) => {
            return Err(HandlerError::InvalidParams);
        }
    };

    // Verify file exists
    if !jsonl_path.exists() {
        return Err(HandlerError::CommandFailed(format!(
            "JSONL file not found: {}",
            jsonl_path.display()
        )));
    }

    // Extract dialogue content
    let extraction = if jq_available() {
        extract_with_jq(&jsonl_path)?
    } else {
        extract_with_rust(&jsonl_path)?
    };

    // Now create the dialogue document with extracted content
    let create_args = json!({
        "title": title,
        "rfc_title": rfc_title,
        "summary": summary,
        "content": extraction.text,
    });

    let mut result = handle_create(state, &create_args)?;

    // Add extraction metadata to result
    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "extraction".to_string(),
            json!({
                "source_file": extraction.source_file,
                "message_count": extraction.message_count,
                "status": format!("{:?}", extraction.status).to_lowercase(),
            }),
        );
    }

    Ok(result)
}

// ==================== Helper Functions ====================

/// Generate dialogue markdown content
fn generate_dialogue_markdown(
    title: &str,
    number: i32,
    rfc_title: Option<&str>,
    summary: Option<&str>,
    content: Option<&str>,
) -> String {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let time = chrono::Utc::now().format("%H:%MZ").to_string();

    let mut md = String::new();

    // Title
    md.push_str(&format!(
        "# Dialogue {:04}: {}\n\n",
        number,
        to_title_case(title)
    ));

    // Metadata table
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!("| **Date** | {} {} |\n", date, time));
    md.push_str("| **Status** | Recorded |\n");
    if let Some(rfc) = rfc_title {
        md.push_str(&format!("| **RFC** | {} |\n", rfc));
    }
    md.push_str("\n---\n\n");

    // Summary
    if let Some(sum) = summary {
        md.push_str("## Summary\n\n");
        md.push_str(sum);
        md.push_str("\n\n");
    }

    // Dialogue content
    md.push_str("## Dialogue\n\n");
    if let Some(c) = content {
        md.push_str(c);
    } else {
        md.push_str("[Dialogue content to be added]\n");
    }
    md.push_str("\n\n");

    // Rounds section placeholder
    md.push_str("## Rounds\n\n");
    md.push_str("| Round | Topic | Outcome |\n");
    md.push_str("|-------|-------|--------|\n");
    md.push_str("| 1 | [Topic] | [Outcome] |\n");
    md.push('\n');

    // Lessons learned
    md.push_str("## Lessons Learned\n\n");
    md.push_str("- [Key insight from this dialogue]\n");

    md
}

// ==================== Alignment Mode Helpers ====================

/// Weighted random sampling without replacement (RFC 0048)
/// Higher relevance = higher selection probability
fn weighted_sample(experts: &[PoolExpert], n: usize) -> Vec<PoolExpert> {
    if n >= experts.len() {
        return experts.to_vec();
    }

    let mut rng = rand::thread_rng();
    let mut remaining: Vec<_> = experts.to_vec();
    let mut selected = Vec::with_capacity(n);

    for _ in 0..n {
        if remaining.is_empty() {
            break;
        }

        let total_weight: f64 = remaining.iter().map(|e| e.relevance).sum();
        if total_weight <= 0.0 {
            // Fall back to uniform sampling if weights are zero
            let idx = rng.gen_range(0..remaining.len());
            selected.push(remaining.remove(idx));
            continue;
        }

        // Weighted selection
        let mut threshold = rng.gen::<f64>() * total_weight;
        let mut idx = 0;
        for (i, expert) in remaining.iter().enumerate() {
            threshold -= expert.relevance;
            if threshold <= 0.0 {
                idx = i;
                break;
            }
        }
        selected.push(remaining.remove(idx));
    }

    selected
}

/// Sample a panel from an expert pool (RFC 0048)
pub fn sample_panel_from_pool(pool: &ExpertPool, panel_size: usize) -> Vec<PoolExpert> {
    let (core_n, adj_n, wc_n) = tier_split(panel_size);

    // Separate experts by tier
    let core: Vec<_> = pool
        .experts
        .iter()
        .filter(|e| e.tier == ExpertTier::Core)
        .cloned()
        .collect();
    let adjacent: Vec<_> = pool
        .experts
        .iter()
        .filter(|e| e.tier == ExpertTier::Adjacent)
        .cloned()
        .collect();
    let wildcard: Vec<_> = pool
        .experts
        .iter()
        .filter(|e| e.tier == ExpertTier::Wildcard)
        .cloned()
        .collect();

    // Sample from each tier
    let mut panel = Vec::new();
    panel.extend(weighted_sample(&core, core_n));
    panel.extend(weighted_sample(&adjacent, adj_n));
    panel.extend(weighted_sample(&wildcard, wc_n));

    // If we don't have enough in a tier, fill from others
    while panel.len() < panel_size && panel.len() < pool.experts.len() {
        let used_roles: std::collections::HashSet<_> = panel.iter().map(|e| &e.role).collect();
        let remaining: Vec<_> = pool
            .experts
            .iter()
            .filter(|e| !used_roles.contains(&e.role))
            .cloned()
            .collect();
        if remaining.is_empty() {
            break;
        }
        let sampled = weighted_sample(&remaining, 1);
        panel.extend(sampled);
    }

    panel
}

/// Compute tier boundaries for agent assignment
fn tier_split(count: usize) -> (usize, usize, usize) {
    if count <= 2 {
        (count, 0, 0)
    } else if count <= 5 {
        let core = 1_usize.max(count / 3);
        let wildcard = 1;
        let adjacent = count - core - wildcard;
        (core, adjacent, wildcard)
    } else {
        // ~33% core, ~42% adjacent, ~25% wildcard
        let wildcard = count / 4;
        let core = count / 3;
        let adjacent = count - core - wildcard;
        (core, adjacent, wildcard)
    }
}

/// Generate a context brief for fresh experts joining mid-dialogue (RFC 0050)
///
/// Fresh experts (from pool or created) need context about what happened
/// in prior rounds so they can engage meaningfully.
pub fn generate_context_brief(output_dir: &str, round: usize) -> Result<String, HandlerError> {
    if round == 0 {
        return Ok(String::new());
    }

    let prev_round = round - 1;
    let mut brief = format!("## Context for Round {}\n\n", round);
    brief.push_str("You are joining this dialogue in progress. Here's what happened:\n\n");

    // Try to read tensions file
    let tensions_path = format!("{}/tensions.md", output_dir);
    if let Ok(tensions) = fs::read_to_string(&tensions_path) {
        brief.push_str("### Key Tensions\n\n");
        // Extract tension lines (lines starting with | T)
        let tension_lines: Vec<&str> = tensions
            .lines()
            .filter(|line| line.starts_with("| T") || line.starts_with("|T"))
            .collect();
        if tension_lines.is_empty() {
            brief.push_str("No tensions recorded yet.\n\n");
        } else {
            for line in tension_lines.iter().take(5) {
                brief.push_str(line);
                brief.push('\n');
            }
            if tension_lines.len() > 5 {
                brief.push_str(&format!(
                    "... and {} more tensions\n",
                    tension_lines.len() - 5
                ));
            }
            brief.push('\n');
        }
    }

    // Try to read prior round summary
    let summary_path = format!("{}/round-{}.summary.md", output_dir, prev_round);
    if let Ok(summary) = fs::read_to_string(&summary_path) {
        brief.push_str(&format!("### Round {} Summary\n\n", prev_round));
        // Take first 500 chars of summary
        let truncated: String = summary.chars().take(500).collect();
        brief.push_str(&truncated);
        if summary.len() > 500 {
            brief.push_str("...\n");
        }
        brief.push('\n');
    }

    // Try to read panel composition from prior round
    let panel_path = format!("{}/round-{}/panel.json", output_dir, prev_round);
    if let Ok(panel_json) = fs::read_to_string(&panel_path) {
        if let Ok(panel) = serde_json::from_str::<Vec<PastryAgent>>(&panel_json) {
            brief.push_str(&format!("### Round {} Panel\n\n", prev_round));
            for agent in panel.iter().take(8) {
                brief.push_str(&format!(
                    "- {} {}: {}\n",
                    agent.emoji, agent.name, agent.role
                ));
            }
            if panel.len() > 8 {
                brief.push_str(&format!("... and {} more experts\n", panel.len() - 8));
            }
            brief.push('\n');
        }
    }

    brief.push_str("### Your Task\n\n");
    brief.push_str("Review these positions and contribute your fresh perspective. ");
    brief.push_str("You bring a viewpoint that may have been missing from earlier rounds.\n");

    Ok(brief)
}

/// Assign pastry names to sampled experts (RFC 0048)
pub fn assign_pastry_names(sampled: Vec<PoolExpert>) -> Vec<PastryAgent> {
    sampled
        .into_iter()
        .enumerate()
        .map(|(i, expert)| {
            let name = if i < PASTRY_NAMES.len() {
                PASTRY_NAMES[i].to_string()
            } else {
                format!("Pastry{}", i + 1)
            };
            PastryAgent {
                name,
                role: expert.role,
                emoji: "🧁".to_string(),
                tier: expert.tier.to_string(),
                relevance: expert.relevance,
                focus: None,
            }
        })
        .collect()
}

/// Generate alignment dialogue markdown scaffold (ADR 0014 compliant)
pub fn generate_alignment_dialogue_markdown(
    title: &str,
    number: i32,
    rfc_title: Option<&str>,
    agents: &[PastryAgent],
    pool: Option<&ExpertPool>,
) -> String {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let time = chrono::Utc::now().format("%H:%MZ").to_string();

    let mut md = String::new();

    // Title
    md.push_str(&format!(
        "# Alignment Dialogue: {}\n\n",
        to_title_case(title)
    ));

    // Metadata
    md.push_str(&format!("**Draft**: Dialogue {:04}\n", number));
    md.push_str(&format!("**Date**: {} {}\n", date, time));
    md.push_str("**Status**: In Progress\n");
    md.push_str(&format!(
        "**Participants**: 💙 Judge, {}\n",
        agents
            .iter()
            .map(|a| format!("{} {}", a.emoji, a.name))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(rfc) = rfc_title {
        md.push_str(&format!("**RFC**: {}\n", rfc));
    }
    md.push('\n');

    // Expert Pool section (RFC 0048)
    if let Some(p) = pool {
        md.push_str("## Expert Pool\n\n");
        md.push_str(&format!("**Domain**: {}\n", p.domain));
        if let Some(ref q) = p.question {
            md.push_str(&format!("**Question**: {}\n", q));
        }
        md.push('\n');

        // Group by tier
        let core: Vec<_> = p
            .experts
            .iter()
            .filter(|e| e.tier == ExpertTier::Core)
            .collect();
        let adjacent: Vec<_> = p
            .experts
            .iter()
            .filter(|e| e.tier == ExpertTier::Adjacent)
            .collect();
        let wildcard: Vec<_> = p
            .experts
            .iter()
            .filter(|e| e.tier == ExpertTier::Wildcard)
            .collect();

        md.push_str("| Tier | Experts |\n");
        md.push_str("|------|--------|\n");
        if !core.is_empty() {
            md.push_str(&format!(
                "| Core | {} |\n",
                core.iter()
                    .map(|e| e.role.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !adjacent.is_empty() {
            md.push_str(&format!(
                "| Adjacent | {} |\n",
                adjacent
                    .iter()
                    .map(|e| e.role.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !wildcard.is_empty() {
            md.push_str(&format!(
                "| Wildcard | {} |\n",
                wildcard
                    .iter()
                    .map(|e| e.role.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        md.push('\n');
    }

    // Expert Panel table (sampled for this dialogue)
    md.push_str("## Expert Panel\n\n");
    md.push_str("| Agent | Role | Tier | Relevance | Emoji |\n");
    md.push_str("|-------|------|------|-----------|-------|\n");
    md.push_str("| 💙 Judge | Orchestrator | — | — | 💙 |\n");
    for agent in agents {
        md.push_str(&format!(
            "| {} {} | {} | {} | {:.2} | {} |\n",
            agent.emoji, agent.name, agent.role, agent.tier, agent.relevance, agent.emoji
        ));
    }
    md.push('\n');

    // Alignment Scoreboard (empty)
    md.push_str("## Alignment Scoreboard\n\n");
    md.push_str("| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |\n");
    md.push_str("|-------|--------|-------------|-------|---------------|----------|\n");
    for agent in agents {
        md.push_str(&format!(
            "| {} {} | 0 | 0 | 0 | 0 | **0** |\n",
            agent.emoji, agent.name
        ));
    }
    md.push_str("\n**Total ALIGNMENT**: 0\n\n");

    // Perspectives Inventory (empty)
    md.push_str("## Perspectives Inventory\n\n");
    md.push_str("| ID | Agent | Perspective | Round |\n");
    md.push_str("|----|-------|-------------|-------|\n");
    md.push_str("| — | — | [Awaiting Round 0] | — |\n\n");

    // Tensions Tracker (empty)
    md.push_str("## Tensions Tracker\n\n");
    md.push_str("| ID | Tension | Status | Raised | Resolved |\n");
    md.push_str("|----|---------|--------|--------|----------|\n");
    md.push_str("| — | [Awaiting Round 0] | — | — | — |\n\n");

    // Opening Arguments placeholder
    md.push_str("## Round 0: Opening Arguments\n\n");
    for agent in agents {
        md.push_str(&format!("### {} {}\n\n", agent.name, agent.emoji));
        md.push_str("[Awaiting response]\n\n");
    }

    md
}

/// Build the judge protocol JSON returned to the caller
pub fn build_judge_protocol(
    agents: &[PastryAgent],
    dialogue_file: &str,
    model: &str,
    sources: &[String],
    output_dir: &str,
    pool: Option<&ExpertPool>,
    rotation: RotationMode,
) -> Value {
    let agent_list: Vec<Value> = agents
        .iter()
        .map(|a| {
            json!({
                "name": a.name,
                "role": a.role,
                "emoji": a.emoji,
                "tier": a.tier,
                "relevance": a.relevance,
                "name_lowercase": a.name.to_lowercase(),
            })
        })
        .collect();

    let source_read_instructions = if sources.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nGROUNDING: Before responding, use the Read tool to read these files:\n{}",
            sources
                .iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    let agent_prompt_template = format!(
        r##"You are {{{{NAME}}}} {{{{EMOJI}}}}, a {{{{ROLE}}}} in an ALIGNMENT-seeking dialogue.

Your role:
- SURFACE perspectives others may have missed
- DEFEND valuable ideas with evidence, not ego
- CHALLENGE assumptions with curiosity, not destruction
- INTEGRATE perspectives that resonate
- CONCEDE gracefully when others see something you missed

Your contribution is scored on PRECISION, not volume.
One sharp insight beats ten paragraphs. You ALL win when the result is aligned.

{{{{CONTEXT_INSTRUCTIONS}}}}

=== MANDATORY FILE OUTPUT ===

You MUST write your response to a file. This is NOT optional.

OUTPUT FILE: {{{{OUTPUT_FILE}}}}

Use the Write tool to write your COMPLETE response to the file above.
If you return your response without writing to the file, YOUR WORK WILL BE LOST.

=== FILE CONTENT STRUCTURE ===

Write EXACTLY this structure to the file:

[PERSPECTIVE P01: brief label]
Your strongest new viewpoint. Two to four sentences maximum. No preamble.

[PERSPECTIVE P02: brief label]  ← optional, only if genuinely distinct
One to two sentences maximum.

[TENSION T01: brief description]  ← optional
One sentence identifying the unresolved issue.

[REFINEMENT: description] or [CONCESSION: description] or [RESOLVED Tn]  ← optional
One sentence each. Use only when engaging with prior round content.

---
Nothing else. No introduction. No conclusion. No elaboration.

=== RETURN CONFIRMATION ===

AFTER writing the file, return ONLY this structured confirmation to the Judge:

FILE_WRITTEN: {{{{OUTPUT_FILE}}}}
Perspectives: P01 [label], P02 [label]
Tensions: T01 [label] or none
Moves: [CONCESSION|REFINEMENT|RESOLVED] or none
Claim: [your single strongest claim in one sentence]

Five lines. The FILE_WRITTEN line proves you wrote the file. Without it, the Judge assumes your work was lost.{source_read_instructions}"##
    );

    let instructions = format!(
        r##"You are the 💙 Judge. Orchestrate this alignment dialogue.

=== FILE ARCHITECTURE (RFC 0033) ===

```
{output_dir}/
├─ scoreboard.md              ← You write + read (~500 bytes)
├─ tensions.md                ← You write, agents read (~1-2KB)
├─ round-0/
│  └─ {{agent}}.md            ← Agents write, peers read (~2-3KB each)
├─ round-0.summary.md         ← You write, agents read (~1-2KB)
└─ round-1/...
```

Every file has exactly one writer and at least one reader.

=== HOW TO SPAWN EXPERT SUBAGENTS ===

BEFORE spawning each round, create the round directory:
  Use Bash: mkdir -p {output_dir}/round-N

For EACH agent, call blue_dialogue_round_prompt to get the fully-substituted prompt:
  blue_dialogue_round_prompt(output_dir="{output_dir}", agent_name="Muffin", agent_emoji="🧁", agent_role="Platform Architect", round=N)
  → Returns ready-to-use prompt with all substitutions done

Then spawn ALL {agent_count} experts in a SINGLE message with {agent_count} Task tool calls.
Multiple Task calls in one message run as parallel subagents.

Each Task call uses the prompt from blue_dialogue_round_prompt:
- subagent_type: "general-purpose" (from task_params in response)
- description: "🧁 Muffin expert deliberation" (from task_params in response)
- prompt: the "prompt" field from blue_dialogue_round_prompt response (already substituted)

All {agent_count} results return when complete WITH STRUCTURED CONFIRMATIONS.

=== ROUND WORKFLOW ===

1. MKDIR: Create round directory via Bash: mkdir -p {output_dir}/round-N
2. SPAWN: One message, {agent_count} Task calls (parallel subagents)
3. COLLECT & VERIFY: Each agent returns a 5-line structured confirmation:
   ```
   FILE_WRITTEN: /path/to/file.md
   Perspectives: P01 [label], P02 [label]
   Tensions: T01 [label] or none
   Moves: [CONCESSION|REFINEMENT|RESOLVED] or none
   Claim: [single sentence]
   ```
   - If FILE_WRITTEN line is present: Agent wrote their file (no action needed)
   - If FILE_WRITTEN line is MISSING: Agent failed to write. Use Read to check if file exists.
     If file missing, the agent's full response may be in the return — write it yourself as fallback.
   Use summaries for synthesis. Read files with Read tool only if summary is insufficient.
4. SCORE: ALIGNMENT = Wisdom + Consistency + Truth + Relationships (UNBOUNDED)
   - Score ONLY AFTER reading agent returns — NEVER pre-fill scores
5. WRITE ARTIFACTS — THIS IS MANDATORY (agents read these files next round):
   Use the Write tool for EACH file. If you skip this, agents have NO context next round.
   a. Write {output_dir}/scoreboard.md — current scores for all agents
   b. Write {output_dir}/tensions.md — ALL tensions (new + prior + resolved) in markdown table format
   c. Write {output_dir}/round-N.summary.md — your synthesis (what converged, what tensions remain open)
   You MUST write all three files BEFORE updating the dialogue file.
6. UPDATE ARCHIVAL RECORD — after writing artifacts:
   Use the Edit tool to append to {dialogue_file}:
   - Round summary (from round-N.summary.md) — NOT full agent responses (those are on disk)
   - Updated Scoreboard table (copy from scoreboard.md)
   - Updated Perspectives Inventory (one row per [PERSPECTIVE Pnn:] marker)
   - Updated Tensions Tracker (one row per [TENSION Tn:] marker)
   Full agent responses stay in {output_dir}/round-N/*.md (ADR 0005: reference, don't copy).
7. CONVERGENCE CHECK (RFC 0057) — BOTH CONDITIONS REQUIRED:
   a. Calculate VELOCITY = open_tensions + new_perspectives
      - Count open/reopened tensions (not resolved or accepted)
      - Count new perspectives surfaced THIS round
   b. Calculate CONVERGE % = (experts_signaling_converge / panel_size) × 100
      - Expert signals convergence via `[MOVE:CONVERGE]` marker in their response
   c. Convergence ONLY when: velocity == 0 AND converge_percent == 100%
   d. If velocity > 0: Start next round to resolve tensions/perspectives
   e. If converge < 100%: Start next round — experts not aligned
   f. Maximum 10 rounds (safety valve) — force convergence with warning

=== SCOREBOARD FORMAT (RFC 0057) ===

The scoreboard tracks ALIGNMENT progress AND convergence metrics.

```markdown
| Round | W | C | T | R | Score | Σ | Open T | New P | Velocity | Converge % |
|-------|---|---|---|---|-------|---|--------|-------|----------|------------|
| 0     | 5 | 3 | 4 | 2 | 14    |14 | 3      | 5     | 8        | 0%         |
| 1     | 8 | 4 | 5 | 3 | 20    |34 | 1      | 2     | 3        | 50%        |
| 2     | 4 | 2 | 2 | 1 | 9     |43 | 0      | 0     | 0        | 100%       |
```

Columns:
- W/C/T/R: Wisdom, Consistency, Truth, Relationships (ALIGNMENT components)
- Score: This round's total (W+C+T+R)
- Σ: Cumulative total score
- Open T: Open tensions count (velocity component 1)
- New P: New perspectives this round (velocity component 2)
- Velocity: Open T + New P (must be 0 to converge)
- Converge %: Experts signaling `[MOVE:CONVERGE]` / panel size × 100

=== TOKEN BUDGET ===

Your reads per round: ~5KB (scoreboard + tensions + prior summary)
Agent reads per round: ~15KB (tensions + peer files + prior summary)
Both well under 25K limit. Opus usage minimized.

AGENTS: {agent_names}
OUTPUT DIR: {output_dir}
MAX ROUNDS: 10

FORMAT RULES — MANDATORY:
- ALWAYS prefix agent names with their emoji (🧁 Muffin) not bare name (Muffin)
- The Judge is 💙 Judge — always include the 💙
- Expert Panel table columns: Agent | Role | Tier | Relevance | Emoji
- Round headers use emoji prefix (### 🧁 Muffin)
- Scores start at 0 — only fill after reading agent returns
- Scoreboard MUST include all RFC 0057 columns (W/C/T/R, Velocity, Converge %)

NOTE: blue_dialogue_round_prompt handles round-specific context (CONTEXT_INSTRUCTIONS) automatically:
- Round 0: No context instructions (agents have no memory of each other)
- Round 1+: Automatically includes READ CONTEXT block with correct paths"##,
        agent_count = agents.len(),
        dialogue_file = dialogue_file,
        output_dir = output_dir,
        agent_names = agents
            .iter()
            .map(|a| format!("{} {} ({})", a.emoji, a.name, a.role))
            .collect::<Vec<_>>()
            .join(", "),
    );

    // RFC 0050: Add graduated rotation guidelines when mode is graduated
    let instructions = if rotation == RotationMode::Graduated {
        format!(
            r##"{base_instructions}

=== GRADUATED PANEL ROTATION (RFC 0050) ===

The panel returned above is a **suggestion**. You have full control over panel composition.

**Before Round 0**: Review the suggested panel. If critical experts are missing, call
`blue_dialogue_evolve_panel` with round=0 to override it before spawning agents.

**Between rounds**: Decide how to evolve the panel based on dialogue dynamics.

Use blue_dialogue_evolve_panel to specify your panel:

```json
{{
  "output_dir": "{output_dir}",
  "round": N,
  "panel": [
    {{ "name": "Muffin", "role": "Value Analyst", "source": "retained" }},
    {{ "name": "Scone", "role": "Data Center Specialist", "source": "pool" }},
    {{ "name": "Palmier", "role": "Supply Chain Risk Analyst", "source": "created", "tier": "Adjacent", "focus": "Geographic concentration" }}
  ]
}}
```

### Retention Criteria
- **High scorers**: Experts who contributed sharp insights should continue
- **Unresolved advocates**: Experts defending positions with open tensions
- **Core relevance**: Experts central to the domain should anchor continuity

### Fresh Perspective Triggers
- **Stale consensus**: If the panel is converging too easily, bring challengers
- **Unexplored angles**: Pull in experts whose focus hasn't been represented
- **Low-scoring experts**: Consider rotating out experts who aren't contributing

### Targeted Expert Injection
When a specific tension emerges that no current expert can address:
1. Check if the pool has a relevant expert → source: "pool"
2. If not, create a new expert → source: "created" with tier and focus

### Panel Size Flexibility
- Target panel size is a guideline, not a constraint
- You may run a smaller panel if the dialogue is converging
- You may expand briefly to address a complex tension

### Expert Creation
You are not limited to the initial pool. If the dialogue surfaces a perspective that no pooled expert covers, create one with source: "created"."##,
            base_instructions = instructions,
            output_dir = output_dir,
        )
    } else {
        instructions
    };

    let mut result = json!({
        "instructions": instructions,
        "agent_prompt_template": agent_prompt_template,
        "dialogue_file": dialogue_file,
        "model": model,
        "sources": sources,
        "output_dir": output_dir,
        "rotation": format!("{:?}", rotation).to_lowercase(),
        // RFC 0057: Updated convergence params
        "convergence": {
            "max_rounds": 10,
            "tension_resolution_gate": true,
            // Velocity is now: open_tensions + new_perspectives (not threshold-based)
            "velocity_formula": "open_tensions + new_perspectives",
            // Convergence requires: velocity == 0 AND converge_percent == 100%
            "convergence_formula": "velocity == 0 AND converge_percent == 100%",
        },
    });

    // RFC 0050: For graduated rotation, the panel is a suggestion that the Judge can override
    // Use "suggested_panel" to make this clear; other modes use "agents" as the final panel
    if rotation == RotationMode::Graduated {
        result
            .as_object_mut()
            .unwrap()
            .insert("suggested_panel".to_string(), json!(agent_list));
        result
            .as_object_mut()
            .unwrap()
            .insert("panel_is_suggestion".to_string(), json!(true));
    } else {
        result
            .as_object_mut()
            .unwrap()
            .insert("agents".to_string(), json!(agent_list));
    }

    // RFC 0048: Include pool info if present
    if let Some(p) = pool {
        result.as_object_mut().unwrap().insert(
            "expert_pool".to_string(),
            json!({
                "domain": p.domain,
                "question": p.question,
                "total_experts": p.experts.len(),
                "pool_file": format!("{}/expert-pool.json", output_dir),
            }),
        );
    }

    result
}

/// Convert slug to title case
fn to_title_case(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Handle blue_dialogue_round_prompt
///
/// Returns a fully-substituted prompt for a specific agent and round,
/// ready to pass directly to the Task tool. Eliminates manual template substitution.
///
/// RFC 0050: Now accepts optional `expert_source` to generate context briefs for fresh experts.
pub fn handle_round_prompt(args: &Value) -> Result<Value, HandlerError> {
    // Required params
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let agent_name = args
        .get("agent_name")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    if !is_valid_pastry_name(agent_name) {
        return Err(HandlerError::CommandFailed(format!(
            "Agent name '{}' is not a valid pastry name. Use names from the suggested panel \
             (Muffin, Cupcake, Scone, Eclair, Donut, Brioche, Croissant, Macaron, Cannoli, \
             Strudel, Beignet, Churro, Profiterole, Tartlet, Galette, Palmier, Kouign, \
             Sfogliatella, Financier, Religieuse) or PastryN overflow format.",
            agent_name
        )));
    }
    let agent_emoji = args
        .get("agent_emoji")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let agent_role = args
        .get("agent_role")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let round = args
        .get("round")
        .and_then(|v| v.as_u64())
        .ok_or(HandlerError::InvalidParams)? as usize;

    // Optional params
    let sources: Vec<String> = args
        .get("sources")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // RFC 0050: Expert source for graduated rotation
    let expert_source: Option<ExpertSource> = args
        .get("expert_source")
        .and_then(|v| v.as_str())
        .and_then(|s| match s {
            "retained" => Some(ExpertSource::Retained),
            "pool" => Some(ExpertSource::Pool),
            "created" => Some(ExpertSource::Created),
            _ => None,
        });

    // RFC 0050: Optional focus for created experts
    let expert_focus = args.get("focus").and_then(|v| v.as_str());

    let agent_lowercase = agent_name.to_lowercase();
    let output_file = format!("{}/round-{}/{}.md", output_dir, round, agent_lowercase);

    // Build source read instructions
    let source_read_instructions = if sources.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nGROUNDING: Before responding, use the Read tool to read these files:\n{}",
            sources
                .iter()
                .map(|s| format!("- {}", s))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    // RFC 0050: Generate context brief for fresh experts (pool or created) joining after round 0
    let context_brief = if round > 0 && expert_source != Some(ExpertSource::Retained) {
        generate_context_brief(output_dir, round).unwrap_or_default()
    } else {
        String::new()
    };

    // RFC 0050: Focus instruction for created experts
    let focus_instruction = if let Some(focus) = expert_focus {
        format!(
            "\n\n**Your Focus**: {}\nBring this specialized perspective to the dialogue.",
            focus
        )
    } else {
        String::new()
    };

    // Build context instructions based on round and expert source
    let context_instructions = if round == 0 {
        // Round 0: No prior context to read, but agents can research if needed
        String::new()
    } else if expert_source == Some(ExpertSource::Retained) {
        // Retained experts read full context
        format!(
            r#"READ CONTEXT — THIS IS MANDATORY:
Use the Read tool to read these files BEFORE writing your response:
1. {output_dir}/tensions.md — accumulated tensions from all rounds
2. {output_dir}/round-{prev}.summary.md — Judge's synthesis of the prior round
3. Each .md file in {output_dir}/round-{prev}/ — peer perspectives from last round
You MUST read these files. Your response MUST engage with prior tensions and peer perspectives."#,
            output_dir = output_dir,
            prev = round - 1,
        )
    } else {
        // RFC 0050: Fresh experts get context brief + read instructions
        format!(
            r#"{context_brief}

READ CONTEXT — THIS IS MANDATORY:
Use the Read tool to read these files BEFORE writing your response:
1. {output_dir}/tensions.md — accumulated tensions from all rounds
2. {output_dir}/round-{prev}.summary.md — Judge's synthesis of the prior round
3. Each .md file in {output_dir}/round-{prev}/ — peer perspectives from last round
You MUST read these files. Your response MUST engage with prior tensions and peer perspectives."#,
            context_brief = context_brief,
            output_dir = output_dir,
            prev = round - 1,
        )
    };

    // Build the fully-substituted prompt
    let prompt = format!(
        r##"You are {name} {emoji}, a {role} in an ALIGNMENT-seeking dialogue.{focus_instruction}

Your role:
- SURFACE perspectives others may have missed
- DEFEND valuable ideas with evidence, not ego
- CHALLENGE assumptions with curiosity, not destruction
- INTEGRATE perspectives that resonate
- CONCEDE gracefully when others see something you missed

Your contribution is scored on PRECISION, not volume.
One sharp insight beats ten paragraphs. You ALL win when the result is aligned.

{context_instructions}

=== MANDATORY FILE OUTPUT ===

You MUST write your response to a file. This is NOT optional.

OUTPUT FILE: {output_file}

Use the Write tool to write your COMPLETE response to the file above.
If you return your response without writing to the file, YOUR WORK WILL BE LOST.

=== FILE CONTENT STRUCTURE ===

Write EXACTLY this structure to the file:

[PERSPECTIVE P01: brief label]
Your strongest new viewpoint. Two to four sentences maximum. No preamble.

[PERSPECTIVE P02: brief label]  ← optional, only if genuinely distinct
One to two sentences maximum.

[TENSION T01: brief description]  ← optional
One sentence identifying the unresolved issue.

[REFINEMENT: description] or [CONCESSION: description] or [RESOLVED Tn]  ← optional
One sentence each. Use only when engaging with prior round content.

---
Nothing else. No introduction. No conclusion. No elaboration.

=== RETURN CONFIRMATION ===

AFTER writing the file, return ONLY this structured confirmation to the Judge:

FILE_WRITTEN: {output_file}
Perspectives: P01 [label], P02 [label]
Tensions: T01 [label] or none
Moves: [CONCESSION|REFINEMENT|RESOLVED] or none
Claim: [your single strongest claim in one sentence]

Five lines. The FILE_WRITTEN line proves you wrote the file. Without it, the Judge assumes your work was lost.{source_read_instructions}"##,
        name = agent_name,
        emoji = agent_emoji,
        role = agent_role,
        focus_instruction = focus_instruction,
        context_instructions = context_instructions,
        output_file = output_file,
        source_read_instructions = source_read_instructions,
    );

    // Write prompt to file for audit/debugging
    // Ensure round directory exists first
    let round_dir = format!("{}/round-{}", output_dir, round);
    if let Err(e) = fs::create_dir_all(&round_dir) {
        eprintln!(
            "Warning: Failed to create round directory {}: {}",
            round_dir, e
        );
    }
    let prompt_file = format!("{}/{}.prompt.md", round_dir, agent_lowercase);
    if let Err(e) = fs::write(&prompt_file, &prompt) {
        eprintln!(
            "Warning: Failed to write prompt file {}: {}",
            prompt_file, e
        );
    }

    let mut response = json!({
        "status": "success",
        "prompt": prompt,
        "prompt_file": prompt_file,
        "output_file": output_file,
        "task_params": {
            "subagent_type": "general-purpose",
            "description": format!("{} {} expert deliberation", agent_emoji, agent_name),
        }
    });

    // RFC 0050: Include source metadata for graduated rotation
    if let Some(source) = expert_source {
        let source_str = match source {
            ExpertSource::Retained => "retained",
            ExpertSource::Pool => "pool",
            ExpertSource::Created => "created",
        };
        response
            .as_object_mut()
            .unwrap()
            .insert("expert_source".to_string(), json!(source_str));
        // Include context brief indicator for fresh experts
        if source != ExpertSource::Retained && round > 0 {
            response
                .as_object_mut()
                .unwrap()
                .insert("has_context_brief".to_string(), json!(true));
        }
    }

    if let Some(focus) = expert_focus {
        response
            .as_object_mut()
            .unwrap()
            .insert("focus".to_string(), json!(focus));
    }

    Ok(response)
}

/// Handle blue_dialogue_sample_panel (RFC 0048)
///
/// Sample a new panel from the expert pool for manual round control.
pub fn handle_sample_panel(args: &Value) -> Result<Value, HandlerError> {
    let dialogue_title = args
        .get("dialogue_title")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let round = args
        .get("round")
        .and_then(|v| v.as_u64())
        .ok_or(HandlerError::InvalidParams)? as usize;

    let panel_size = args
        .get("panel_size")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(12);

    // Parse retain/exclude lists
    let retain: Vec<String> = args
        .get("retain")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let exclude: Vec<String> = args
        .get("exclude")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Load pool from dialogue directory
    let slug = title_to_slug(dialogue_title);
    let pool_path = format!("/tmp/blue-dialogue/{}/expert-pool.json", slug);

    let pool_content = fs::read_to_string(&pool_path).map_err(|e| {
        HandlerError::CommandFailed(format!(
            "Failed to read expert pool at {}: {}. Did you create the dialogue with expert_pool?",
            pool_path, e
        ))
    })?;

    let pool: ExpertPool = serde_json::from_str(&pool_content)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to parse expert pool: {}", e)))?;

    // Filter pool based on retain/exclude
    let filtered: Vec<PoolExpert> = pool
        .experts
        .iter()
        .filter(|e| {
            let role_lower = e.role.to_lowercase();
            // Include if in retain list (if retain is non-empty)
            let in_retain = retain.is_empty()
                || retain
                    .iter()
                    .any(|r| role_lower.contains(&r.to_lowercase()));
            // Exclude if in exclude list
            let in_exclude = exclude
                .iter()
                .any(|x| role_lower.contains(&x.to_lowercase()));
            in_retain && !in_exclude
        })
        .cloned()
        .collect();

    if filtered.is_empty() {
        return Err(HandlerError::CommandFailed(
            "No experts remain after filtering. Check retain/exclude parameters.".to_string(),
        ));
    }

    // Create filtered pool for sampling
    let filtered_pool = ExpertPool {
        domain: pool.domain.clone(),
        question: pool.question.clone(),
        experts: filtered,
    };

    // Sample panel
    let sampled = sample_panel_from_pool(&filtered_pool, panel_size);
    let agents = assign_pastry_names(sampled);

    // Create round directory and save panel
    let output_dir = format!("/tmp/blue-dialogue/{}", slug);
    let round_dir = format!("{}/round-{}", output_dir, round);
    fs::create_dir_all(&round_dir)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to create round dir: {}", e)))?;

    let panel_path = format!("{}/panel.json", round_dir);
    let panel_json = serde_json::to_string_pretty(&agents)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to serialize panel: {}", e)))?;
    fs::write(&panel_path, panel_json)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to write panel: {}", e)))?;

    Ok(json!({
        "status": "success",
        "message": format!("Sampled {} experts for round {}", agents.len(), round),
        "round": round,
        "panel_file": panel_path,
        "panel": agents.iter().map(|a| json!({
            "name": a.name,
            "role": a.role,
            "emoji": a.emoji,
            "tier": a.tier,
            "relevance": a.relevance,
        })).collect::<Vec<_>>(),
    }))
}

/// Handle blue_dialogue_evolve_panel (RFC 0050)
///
/// Judge-driven panel evolution for graduated rotation mode.
/// The Judge specifies exactly which experts to include, their sources,
/// and can create new experts on-demand.
pub fn handle_evolve_panel(args: &Value) -> Result<Value, HandlerError> {
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let round = args
        .get("round")
        .and_then(|v| v.as_u64())
        .ok_or(HandlerError::InvalidParams)? as usize;

    // Parse panel specification
    let panel_spec: Vec<PanelExpertSpec> = args
        .get("panel")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| {
            HandlerError::CommandFailed(
                "panel parameter required: array of {name, role, source, tier?, focus?}"
                    .to_string(),
            )
        })?;

    if panel_spec.is_empty() {
        return Err(HandlerError::CommandFailed(
            "Panel cannot be empty".to_string(),
        ));
    }

    // Validate unique names
    let names: std::collections::HashSet<_> = panel_spec.iter().map(|e| &e.name).collect();
    if names.len() != panel_spec.len() {
        return Err(HandlerError::CommandFailed(
            "Expert names must be unique".to_string(),
        ));
    }

    // Load expert pool for validation and name lookup
    let pool_path = format!("{}/expert-pool.json", output_dir);
    let pool: Option<ExpertPool> = fs::read_to_string(&pool_path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok());

    // Track sources for response
    let mut retained_count = 0;
    let mut from_pool_count = 0;
    let mut created_count = 0;

    // Build panel agents
    let mut agents: Vec<PastryAgent> = Vec::new();
    let mut used_pastry_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for spec in &panel_spec {
        match spec.source {
            ExpertSource::Retained => retained_count += 1,
            ExpertSource::Pool => from_pool_count += 1,
            ExpertSource::Created => created_count += 1,
        }

        // Determine tier and relevance
        let (tier, relevance) = if spec.source == ExpertSource::Created {
            // Created experts use specified tier or default to Adjacent
            let tier = spec.tier.clone().unwrap_or_else(|| "Adjacent".to_string());
            (tier, 0.75) // Default relevance for created experts
        } else if let Some(ref p) = pool {
            // Look up from pool
            p.experts
                .iter()
                .find(|e| e.role.to_lowercase() == spec.role.to_lowercase())
                .map(|e| (e.tier.to_string(), e.relevance))
                .unwrap_or_else(|| ("Adjacent".to_string(), 0.70))
        } else {
            ("Adjacent".to_string(), 0.70)
        };

        // Assign pastry name if not already known
        let name = if PASTRY_NAMES.contains(&spec.name.as_str()) {
            used_pastry_names.insert(spec.name.clone());
            spec.name.clone()
        } else if spec.name.starts_with("Pastry") {
            // Accept overflow names
            spec.name.clone()
        } else {
            // Find next available pastry name
            let available = PASTRY_NAMES
                .iter()
                .find(|n| !used_pastry_names.contains(**n))
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Pastry{}", agents.len() + 21));
            used_pastry_names.insert(available.clone());
            available
        };

        agents.push(PastryAgent {
            name,
            role: spec.role.clone(),
            emoji: "🧁".to_string(),
            tier,
            relevance,
            focus: spec.focus.clone(),
        });
    }

    // Create round directory
    let round_dir = format!("{}/round-{}", output_dir, round);
    fs::create_dir_all(&round_dir)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to create round dir: {}", e)))?;

    // Build panel history
    let history = PanelHistory {
        round,
        panel_size: agents.len(),
        retained_count,
        from_pool_count,
        created_count,
        experts: panel_spec.clone(),
    };

    // Save panel with history
    let panel_path = format!("{}/panel.json", round_dir);
    let panel_data = json!({
        "agents": agents,
        "history": history,
    });
    let panel_json = serde_json::to_string_pretty(&panel_data)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to serialize panel: {}", e)))?;
    fs::write(&panel_path, panel_json)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to write panel: {}", e)))?;

    // Generate context brief for fresh experts if round > 0
    let context_brief = if round > 0 {
        generate_context_brief(output_dir, round).ok()
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "message": format!(
            "Panel evolved for round {}: {} retained, {} from pool, {} created",
            round, retained_count, from_pool_count, created_count
        ),
        "round": round,
        "panel_size": agents.len(),
        "retained": retained_count,
        "from_pool": from_pool_count,
        "created": created_count,
        "panel_file": panel_path,
        "context_brief": context_brief,
        "agents": agents.iter().map(|a| json!({
            "name": a.name,
            "role": a.role,
            "emoji": a.emoji,
            "tier": a.tier,
            "relevance": a.relevance,
            "focus": a.focus,
        })).collect::<Vec<_>>(),
    }))
}

// ==================== RFC 0051: Global Perspective & Tension Tracking ====================

/// Handle blue_dialogue_round_context (RFC 0051)
///
/// Bulk fetch context for all panel experts in a single call.
/// Returns structured data for prompt building: open perspectives, tensions, etc.
pub fn handle_round_context(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let dialogue_id = args
        .get("dialogue_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let round = args
        .get("round")
        .and_then(|v| v.as_i64())
        .ok_or(HandlerError::InvalidParams)? as i32;

    let conn = state.store.conn();

    // Get dialogue
    let dialogue = get_dialogue(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Dialogue not found: {}", e)))?;

    // Get experts
    let experts = get_experts(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get experts: {}", e)))?;

    // Get all perspectives
    let perspectives = get_perspectives(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get perspectives: {}", e)))?;

    // Get all tensions (filter for open/addressed in response)
    let tensions = get_tensions(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get tensions: {}", e)))?;

    // Get recommendations
    let recommendations = get_recommendations(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get recommendations: {}", e)))?;

    // Get evidence
    let evidence = get_evidence(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get evidence: {}", e)))?;

    // Get claims
    let claims = get_claims(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get claims: {}", e)))?;

    // Get verdicts
    let verdicts = get_verdicts(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get verdicts: {}", e)))?;

    // RFC 0057: Get scoreboard and convergence status
    let scoreboard = get_scoreboard(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get scoreboard: {}", e)))?;

    let (can_converge, blockers) = can_dialogue_converge(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to check convergence: {}", e)))?;

    let last_round_metrics = scoreboard.last().map(|r| {
        json!({
            "round": r.round,
            "score": {
                "W": r.w,
                "C": r.c,
                "T": r.t,
                "R": r.r,
                "total": r.total,
            },
            "velocity": r.velocity,
            "open_tensions": r.open_tensions,
            "new_perspectives": r.new_perspectives,
            "converge_signals": r.converge_signals,
            "panel_size": r.panel_size,
            "converge_percent": r.converge_percent,
            "cumulative_score": r.cumulative_score,
        })
    });

    // Build context response
    Ok(json!({
        "status": "success",
        "dialogue_id": dialogue_id,
        "round": round,
        "dialogue": {
            "title": dialogue.title,
            "question": dialogue.question,
            "status": dialogue.status.as_str(),
            "total_rounds": dialogue.total_rounds,
            "total_alignment": dialogue.total_alignment,
        },
        // RFC 0057: Convergence status
        "convergence": {
            "can_converge": can_converge,
            "blockers": blockers,
            "last_round": last_round_metrics,
        },
        "experts": experts.iter().map(|e| json!({
            "expert_slug": e.expert_slug,
            "role": e.role,
            "tier": e.tier.as_str(),
            "source": e.source.as_str(),
            "focus": e.focus,
            "total_score": e.total_score,
        })).collect::<Vec<_>>(),
        "perspectives": perspectives.iter().map(|p| json!({
            "id": display_id(EntityType::Perspective, p.round, p.seq),
            "round": p.round,
            "label": p.label,
            "content": p.content,
            "contributors": p.contributors,
            "status": p.status.as_str(),
        })).collect::<Vec<_>>(),
        "tensions": tensions.iter().map(|t| json!({
            "id": display_id(EntityType::Tension, t.round, t.seq),
            "round": t.round,
            "label": t.label,
            "description": t.description,
            "contributors": t.contributors,
            "status": t.status.as_str(),
        })).collect::<Vec<_>>(),
        "open_tensions": tensions.iter()
            .filter(|t| matches!(t.status, alignment_db::TensionStatus::Open | alignment_db::TensionStatus::Reopened))
            .map(|t| display_id(EntityType::Tension, t.round, t.seq))
            .collect::<Vec<_>>(),
        "recommendations": recommendations.iter().map(|r| json!({
            "id": display_id(EntityType::Recommendation, r.round, r.seq),
            "round": r.round,
            "label": r.label,
            "content": r.content,
            "contributors": r.contributors,
            "status": r.status.as_str(),
            "parameters": r.parameters,
        })).collect::<Vec<_>>(),
        "evidence": evidence.iter().map(|e| json!({
            "id": display_id(EntityType::Evidence, e.round, e.seq),
            "round": e.round,
            "label": e.label,
            "content": e.content,
            "contributors": e.contributors,
            "status": e.status.as_str(),
        })).collect::<Vec<_>>(),
        "claims": claims.iter().map(|c| json!({
            "id": display_id(EntityType::Claim, c.round, c.seq),
            "round": c.round,
            "label": c.label,
            "content": c.content,
            "contributors": c.contributors,
            "status": c.status.as_str(),
        })).collect::<Vec<_>>(),
        "verdicts": verdicts.iter().map(|v| json!({
            "verdict_id": v.verdict_id,
            "verdict_type": v.verdict_type.as_str(),
            "round": v.round,
            "recommendation": v.recommendation,
            "description": v.description,
        })).collect::<Vec<_>>(),
    }))
}

/// Handle blue_dialogue_expert_create (RFC 0051)
///
/// Create a new expert mid-dialogue to address emerging needs.
pub fn handle_expert_create(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let dialogue_id = args
        .get("dialogue_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let expert_slug = args
        .get("expert_slug")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    if !is_valid_pastry_name(expert_slug) {
        return Err(HandlerError::CommandFailed(format!(
            "Expert slug '{}' is not a valid pastry name. Use names like Muffin, Cupcake, \
             Scone, etc. from the pastry name list.",
            expert_slug
        )));
    }

    let role = args
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let tier_str = args
        .get("tier")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let tier = DbExpertTier::from_str(tier_str);

    let focus = args.get("focus").and_then(|v| v.as_str());

    let creation_reason = args
        .get("creation_reason")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let first_round = args
        .get("first_round")
        .and_then(|v| v.as_i64())
        .ok_or(HandlerError::InvalidParams)? as i32;

    let conn = state.store.conn();

    // Verify dialogue exists
    let _dialogue = get_dialogue(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Dialogue not found: {}", e)))?;

    // Register expert with source=created
    register_expert(
        conn,
        dialogue_id,
        expert_slug,
        role,
        tier,
        DbExpertSource::Created,
        None, // description
        focus,
        None, // relevance
        Some(creation_reason),
        None, // color
        Some(first_round),
    )
    .map_err(|e| HandlerError::CommandFailed(format!("Failed to create expert: {}", e)))?;

    Ok(json!({
        "status": "success",
        "message": format!("Created expert '{}' ({}) for round {}", expert_slug, role, first_round),
        "expert": {
            "expert_slug": expert_slug,
            "role": role,
            "tier": tier.as_str(),
            "source": "created",
            "focus": focus,
            "creation_reason": creation_reason,
            "first_round": first_round,
        }
    }))
}

/// Validate round_register inputs (RFC 0051 Phase 2c)
///
/// Returns all validation errors, not just the first one.
fn validate_round_register_inputs(args: &Value) -> Vec<ValidationError> {
    let mut collector = ValidationCollector::new();

    // Validate perspectives
    if let Some(perspectives) = args.get("perspectives").and_then(|v| v.as_array()) {
        for (i, p) in perspectives.iter().enumerate() {
            let field_prefix = format!("perspectives[{}]", i);

            // Required fields
            if p.get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("label")
                        .with_field(format!("{}.label", field_prefix)),
                );
            }
            if p.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("content")
                        .with_field(format!("{}.content", field_prefix)),
                );
            }

            // Validate references
            if let Some(refs) = p.get("references").and_then(|v| v.as_array()) {
                for (j, r) in refs.iter().enumerate() {
                    let ref_field = format!("{}.references[{}]", field_prefix, j);

                    let ref_type_str = r.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let target = r.get("target").and_then(|v| v.as_str()).unwrap_or("");

                    // Validate ref type
                    if RefType::from_str(ref_type_str).is_none() && !ref_type_str.is_empty() {
                        collector.add(
                            ValidationError::invalid_ref_type(ref_type_str)
                                .with_field(format!("{}.type", ref_field)),
                        );
                    }

                    // Validate target ID format
                    if !target.is_empty() {
                        if let Err(e) = validate_display_id(target) {
                            collector.add(e.with_field(format!("{}.target", ref_field)));
                        } else if let (Some(ref_type), Some((target_type, _, _))) =
                            (RefType::from_str(ref_type_str), parse_display_id(target))
                        {
                            // Validate semantic constraints
                            if let Some(e) = validate_ref_semantics(
                                ref_type,
                                EntityType::Perspective,
                                target_type,
                            ) {
                                collector.add(e.with_field(ref_field));
                            }
                        }
                    }
                }
            }
        }
    }

    // Validate tensions
    if let Some(tensions) = args.get("tensions").and_then(|v| v.as_array()) {
        for (i, t) in tensions.iter().enumerate() {
            let field_prefix = format!("tensions[{}]", i);

            if t.get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("label")
                        .with_field(format!("{}.label", field_prefix)),
                );
            }
            if t.get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("description")
                        .with_field(format!("{}.description", field_prefix)),
                );
            }

            // Validate references
            if let Some(refs) = t.get("references").and_then(|v| v.as_array()) {
                for (j, r) in refs.iter().enumerate() {
                    let ref_field = format!("{}.references[{}]", field_prefix, j);
                    let ref_type_str = r.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let target = r.get("target").and_then(|v| v.as_str()).unwrap_or("");

                    if RefType::from_str(ref_type_str).is_none() && !ref_type_str.is_empty() {
                        collector.add(
                            ValidationError::invalid_ref_type(ref_type_str)
                                .with_field(format!("{}.type", ref_field)),
                        );
                    }

                    if !target.is_empty() {
                        if let Err(e) = validate_display_id(target) {
                            collector.add(e.with_field(format!("{}.target", ref_field)));
                        } else if let (Some(ref_type), Some((target_type, _, _))) =
                            (RefType::from_str(ref_type_str), parse_display_id(target))
                        {
                            if let Some(e) =
                                validate_ref_semantics(ref_type, EntityType::Tension, target_type)
                            {
                                collector.add(e.with_field(ref_field));
                            }
                        }
                    }
                }
            }
        }
    }

    // Validate recommendations
    if let Some(recommendations) = args.get("recommendations").and_then(|v| v.as_array()) {
        for (i, r) in recommendations.iter().enumerate() {
            let field_prefix = format!("recommendations[{}]", i);

            if r.get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("label")
                        .with_field(format!("{}.label", field_prefix)),
                );
            }
            if r.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("content")
                        .with_field(format!("{}.content", field_prefix)),
                );
            }

            // Validate references
            if let Some(refs) = r.get("references").and_then(|v| v.as_array()) {
                for (j, ref_obj) in refs.iter().enumerate() {
                    let ref_field = format!("{}.references[{}]", field_prefix, j);
                    let ref_type_str = ref_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let target = ref_obj.get("target").and_then(|v| v.as_str()).unwrap_or("");

                    if RefType::from_str(ref_type_str).is_none() && !ref_type_str.is_empty() {
                        collector.add(
                            ValidationError::invalid_ref_type(ref_type_str)
                                .with_field(format!("{}.type", ref_field)),
                        );
                    }

                    if !target.is_empty() {
                        if let Err(e) = validate_display_id(target) {
                            collector.add(e.with_field(format!("{}.target", ref_field)));
                        } else if let (Some(ref_type), Some((target_type, _, _))) =
                            (RefType::from_str(ref_type_str), parse_display_id(target))
                        {
                            if let Some(e) = validate_ref_semantics(
                                ref_type,
                                EntityType::Recommendation,
                                target_type,
                            ) {
                                collector.add(e.with_field(ref_field));
                            }
                        }
                    }
                }
            }
        }
    }

    // Validate evidence
    if let Some(evidence) = args.get("evidence").and_then(|v| v.as_array()) {
        for (i, e) in evidence.iter().enumerate() {
            let field_prefix = format!("evidence[{}]", i);

            if e.get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("label")
                        .with_field(format!("{}.label", field_prefix)),
                );
            }
            if e.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("content")
                        .with_field(format!("{}.content", field_prefix)),
                );
            }
        }
    }

    // Validate claims
    if let Some(claims) = args.get("claims").and_then(|v| v.as_array()) {
        for (i, c) in claims.iter().enumerate() {
            let field_prefix = format!("claims[{}]", i);

            if c.get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("label")
                        .with_field(format!("{}.label", field_prefix)),
                );
            }
            if c.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                collector.add(
                    ValidationError::missing_field("content")
                        .with_field(format!("{}.content", field_prefix)),
                );
            }
        }
    }

    // Validate tension updates
    if let Some(updates) = args.get("tension_updates").and_then(|v| v.as_array()) {
        for (i, u) in updates.iter().enumerate() {
            let field_prefix = format!("tension_updates[{}]", i);
            let tension_id = u.get("id").and_then(|v| v.as_str()).unwrap_or("");

            if tension_id.is_empty() {
                collector.add(
                    ValidationError::missing_field("id").with_field(format!("{}.id", field_prefix)),
                );
            } else if let Err(e) = validate_display_id(tension_id) {
                collector.add(e.with_field(format!("{}.id", field_prefix)));
            } else if let Some((entity_type, _, _)) = parse_display_id(tension_id) {
                if entity_type != EntityType::Tension {
                    collector.add(
                        ValidationError::type_id_mismatch("T", tension_id)
                            .with_field(format!("{}.id", field_prefix)),
                    );
                }
            }
        }
    }

    collector.into_errors()
}

/// Convert validation errors to a structured JSON response
fn validation_errors_to_json(errors: &[ValidationError]) -> Value {
    json!({
        "status": "error",
        "error_code": "validation_failed",
        "error_count": errors.len(),
        "message": format!("Validation failed with {} error(s)", errors.len()),
        "errors": errors.iter().map(|e| json!({
            "code": e.code.as_str(),
            "message": e.message,
            "field": e.field,
            "suggestion": e.suggestion,
            "context": e.context,
        })).collect::<Vec<_>>(),
    })
}

/// Handle blue_dialogue_round_register (RFC 0051)
///
/// Bulk register all round data in a single atomic call.
pub fn handle_round_register(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let dialogue_id = args
        .get("dialogue_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let round = args
        .get("round")
        .and_then(|v| v.as_i64())
        .ok_or(HandlerError::InvalidParams)? as i32;

    let score = args
        .get("score")
        .and_then(|v| v.as_i64())
        .ok_or(HandlerError::InvalidParams)? as i32;

    let summary = args.get("summary").and_then(|v| v.as_str());

    // RFC 0057: Parse score components (W/C/T/R breakdown)
    let score_components = args.get("score_components").map(|sc| ScoreComponents {
        wisdom: sc.get("wisdom").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        consistency: sc.get("consistency").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        truth: sc.get("truth").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        relationships: sc
            .get("relationships")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32,
    });

    // RFC 0057: Parse convergence metrics
    let convergence_metrics = {
        let open_tensions = args
            .get("open_tensions")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let new_perspectives = args
            .get("new_perspectives")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let converge_signals = args
            .get("converge_signals")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let panel_size = args.get("panel_size").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        if open_tensions > 0 || new_perspectives > 0 || converge_signals > 0 || panel_size > 0 {
            Some(ConvergenceMetrics {
                open_tensions,
                new_perspectives,
                converge_signals,
                panel_size,
            })
        } else {
            None
        }
    };

    // Phase 2c: Batch validation - collect ALL errors before registration
    let validation_errors = validate_round_register_inputs(args);
    if !validation_errors.is_empty() {
        return Ok(validation_errors_to_json(&validation_errors));
    }

    let conn = state.store.conn();

    // Verify dialogue exists
    let _dialogue = get_dialogue(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Dialogue not found: {}", e)))?;

    // Create round record with RFC 0057 metrics
    create_round_with_metrics(
        conn,
        dialogue_id,
        round,
        summary,
        score,
        score_components.as_ref(),
        convergence_metrics.as_ref(),
    )
    .map_err(|e| HandlerError::CommandFailed(format!("Failed to create round: {}", e)))?;

    let mut registered = json!({
        "perspectives": [],
        "tensions": [],
        "recommendations": [],
        "evidence": [],
        "claims": [],
        "refs": 0,
        "expert_scores": [],
    });

    // Register perspectives
    if let Some(perspectives) = args.get("perspectives").and_then(|v| v.as_array()) {
        let mut p_ids = Vec::new();
        for p in perspectives {
            let local_id = p.get("local_id").and_then(|v| v.as_str()).unwrap_or("");
            let label = p.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let content = p.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let contributors: Vec<String> = p
                .get("contributors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let global_id = register_perspective(
                conn,
                dialogue_id,
                round,
                label,
                content,
                &contributors,
                None,
            )
            .map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to register perspective: {}", e))
            })?;

            // Register refs for this perspective
            if let Some(refs) = p.get("references").and_then(|v| v.as_array()) {
                for r in refs {
                    let ref_type_str = r.get("type").and_then(|v| v.as_str()).unwrap_or("support");
                    let target = r.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    if let (Some(ref_type), Some((target_type, _, _))) =
                        (RefType::from_str(ref_type_str), parse_display_id(target))
                    {
                        let _ = register_ref(
                            conn,
                            dialogue_id,
                            EntityType::Perspective,
                            &global_id,
                            ref_type,
                            target_type,
                            target,
                        );
                    }
                }
            }

            p_ids.push(json!({ "local_id": local_id, "global_id": global_id }));
        }
        registered["perspectives"] = json!(p_ids);
    }

    // Register tensions
    if let Some(tensions) = args.get("tensions").and_then(|v| v.as_array()) {
        let mut t_ids = Vec::new();
        for t in tensions {
            let local_id = t.get("local_id").and_then(|v| v.as_str()).unwrap_or("");
            let label = t.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let description = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
            let contributors: Vec<String> = t
                .get("contributors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let global_id = register_tension(
                conn,
                dialogue_id,
                round,
                label,
                description,
                &contributors,
                None,
            )
            .map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to register tension: {}", e))
            })?;

            // Register refs for this tension
            if let Some(refs) = t.get("references").and_then(|v| v.as_array()) {
                for r in refs {
                    let ref_type_str = r.get("type").and_then(|v| v.as_str()).unwrap_or("support");
                    let target = r.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    if let (Some(ref_type), Some((target_type, _, _))) =
                        (RefType::from_str(ref_type_str), parse_display_id(target))
                    {
                        let _ = register_ref(
                            conn,
                            dialogue_id,
                            EntityType::Tension,
                            &global_id,
                            ref_type,
                            target_type,
                            target,
                        );
                    }
                }
            }

            t_ids.push(json!({ "local_id": local_id, "global_id": global_id }));
        }
        registered["tensions"] = json!(t_ids);
    }

    // Register recommendations
    if let Some(recommendations) = args.get("recommendations").and_then(|v| v.as_array()) {
        let mut r_ids = Vec::new();
        for r in recommendations {
            let local_id = r.get("local_id").and_then(|v| v.as_str()).unwrap_or("");
            let label = r.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let content = r.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let contributors: Vec<String> = r
                .get("contributors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let parameters = r.get("parameters").cloned();

            let global_id = register_recommendation(
                conn,
                dialogue_id,
                round,
                label,
                content,
                &contributors,
                parameters.as_ref(),
                None,
            )
            .map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to register recommendation: {}", e))
            })?;

            // Register refs
            if let Some(refs) = r.get("references").and_then(|v| v.as_array()) {
                for ref_obj in refs {
                    let ref_type_str = ref_obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("support");
                    let target = ref_obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    if let (Some(ref_type), Some((target_type, _, _))) =
                        (RefType::from_str(ref_type_str), parse_display_id(target))
                    {
                        let _ = register_ref(
                            conn,
                            dialogue_id,
                            EntityType::Recommendation,
                            &global_id,
                            ref_type,
                            target_type,
                            target,
                        );
                    }
                }
            }

            r_ids.push(json!({ "local_id": local_id, "global_id": global_id }));
        }
        registered["recommendations"] = json!(r_ids);
    }

    // Register evidence
    if let Some(evidence_arr) = args.get("evidence").and_then(|v| v.as_array()) {
        let mut e_ids = Vec::new();
        for e in evidence_arr {
            let local_id = e.get("local_id").and_then(|v| v.as_str()).unwrap_or("");
            let label = e.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let content = e.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let contributors: Vec<String> = e
                .get("contributors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let global_id = register_evidence(
                conn,
                dialogue_id,
                round,
                label,
                content,
                &contributors,
                None,
            )
            .map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to register evidence: {}", e))
            })?;

            // Register refs
            if let Some(refs) = e.get("references").and_then(|v| v.as_array()) {
                for ref_obj in refs {
                    let ref_type_str = ref_obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("support");
                    let target = ref_obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    if let (Some(ref_type), Some((target_type, _, _))) =
                        (RefType::from_str(ref_type_str), parse_display_id(target))
                    {
                        let _ = register_ref(
                            conn,
                            dialogue_id,
                            EntityType::Evidence,
                            &global_id,
                            ref_type,
                            target_type,
                            target,
                        );
                    }
                }
            }

            e_ids.push(json!({ "local_id": local_id, "global_id": global_id }));
        }
        registered["evidence"] = json!(e_ids);
    }

    // Register claims
    if let Some(claims_arr) = args.get("claims").and_then(|v| v.as_array()) {
        let mut c_ids = Vec::new();
        for c in claims_arr {
            let local_id = c.get("local_id").and_then(|v| v.as_str()).unwrap_or("");
            let label = c.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let content = c.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let contributors: Vec<String> = c
                .get("contributors")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let global_id = register_claim(
                conn,
                dialogue_id,
                round,
                label,
                content,
                &contributors,
                None,
            )
            .map_err(|e| HandlerError::CommandFailed(format!("Failed to register claim: {}", e)))?;

            // Register refs
            if let Some(refs) = c.get("references").and_then(|v| v.as_array()) {
                for ref_obj in refs {
                    let ref_type_str = ref_obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("support");
                    let target = ref_obj.get("target").and_then(|v| v.as_str()).unwrap_or("");
                    if let (Some(ref_type), Some((target_type, _, _))) =
                        (RefType::from_str(ref_type_str), parse_display_id(target))
                    {
                        let _ = register_ref(
                            conn,
                            dialogue_id,
                            EntityType::Claim,
                            &global_id,
                            ref_type,
                            target_type,
                            target,
                        );
                    }
                }
            }

            c_ids.push(json!({ "local_id": local_id, "global_id": global_id }));
        }
        registered["claims"] = json!(c_ids);
    }

    // Process tension updates
    if let Some(updates) = args.get("tension_updates").and_then(|v| v.as_array()) {
        for u in updates {
            let tension_id = u.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let status_str = u.get("status").and_then(|v| v.as_str()).unwrap_or("open");
            let actors: Vec<String> = u
                .get("by")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .or_else(|| {
                    u.get("by")
                        .and_then(|v| v.as_str())
                        .map(|s| vec![s.to_string()])
                })
                .unwrap_or_default();
            let via = u.get("via").and_then(|v| v.as_str());

            let status = alignment_db::TensionStatus::from_str(status_str);
            let _ =
                update_tension_status(conn, dialogue_id, tension_id, status, &actors, via, round);
        }
    }

    // Update expert scores
    if let Some(expert_scores) = args.get("expert_scores").and_then(|v| v.as_object()) {
        let mut updated_scores = Vec::new();
        for (expert_slug, score_val) in expert_scores {
            if let Some(expert_score) = score_val.as_i64() {
                let _ =
                    update_expert_score(conn, dialogue_id, expert_slug, round, expert_score as i32);
                updated_scores.push(json!({ "expert_slug": expert_slug, "score": expert_score }));
            }
        }
        registered["expert_scores"] = json!(updated_scores);
    }

    Ok(json!({
        "status": "success",
        "message": format!("Registered round {} data for dialogue '{}'", round, dialogue_id),
        "dialogue_id": dialogue_id,
        "round": round,
        "score": score,
        "registered": registered,
    }))
}

/// Handle blue_dialogue_verdict_register (RFC 0051)
///
/// Register a verdict (interim, final, minority, or dissent).
pub fn handle_verdict_register(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let dialogue_id = args
        .get("dialogue_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let verdict_id = args
        .get("verdict_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let verdict_type_str = args
        .get("verdict_type")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let verdict_type = VerdictType::from_str(verdict_type_str);

    let round = args
        .get("round")
        .and_then(|v| v.as_i64())
        .ok_or(HandlerError::InvalidParams)? as i32;

    let recommendation = args
        .get("recommendation")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    // Optional fields
    let conditions: Option<Vec<String>> =
        args.get("conditions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

    let vote = args.get("vote").and_then(|v| v.as_str()).map(String::from);
    let confidence = args
        .get("confidence")
        .and_then(|v| v.as_str())
        .map(String::from);
    let author_expert = args
        .get("author_expert")
        .and_then(|v| v.as_str())
        .map(String::from);

    let tensions_resolved: Option<Vec<String>> = args
        .get("tensions_resolved")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let tensions_accepted: Option<Vec<String>> = args
        .get("tensions_accepted")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let recommendations_adopted: Option<Vec<String>> = args
        .get("recommendations_adopted")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let supporting_experts: Option<Vec<String>> = args
        .get("supporting_experts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    // RFC 0057: Force flag allows convergence without meeting criteria (with warning)
    let force = coerce_bool(args.get("force").unwrap_or(&Value::Bool(false))).unwrap_or(false);

    let conn = state.store.conn();

    // Verify dialogue exists
    let _dialogue = get_dialogue(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Dialogue not found: {}", e)))?;

    // RFC 0057: Convergence validation for final verdicts
    let mut forced_warning: Option<String> = None;
    if verdict_type == VerdictType::Final {
        let (can_converge, blockers) = can_dialogue_converge(conn, dialogue_id).map_err(|e| {
            HandlerError::CommandFailed(format!("Failed to check convergence: {}", e))
        })?;

        if !can_converge {
            if force {
                // Allow with warning
                forced_warning = Some(format!(
                    "FORCED CONVERGENCE: Convergence criteria not met. Blockers: {}",
                    blockers.join("; ")
                ));
            } else {
                // Block registration with structured error
                let scoreboard = get_scoreboard(conn, dialogue_id).ok();
                let last_round = scoreboard.as_ref().and_then(|s| s.last());

                return Ok(json!({
                    "status": "error",
                    "error_type": "convergence_blocked",
                    "message": format!("Cannot register final verdict: convergence criteria not met"),
                    "blockers": blockers,
                    "current_state": {
                        "velocity": last_round.map(|r| r.velocity).unwrap_or(-1),
                        "open_tensions": last_round.map(|r| r.open_tensions).unwrap_or(0),
                        "new_perspectives": last_round.map(|r| r.new_perspectives).unwrap_or(0),
                        "converge_percent": last_round.map(|r| r.converge_percent).unwrap_or(0.0),
                        "converge_signals": last_round.map(|r| r.converge_signals).unwrap_or(0),
                        "panel_size": last_round.map(|r| r.panel_size).unwrap_or(0),
                    },
                    "suggestion": "Either resolve remaining tensions/perspectives and get 100% expert convergence, or use force=true to override (not recommended).",
                    "hint": "Use blue_dialogue_round_context to see detailed convergence status"
                }));
            }
        }
    }

    let verdict = Verdict {
        dialogue_id: dialogue_id.to_string(),
        verdict_id: verdict_id.to_string(),
        verdict_type,
        round,
        author_expert,
        recommendation: recommendation.to_string(),
        description: description.to_string(),
        conditions,
        vote,
        confidence,
        tensions_resolved,
        tensions_accepted,
        recommendations_adopted,
        key_evidence: None,
        key_claims: None,
        supporting_experts,
        ethos_compliance: None,
        created_at: chrono::Utc::now(),
    };

    register_verdict(conn, &verdict)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to register verdict: {}", e)))?;

    // RFC 0057: Include warning if convergence was forced
    let mut response = json!({
        "status": "success",
        "message": format!("Registered {} verdict '{}' for dialogue '{}'", verdict_type.as_str(), verdict_id, dialogue_id),
        "dialogue_id": dialogue_id,
        "verdict_id": verdict_id,
        "verdict_type": verdict_type.as_str(),
        "round": round,
    });

    if let Some(warning) = forced_warning {
        response["warning"] = json!(warning);
        response["forced"] = json!(true);
    }

    Ok(response)
}

/// Handle blue_dialogue_export (RFC 0051)
///
/// Export dialogue to JSON with full provenance from database.
pub fn handle_export(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let dialogue_id = args
        .get("dialogue_id")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let output_path = args.get("output_path").and_then(|v| v.as_str());

    let conn = state.store.conn();

    // Get dialogue
    let dialogue = get_dialogue(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Dialogue not found: {}", e)))?;

    // Get all data
    let experts = get_experts(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get experts: {}", e)))?;

    let perspectives = get_perspectives(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get perspectives: {}", e)))?;

    let tensions = get_tensions(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get tensions: {}", e)))?;

    let recommendations = get_recommendations(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get recommendations: {}", e)))?;

    let evidence = get_evidence(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get evidence: {}", e)))?;

    let claims = get_claims(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get claims: {}", e)))?;

    let verdicts = get_verdicts(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get verdicts: {}", e)))?;

    // RFC 0057: Get scoreboard with full metrics
    let scoreboard = get_scoreboard(conn, dialogue_id)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get scoreboard: {}", e)))?;

    // Build export structure
    let export_data = json!({
        "dialogue": {
            "dialogue_id": dialogue.dialogue_id,
            "title": dialogue.title,
            "question": dialogue.question,
            "status": dialogue.status.as_str(),
            "created_at": dialogue.created_at.to_rfc3339(),
            "converged_at": dialogue.converged_at.map(|dt| dt.to_rfc3339()),
            "total_rounds": dialogue.total_rounds,
            "total_alignment": dialogue.total_alignment,
            "output_dir": dialogue.output_dir,
            "calibrated": dialogue.calibrated,
            "background": dialogue.background,
        },
        "experts": experts.iter().map(|e| json!({
            "expert_slug": e.expert_slug,
            "role": e.role,
            "description": e.description,
            "focus": e.focus,
            "tier": e.tier.as_str(),
            "source": e.source.as_str(),
            "relevance": e.relevance,
            "creation_reason": e.creation_reason,
            "scores": e.scores,
            "total_score": e.total_score,
            "first_round": e.first_round,
            "created_at": e.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "perspectives": perspectives.iter().map(|p| json!({
            "id": display_id(EntityType::Perspective, p.round, p.seq),
            "round": p.round,
            "seq": p.seq,
            "label": p.label,
            "content": p.content,
            "contributors": p.contributors,
            "status": p.status.as_str(),
            "refs": p.refs,
            "created_at": p.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "tensions": tensions.iter().map(|t| json!({
            "id": display_id(EntityType::Tension, t.round, t.seq),
            "round": t.round,
            "seq": t.seq,
            "label": t.label,
            "description": t.description,
            "contributors": t.contributors,
            "status": t.status.as_str(),
            "refs": t.refs,
            "created_at": t.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "recommendations": recommendations.iter().map(|r| json!({
            "id": display_id(EntityType::Recommendation, r.round, r.seq),
            "round": r.round,
            "seq": r.seq,
            "label": r.label,
            "content": r.content,
            "contributors": r.contributors,
            "parameters": r.parameters,
            "status": r.status.as_str(),
            "refs": r.refs,
            "adopted_in_verdict": r.adopted_in_verdict,
            "created_at": r.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "evidence": evidence.iter().map(|e| json!({
            "id": display_id(EntityType::Evidence, e.round, e.seq),
            "round": e.round,
            "seq": e.seq,
            "label": e.label,
            "content": e.content,
            "contributors": e.contributors,
            "status": e.status.as_str(),
            "refs": e.refs,
            "created_at": e.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "claims": claims.iter().map(|c| json!({
            "id": display_id(EntityType::Claim, c.round, c.seq),
            "round": c.round,
            "seq": c.seq,
            "label": c.label,
            "content": c.content,
            "contributors": c.contributors,
            "status": c.status.as_str(),
            "refs": c.refs,
            "created_at": c.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        "verdicts": verdicts.iter().map(|v| json!({
            "verdict_id": v.verdict_id,
            "verdict_type": v.verdict_type.as_str(),
            "round": v.round,
            "author_expert": v.author_expert,
            "recommendation": v.recommendation,
            "description": v.description,
            "conditions": v.conditions,
            "vote": v.vote,
            "confidence": v.confidence,
            "tensions_resolved": v.tensions_resolved,
            "tensions_accepted": v.tensions_accepted,
            "recommendations_adopted": v.recommendations_adopted,
            "key_evidence": v.key_evidence,
            "key_claims": v.key_claims,
            "supporting_experts": v.supporting_experts,
            "created_at": v.created_at.to_rfc3339(),
        })).collect::<Vec<_>>(),
        // RFC 0057: Full scoreboard with velocity and convergence tracking
        "scoreboard": scoreboard.iter().map(|r| json!({
            "round": r.round,
            "score": {
                "W": r.w,
                "C": r.c,
                "T": r.t,
                "R": r.r,
                "total": r.total,
            },
            "velocity": r.velocity,
            "open_tensions": r.open_tensions,
            "new_perspectives": r.new_perspectives,
            "converge_signals": r.converge_signals,
            "panel_size": r.panel_size,
            "converge_percent": r.converge_percent,
            "cumulative": {
                "score": r.cumulative_score,
                "W": r.cumulative_w,
                "C": r.cumulative_c,
                "T": r.cumulative_t,
                "R": r.cumulative_r,
            },
        })).collect::<Vec<_>>(),
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });

    // Determine output path
    let final_path = match output_path {
        Some(p) => p.to_string(),
        None => {
            let dir = dialogue
                .output_dir
                .as_deref()
                .unwrap_or("/tmp/blue-dialogue");
            format!("{}/{}/dialogue.json", dir, dialogue_id)
        }
    };

    // Ensure parent directory exists
    if let Some(parent) = Path::new(&final_path).parent() {
        fs::create_dir_all(parent).map_err(|e| {
            HandlerError::CommandFailed(format!("Failed to create output directory: {}", e))
        })?;
    }

    // Write JSON file
    let json_str = serde_json::to_string_pretty(&export_data).map_err(|e| {
        HandlerError::CommandFailed(format!("Failed to serialize export data: {}", e))
    })?;

    fs::write(&final_path, &json_str)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to write export file: {}", e)))?;

    Ok(json!({
        "status": "success",
        "message": format!("Exported dialogue '{}' to {}", dialogue_id, final_path),
        "dialogue_id": dialogue_id,
        "output_path": final_path,
        "stats": {
            "experts": experts.len(),
            "perspectives": perspectives.len(),
            "tensions": tensions.len(),
            "recommendations": recommendations.len(),
            "evidence": evidence.len(),
            "claims": claims.len(),
            "verdicts": verdicts.len(),
        }
    }))
}

/// Handle blue_dialogue_round_verify
///
/// Verifies that all expected files exist for a completed round.
pub fn handle_round_verify(args: &Value) -> Result<Value, HandlerError> {
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let round = args
        .get("round")
        .and_then(|v| v.as_u64())
        .ok_or(HandlerError::InvalidParams)? as usize;
    let agents: Vec<String> = args
        .get("agents")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .ok_or(HandlerError::InvalidParams)?;

    let mut present = Vec::new();
    let mut missing = Vec::new();

    // Check round directory
    let round_dir = format!("{}/round-{}", output_dir, round);
    if !Path::new(&round_dir).is_dir() {
        missing.push(format!("round-{}/", round));
    }

    // Check agent files
    for agent in &agents {
        let agent_file = format!("{}/{}.md", round_dir, agent.to_lowercase());
        if Path::new(&agent_file).exists() {
            present.push(agent_file);
        } else {
            missing.push(agent_file);
        }
    }

    // Check judge artifacts
    let scoreboard = format!("{}/scoreboard.md", output_dir);
    if Path::new(&scoreboard).exists() {
        present.push(scoreboard);
    } else {
        missing.push(scoreboard);
    }

    let tensions = format!("{}/tensions.md", output_dir);
    if Path::new(&tensions).exists() {
        present.push(tensions);
    } else {
        missing.push(tensions);
    }

    let summary = format!("{}/round-{}.summary.md", output_dir, round);
    if Path::new(&summary).exists() {
        present.push(summary);
    } else {
        missing.push(summary);
    }

    let status = if missing.is_empty() {
        "ok"
    } else {
        "missing_files"
    };

    Ok(json!({
        "status": status,
        "message": if missing.is_empty() {
            format!("All {} files present for round {}", present.len(), round)
        } else {
            format!("{} files missing for round {}: {}", missing.len(), round, missing.join(", "))
        },
        "present": present,
        "missing": missing,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jq_check() {
        // Just verify this doesn't panic
        let _ = jq_available();
    }

    #[test]
    fn test_title_to_slug() {
        assert_eq!(
            title_to_slug("RFC Implementation Discussion"),
            "rfc-implementation-discussion"
        );
        assert_eq!(title_to_slug("quick-chat"), "quick-chat");
    }

    #[test]
    fn test_dialogue_markdown_generation() {
        let md = generate_dialogue_markdown(
            "test-dialogue",
            1,
            Some("test-rfc"),
            Some("A test summary"),
            Some("Some dialogue content"),
        );
        assert!(md.contains("# Dialogue 0001: Test Dialogue"));
        assert!(md.contains("| **RFC** | test-rfc |"));
        assert!(md.contains("A test summary"));
        assert!(md.contains("Some dialogue content"));
    }

    // ==================== Alignment Mode Tests ====================

    /// Helper: create a test pool with the specified number of experts
    fn test_pool(n: usize) -> ExpertPool {
        let mut experts = Vec::new();
        let base_roles = [
            ("Systems Architect", ExpertTier::Core),
            ("Security Engineer", ExpertTier::Core),
            ("API Designer", ExpertTier::Core),
            ("Data Architect", ExpertTier::Adjacent),
            ("Quality Engineer", ExpertTier::Adjacent),
            ("UX Architect", ExpertTier::Adjacent),
            ("DevOps Engineer", ExpertTier::Adjacent),
            ("Performance Engineer", ExpertTier::Wildcard),
            ("Technical Writer", ExpertTier::Wildcard),
            ("Risk Analyst", ExpertTier::Wildcard),
        ];
        for i in 0..n {
            let (base_role, tier) = base_roles[i % base_roles.len()];
            // Make roles unique by adding a suffix for overflow
            let role = if i < base_roles.len() {
                base_role.to_string()
            } else {
                format!("{} {}", base_role, i / base_roles.len() + 1)
            };
            let relevance = match tier {
                ExpertTier::Core => 0.95 - (i as f64 * 0.02),
                ExpertTier::Adjacent => 0.70 - (i as f64 * 0.02),
                ExpertTier::Wildcard => 0.40 - (i as f64 * 0.02),
            };
            experts.push(PoolExpert {
                role,
                tier,
                relevance: relevance.max(0.20),
            });
        }
        ExpertPool {
            domain: "Test Domain".to_string(),
            question: Some("Test question?".to_string()),
            experts,
        }
    }

    /// Helper: create test agents from a pool
    fn test_agents(n: usize) -> Vec<PastryAgent> {
        let pool = test_pool(n.max(10));
        let sampled = sample_panel_from_pool(&pool, n);
        assign_pastry_names(sampled)
    }

    #[test]
    fn test_assign_pastry_names() {
        let agents = test_agents(3);
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].name, "Muffin");
        assert_eq!(agents[1].name, "Cupcake");
        assert_eq!(agents[2].name, "Scone");
        for agent in &agents {
            assert_eq!(agent.emoji, "🧁");
            assert!(!agent.role.is_empty());
        }
    }

    #[test]
    fn test_assign_pastry_names_overflow() {
        // Create a pool with 25 experts
        let pool = test_pool(25);
        let sampled = sample_panel_from_pool(&pool, 25);
        let agents = assign_pastry_names(sampled);
        assert_eq!(agents.len(), 25);
        // First 20 use named pastries
        assert_eq!(agents[0].name, "Muffin");
        assert_eq!(agents[19].name, "Religieuse");
        // Overflow agents get numbered names
        assert_eq!(agents[20].name, "Pastry21");
        assert_eq!(agents[24].name, "Pastry25");
    }

    #[test]
    fn test_sample_panel_from_pool() {
        let pool = test_pool(15);
        let sampled = sample_panel_from_pool(&pool, 7);
        assert_eq!(sampled.len(), 7);
        // All sampled experts should have valid roles
        for expert in &sampled {
            assert!(!expert.role.is_empty());
            assert!(expert.relevance > 0.0);
        }
    }

    #[test]
    fn test_weighted_sample_respects_size() {
        let experts = vec![
            PoolExpert {
                role: "A".to_string(),
                tier: ExpertTier::Core,
                relevance: 0.9,
            },
            PoolExpert {
                role: "B".to_string(),
                tier: ExpertTier::Core,
                relevance: 0.8,
            },
            PoolExpert {
                role: "C".to_string(),
                tier: ExpertTier::Core,
                relevance: 0.7,
            },
        ];
        // Request more than available
        let sampled = weighted_sample(&experts, 5);
        assert_eq!(sampled.len(), 3); // Should return all available

        // Request fewer than available
        let sampled = weighted_sample(&experts, 2);
        assert_eq!(sampled.len(), 2);
    }

    #[test]
    fn test_alignment_dialogue_markdown() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let md = generate_alignment_dialogue_markdown(
            "test-alignment",
            1,
            Some("test-rfc"),
            &agents,
            Some(&pool),
        );

        // Required sections
        assert!(md.contains("# Alignment Dialogue:"));
        assert!(md.contains("## Expert Panel"));
        assert!(md.contains("## Alignment Scoreboard"));
        assert!(md.contains("## Perspectives Inventory"));
        assert!(md.contains("## Tensions Tracker"));
        assert!(md.contains("## Round 0: Opening Arguments"));

        // Agent names present
        assert!(md.contains("Muffin"));
        assert!(md.contains("Cupcake"));
        assert!(md.contains("Scone"));

        // Scoreboard structure
        assert!(md.contains("| Wisdom | Consistency | Truth | Relationships |"));
        assert!(md.contains("**Total ALIGNMENT**: 0"));

        // Metadata
        assert!(md.contains("**Draft**: Dialogue 0001"));
        assert!(md.contains("**Status**: In Progress"));
        assert!(md.contains("**RFC**: test-rfc"));
        assert!(md.contains("💙 Judge"));

        // RFC 0048: Expert Pool section
        assert!(md.contains("## Expert Pool"));
        assert!(md.contains("**Domain**: Test Domain"));
    }

    #[test]
    fn test_build_judge_protocol() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &["/tmp/source.rs".to_string()],
            "/tmp/blue-dialogue/system-design",
            Some(&pool),
            RotationMode::None,
        );

        // Must have instructions
        let instructions = protocol.get("instructions").unwrap().as_str().unwrap();
        assert!(instructions.contains("general-purpose"));
        assert!(instructions.contains("ALIGNMENT"));
        assert!(instructions.contains("Wisdom"));
        assert!(instructions.contains("convergence"));
        // RFC 0029: file-based output instructions
        assert!(instructions.contains("/tmp/blue-dialogue/system-design"));
        assert!(instructions.contains("mkdir"));
        assert!(instructions.contains("Read tool"));
        // RFC 0033: mandatory artifact writing with explicit paths
        assert!(instructions.contains("WRITE ARTIFACTS — THIS IS MANDATORY"));
        assert!(instructions.contains("scoreboard.md"));
        assert!(instructions.contains("tensions.md"));
        assert!(instructions.contains("round-N.summary.md"));
        // RFC 0033: archival record differentiation
        assert!(instructions.contains("ARCHIVAL RECORD"));
        // RFC 0033: round-specific context instructions
        assert!(instructions.contains("CONTEXT_INSTRUCTIONS"));
        assert!(instructions.contains("READ CONTEXT"));

        // Must have agent prompt template with Read tool reference
        let template = protocol
            .get("agent_prompt_template")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(template.contains("{{NAME}}"));
        assert!(template.contains("{{ROLE}}"));
        assert!(template.contains("PERSPECTIVE"));
        assert!(template.contains("TENSION"));
        assert!(template.contains("Read tool"));
        // RFC 0029: MANDATORY FILE OUTPUT section
        assert!(template.contains("MANDATORY FILE OUTPUT"));
        assert!(template.contains("{{OUTPUT_FILE}}"));
        // RFC 0033: CONTEXT_INSTRUCTIONS placeholder for round 1+ context
        assert!(template.contains("{{CONTEXT_INSTRUCTIONS}}"));

        // Must have agents array with name_lowercase
        let agents_arr = protocol.get("agents").unwrap().as_array().unwrap();
        assert_eq!(agents_arr.len(), 3);
        assert_eq!(agents_arr[0]["name"], "Muffin");
        assert_eq!(agents_arr[0]["name_lowercase"], "muffin");

        // Must have model
        assert_eq!(protocol["model"], "sonnet");

        // Must have sources
        let sources = protocol["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0], "/tmp/source.rs");

        // Must have output_dir
        assert_eq!(protocol["output_dir"], "/tmp/blue-dialogue/system-design");

        // Must have convergence params (RFC 0057)
        assert_eq!(protocol["convergence"]["max_rounds"], 10);
        assert!(protocol["convergence"]["tension_resolution_gate"]
            .as_bool()
            .unwrap());
        assert!(protocol["convergence"]["velocity_formula"]
            .as_str()
            .is_some());
        assert!(protocol["convergence"]["convergence_formula"]
            .as_str()
            .is_some());
    }

    #[test]
    fn test_build_judge_protocol_no_sources() {
        let agents = test_agents(2);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "haiku",
            &[],
            "/tmp/blue-dialogue/quick-topic",
            None,
            RotationMode::None,
        );

        // Template should NOT contain grounding instructions when no sources
        let template = protocol
            .get("agent_prompt_template")
            .unwrap()
            .as_str()
            .unwrap();
        assert!(!template.contains("GROUNDING"));
    }

    #[test]
    fn test_build_judge_protocol_output_paths() {
        let agents = test_agents(4);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/api-design",
            None,
            RotationMode::None,
        );

        // output_dir in JSON
        assert_eq!(protocol["output_dir"], "/tmp/blue-dialogue/api-design");

        // All agents have name_lowercase
        let agents_arr = protocol["agents"].as_array().unwrap();
        assert_eq!(agents_arr[0]["name_lowercase"], "muffin");
        assert_eq!(agents_arr[1]["name_lowercase"], "cupcake");
        assert_eq!(agents_arr[2]["name_lowercase"], "scone");
        assert_eq!(agents_arr[3]["name_lowercase"], "eclair");

        // MANDATORY FILE OUTPUT in template
        let template = protocol["agent_prompt_template"].as_str().unwrap();
        assert!(template.contains("MANDATORY FILE OUTPUT"));
        assert!(template.contains("{{OUTPUT_FILE}}"));
        assert!(template.contains("Write tool"));

        // output_dir referenced in instructions
        let instructions = protocol["instructions"].as_str().unwrap();
        assert!(instructions.contains("/tmp/blue-dialogue/api-design"));
        assert!(instructions.contains("OUTPUT DIR:"));
    }

    #[test]
    fn test_judge_protocol_artifact_write_instructions() {
        let agents = test_agents(3);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/test-artifacts",
            None,
            RotationMode::None,
        );

        let instructions = protocol["instructions"].as_str().unwrap();

        // Step 5 must explicitly reference Write tool for each artifact
        assert!(
            instructions.contains("Write /tmp/blue-dialogue/test-artifacts/scoreboard.md"),
            "Protocol must instruct Judge to write scoreboard.md with exact path"
        );
        assert!(
            instructions.contains("Write /tmp/blue-dialogue/test-artifacts/tensions.md"),
            "Protocol must instruct Judge to write tensions.md with exact path"
        );
        assert!(
            instructions.contains("Write /tmp/blue-dialogue/test-artifacts/round-N.summary.md"),
            "Protocol must instruct Judge to write round-N.summary.md with exact path"
        );

        // Artifact writing must come BEFORE updating dialogue file (step 5 before step 6)
        let artifact_pos = instructions.find("WRITE ARTIFACTS").unwrap();
        let archival_pos = instructions.find("UPDATE ARCHIVAL RECORD").unwrap();
        assert!(
            artifact_pos < archival_pos,
            "Artifacts must be written BEFORE updating archival record"
        );

        // Must explicitly say to use Write tool
        assert!(
            instructions.contains("Use the Write tool for EACH file"),
            "Protocol must tell Judge to use Write tool"
        );

        // Must warn about consequences of skipping
        assert!(
            instructions.contains("agents have NO context next round"),
            "Protocol must warn about consequences of skipping artifact writes"
        );
    }

    #[test]
    fn test_judge_protocol_context_references_artifacts() {
        let agents = test_agents(3);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/context-test",
            None,
            RotationMode::None,
        );

        let instructions = protocol["instructions"].as_str().unwrap();

        // Instructions must mention blue_dialogue_round_prompt handles context
        assert!(
            instructions.contains("blue_dialogue_round_prompt handles round-specific context"),
            "Instructions must mention blue_dialogue_round_prompt for context"
        );

        // File architecture diagram must show all artifact files
        assert!(
            instructions.contains("scoreboard.md"),
            "File architecture must show scoreboard.md"
        );
        assert!(
            instructions.contains("tensions.md"),
            "File architecture must show tensions.md"
        );
        assert!(
            instructions.contains("round-0.summary.md"),
            "File architecture must show round-0.summary.md"
        );
    }

    #[test]
    fn test_handle_round_prompt_round_0() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Muffin",
            "agent_emoji": "🧁",
            "agent_role": "Platform Architect",
            "round": 0
        });

        let result = handle_round_prompt(&args).unwrap();

        // Must have success status
        assert_eq!(result["status"], "success");

        // Must have fully-substituted output_file path
        assert_eq!(
            result["output_file"],
            "/tmp/blue-dialogue/test-topic/round-0/muffin.md"
        );

        // Prompt must contain substituted values (no placeholders)
        let prompt = result["prompt"].as_str().unwrap();
        assert!(prompt.contains("You are Muffin 🧁"));
        assert!(prompt.contains("Platform Architect"));
        assert!(prompt.contains("/tmp/blue-dialogue/test-topic/round-0/muffin.md"));
        assert!(!prompt.contains("{{NAME}}"));
        assert!(!prompt.contains("{{EMOJI}}"));
        assert!(!prompt.contains("{{OUTPUT_FILE}}"));

        // Round 0 should NOT have context instructions
        assert!(!prompt.contains("READ CONTEXT"));

        // Must have task_params for spawning
        assert_eq!(result["task_params"]["subagent_type"], "general-purpose");
        // No max_turns - agents run until complete
        assert!(result["task_params"].get("max_turns").is_none());
    }

    #[test]
    fn test_handle_round_prompt_round_1_has_context() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Cupcake",
            "agent_emoji": "🧁",
            "agent_role": "Security Engineer",
            "round": 1
        });

        let result = handle_round_prompt(&args).unwrap();
        let prompt = result["prompt"].as_str().unwrap();

        // Round 1+ should have context instructions
        assert!(prompt.contains("READ CONTEXT"));
        assert!(prompt.contains("/tmp/blue-dialogue/test-topic/tensions.md"));
        assert!(prompt.contains("/tmp/blue-dialogue/test-topic/round-0.summary.md"));
        assert!(prompt.contains("/tmp/blue-dialogue/test-topic/round-0/"));

        // Output file should be round-1
        assert_eq!(
            result["output_file"],
            "/tmp/blue-dialogue/test-topic/round-1/cupcake.md"
        );
    }

    #[test]
    fn test_handle_round_prompt_with_sources() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Scone",
            "agent_emoji": "🧁",
            "agent_role": "Backend Engineer",
            "round": 0,
            "sources": ["/path/to/file1.rs", "/path/to/file2.rs"]
        });

        let result = handle_round_prompt(&args).unwrap();
        let prompt = result["prompt"].as_str().unwrap();

        // Should have grounding instructions
        assert!(prompt.contains("GROUNDING:"));
        assert!(prompt.contains("/path/to/file1.rs"));
        assert!(prompt.contains("/path/to/file2.rs"));
    }

    // ==================== RFC 0050: Graduated Panel Rotation Tests ====================

    #[test]
    fn test_rotation_mode_graduated() {
        // Verify graduated mode parses correctly
        let mode: RotationMode = serde_json::from_str(r#""graduated""#).unwrap();
        assert_eq!(mode, RotationMode::Graduated);
    }

    #[test]
    fn test_rotation_mode_default_is_graduated() {
        // RFC 0050: graduated is now the default rotation mode
        let mode = RotationMode::default();
        assert_eq!(mode, RotationMode::Graduated);
    }

    #[test]
    fn test_expert_source_serialization() {
        let retained: ExpertSource = serde_json::from_str(r#""retained""#).unwrap();
        assert_eq!(retained, ExpertSource::Retained);

        let pool: ExpertSource = serde_json::from_str(r#""pool""#).unwrap();
        assert_eq!(pool, ExpertSource::Pool);

        let created: ExpertSource = serde_json::from_str(r#""created""#).unwrap();
        assert_eq!(created, ExpertSource::Created);
    }

    #[test]
    fn test_panel_expert_spec_parsing() {
        let spec_json = r#"{
            "name": "Muffin",
            "role": "Value Analyst",
            "source": "retained"
        }"#;
        let spec: PanelExpertSpec = serde_json::from_str(spec_json).unwrap();
        assert_eq!(spec.name, "Muffin");
        assert_eq!(spec.role, "Value Analyst");
        assert_eq!(spec.source, ExpertSource::Retained);
        assert!(spec.tier.is_none());
        assert!(spec.focus.is_none());
    }

    #[test]
    fn test_panel_expert_spec_with_focus() {
        let spec_json = r#"{
            "name": "Palmier",
            "role": "Supply Chain Analyst",
            "source": "created",
            "tier": "Adjacent",
            "focus": "Geographic concentration risk"
        }"#;
        let spec: PanelExpertSpec = serde_json::from_str(spec_json).unwrap();
        assert_eq!(spec.name, "Palmier");
        assert_eq!(spec.source, ExpertSource::Created);
        assert_eq!(spec.tier, Some("Adjacent".to_string()));
        assert_eq!(
            spec.focus,
            Some("Geographic concentration risk".to_string())
        );
    }

    #[test]
    fn test_pastry_agent_with_focus() {
        let agent = PastryAgent {
            name: "Palmier".to_string(),
            role: "Supply Chain Analyst".to_string(),
            emoji: "🧁".to_string(),
            tier: "Adjacent".to_string(),
            relevance: 0.75,
            focus: Some("Geographic concentration risk".to_string()),
        };
        let json = serde_json::to_string(&agent).unwrap();
        assert!(json.contains("focus"));
        assert!(json.contains("Geographic concentration"));
    }

    #[test]
    fn test_handle_round_prompt_with_expert_source() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Scone",
            "agent_emoji": "🧁",
            "agent_role": "Data Center Specialist",
            "round": 1,
            "expert_source": "pool"
        });

        let result = handle_round_prompt(&args).unwrap();

        // Should include expert_source in response
        assert_eq!(result["expert_source"], "pool");
        assert_eq!(result["has_context_brief"], true);
    }

    #[test]
    fn test_handle_round_prompt_retained_no_context_brief() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Muffin",
            "agent_emoji": "🧁",
            "agent_role": "Value Analyst",
            "round": 1,
            "expert_source": "retained"
        });

        let result = handle_round_prompt(&args).unwrap();

        // Retained experts should NOT have context brief marker
        assert_eq!(result["expert_source"], "retained");
        assert!(result.get("has_context_brief").is_none());
    }

    #[test]
    fn test_handle_round_prompt_with_focus() {
        let args = json!({
            "output_dir": "/tmp/blue-dialogue/test-topic",
            "agent_name": "Palmier",
            "agent_emoji": "🧁",
            "agent_role": "Supply Chain Analyst",
            "round": 1,
            "expert_source": "created",
            "focus": "Geographic concentration risk"
        });

        let result = handle_round_prompt(&args).unwrap();
        let prompt = result["prompt"].as_str().unwrap();

        // Created experts should have focus in prompt
        assert!(prompt.contains("Your Focus"));
        assert!(prompt.contains("Geographic concentration risk"));
        assert_eq!(result["focus"], "Geographic concentration risk");
    }

    #[test]
    fn test_build_judge_protocol_graduated_mode() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/graduated-test",
            Some(&pool),
            RotationMode::Graduated,
        );

        let instructions = protocol.get("instructions").unwrap().as_str().unwrap();

        // Must have graduated rotation guidelines
        assert!(instructions.contains("GRADUATED PANEL ROTATION"));
        assert!(instructions.contains("RFC 0050"));
        assert!(instructions.contains("blue_dialogue_evolve_panel"));
        assert!(instructions.contains("Retention Criteria"));
        assert!(instructions.contains("Fresh Perspective Triggers"));
        assert!(instructions.contains("Expert Creation"));
        assert!(instructions.contains(r#""source": "retained""#));
        assert!(instructions.contains(r#""source": "pool""#));
        assert!(instructions.contains(r#""source": "created""#));

        // Must tell Judge they can override Round 0
        assert!(instructions.contains("suggestion"));
        assert!(instructions.contains("Before Round 0"));
        assert!(instructions.contains("round=0"));
    }

    #[test]
    fn test_build_judge_protocol_graduated_uses_suggested_panel() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/suggested-test",
            Some(&pool),
            RotationMode::Graduated,
        );

        // Graduated mode uses suggested_panel, not agents
        assert!(protocol.get("suggested_panel").is_some());
        assert!(protocol.get("agents").is_none());
        assert_eq!(protocol["panel_is_suggestion"], true);

        // Verify suggested_panel has the right structure
        let suggested = protocol["suggested_panel"].as_array().unwrap();
        assert_eq!(suggested.len(), 3);
        assert_eq!(suggested[0]["name"], "Muffin");
    }

    #[test]
    fn test_build_judge_protocol_non_graduated_uses_agents() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/non-graduated-test",
            Some(&pool),
            RotationMode::None,
        );

        // Non-graduated modes use agents, not suggested_panel
        assert!(protocol.get("agents").is_some());
        assert!(protocol.get("suggested_panel").is_none());
        assert!(protocol.get("panel_is_suggestion").is_none());
    }

    #[test]
    fn test_build_judge_protocol_non_graduated_no_extra_instructions() {
        let agents = test_agents(3);
        let pool = test_pool(10);
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/none-test",
            Some(&pool),
            RotationMode::None,
        );

        let instructions = protocol.get("instructions").unwrap().as_str().unwrap();

        // Should NOT have graduated rotation guidelines
        assert!(!instructions.contains("GRADUATED PANEL ROTATION"));
        assert!(!instructions.contains("blue_dialogue_evolve_panel"));
    }

    #[test]
    fn test_panel_history_serialization() {
        let history = PanelHistory {
            round: 1,
            panel_size: 12,
            retained_count: 7,
            from_pool_count: 4,
            created_count: 1,
            experts: vec![
                PanelExpertSpec {
                    name: "Muffin".to_string(),
                    role: "Value Analyst".to_string(),
                    source: ExpertSource::Retained,
                    tier: None,
                    focus: None,
                },
                PanelExpertSpec {
                    name: "Palmier".to_string(),
                    role: "Supply Chain Analyst".to_string(),
                    source: ExpertSource::Created,
                    tier: Some("Adjacent".to_string()),
                    focus: Some("Geographic concentration".to_string()),
                },
            ],
        };

        let json = serde_json::to_string_pretty(&history).unwrap();
        assert!(json.contains("retained_count"));
        assert!(json.contains("from_pool_count"));
        assert!(json.contains("created_count"));
        assert!(json.contains("Geographic concentration"));
    }

    #[test]
    fn test_handle_evolve_panel_integration() {
        use std::fs;

        // Create a temp directory for testing
        let test_dir = "/tmp/blue-dialogue/evolve-panel-test";
        fs::create_dir_all(test_dir).unwrap();

        // Create a mock expert pool
        let pool = ExpertPool {
            domain: "Investment Analysis".to_string(),
            question: Some("Should we invest?".to_string()),
            experts: vec![
                PoolExpert {
                    role: "Value Analyst".to_string(),
                    tier: ExpertTier::Core,
                    relevance: 0.95,
                },
                PoolExpert {
                    role: "Risk Manager".to_string(),
                    tier: ExpertTier::Core,
                    relevance: 0.90,
                },
                PoolExpert {
                    role: "Growth Analyst".to_string(),
                    tier: ExpertTier::Adjacent,
                    relevance: 0.75,
                },
                PoolExpert {
                    role: "ESG Analyst".to_string(),
                    tier: ExpertTier::Adjacent,
                    relevance: 0.70,
                },
                PoolExpert {
                    role: "Contrarian".to_string(),
                    tier: ExpertTier::Wildcard,
                    relevance: 0.35,
                },
            ],
        };
        let pool_path = format!("{}/expert-pool.json", test_dir);
        fs::write(&pool_path, serde_json::to_string_pretty(&pool).unwrap()).unwrap();

        // Test evolve_panel with mixed sources
        let args = json!({
            "output_dir": test_dir,
            "round": 1,
            "panel": [
                { "name": "Muffin", "role": "Value Analyst", "source": "retained" },
                { "name": "Cupcake", "role": "Risk Manager", "source": "retained" },
                { "name": "Scone", "role": "ESG Analyst", "source": "pool" },
                { "name": "Palmier", "role": "Supply Chain Analyst", "source": "created", "tier": "Adjacent", "focus": "Geographic concentration risk" }
            ]
        });

        let result = handle_evolve_panel(&args).unwrap();

        // Verify response
        assert_eq!(result["status"], "success");
        assert_eq!(result["round"], 1);
        assert_eq!(result["panel_size"], 4);
        assert_eq!(result["retained"], 2);
        assert_eq!(result["from_pool"], 1);
        assert_eq!(result["created"], 1);

        // Verify panel file was created
        let panel_path = format!("{}/round-1/panel.json", test_dir);
        assert!(std::path::Path::new(&panel_path).exists());

        // Verify panel content
        let panel_content = fs::read_to_string(&panel_path).unwrap();
        let panel_data: Value = serde_json::from_str(&panel_content).unwrap();

        // Check history section
        assert_eq!(panel_data["history"]["retained_count"], 2);
        assert_eq!(panel_data["history"]["from_pool_count"], 1);
        assert_eq!(panel_data["history"]["created_count"], 1);

        // Check agents array
        let agents = panel_data["agents"].as_array().unwrap();
        assert_eq!(agents.len(), 4);

        // Verify created expert has focus
        let palmier = agents.iter().find(|a| a["name"] == "Palmier").unwrap();
        assert_eq!(palmier["focus"], "Geographic concentration risk");

        // Cleanup
        fs::remove_dir_all(test_dir).ok();
    }

    #[test]
    fn test_handle_evolve_panel_validates_unique_names() {
        let test_dir = "/tmp/blue-dialogue/evolve-panel-unique-test";
        fs::create_dir_all(test_dir).unwrap();

        // Test with duplicate names
        let args = json!({
            "output_dir": test_dir,
            "round": 1,
            "panel": [
                { "name": "Muffin", "role": "Value Analyst", "source": "retained" },
                { "name": "Muffin", "role": "Risk Manager", "source": "retained" }
            ]
        });

        let result = handle_evolve_panel(&args);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_str = format!("{:?}", err);
        assert!(err_str.contains("unique"));

        // Cleanup
        fs::remove_dir_all(test_dir).ok();
    }

    #[test]
    fn test_generate_context_brief_round_0() {
        // Round 0 should return empty brief
        let brief = generate_context_brief("/tmp/nonexistent", 0).unwrap();
        assert!(brief.is_empty());
    }

    #[test]
    fn test_generate_context_brief_round_1() {
        use std::fs;

        let test_dir = "/tmp/blue-dialogue/context-brief-test";
        fs::create_dir_all(test_dir).unwrap();

        // Create tensions file
        let tensions = "| ID | Tension | Status |\n|---|---|---|\n| T01 | Valuation vs Growth | Open |\n| T02 | Risk concentration | Open |";
        fs::write(format!("{}/tensions.md", test_dir), tensions).unwrap();

        // Create round-0 summary
        let summary = "Round 0 saw strong disagreement on valuation metrics. Key tension emerged around geographic concentration.";
        fs::write(format!("{}/round-0.summary.md", test_dir), summary).unwrap();

        // Generate context brief for round 1
        let brief = generate_context_brief(test_dir, 1).unwrap();

        // Verify it includes key sections
        assert!(brief.contains("Context for Round 1"));
        assert!(brief.contains("Key Tensions"));
        assert!(brief.contains("T01"));
        assert!(brief.contains("Round 0 Summary"));
        assert!(brief.contains("valuation"));
        assert!(brief.contains("Your Task"));

        // Cleanup
        fs::remove_dir_all(test_dir).ok();
    }

    #[test]
    fn test_graduated_panel_workflow_small_panel() {
        //! Integration test: Full graduated panel workflow with a small panel
        //!
        //! Scenario: Data design RFC with 4 experts in pool, panel size 2
        //! Problem: Sampling might miss critical "Data Architect"
        //! Solution: Judge overrides Round 0 panel using evolve_panel

        let test_dir = "/tmp/blue-dialogue/graduated-workflow-test";
        fs::remove_dir_all(test_dir).ok();
        fs::create_dir_all(test_dir).unwrap();

        // Step 1: Create a small expert pool where Data Architect is Wildcard
        // (simulating a case where critical expertise might not be sampled)
        let pool = ExpertPool {
            domain: "Data Architecture".to_string(),
            question: Some("How should we design the data layer?".to_string()),
            experts: vec![
                PoolExpert {
                    role: "API Architect".to_string(),
                    tier: ExpertTier::Core,
                    relevance: 0.95,
                },
                PoolExpert {
                    role: "Security Engineer".to_string(),
                    tier: ExpertTier::Core,
                    relevance: 0.90,
                },
                PoolExpert {
                    role: "Performance Engineer".to_string(),
                    tier: ExpertTier::Adjacent,
                    relevance: 0.70,
                },
                // Data Architect as Wildcard - might not be sampled in a small panel!
                PoolExpert {
                    role: "Data Architect".to_string(),
                    tier: ExpertTier::Wildcard,
                    relevance: 0.40,
                },
            ],
        };
        let pool_path = format!("{}/expert-pool.json", test_dir);
        fs::write(&pool_path, serde_json::to_string_pretty(&pool).unwrap()).unwrap();

        // Step 2: Sample panel size 2 - might miss Data Architect
        let sampled = sample_panel_from_pool(&pool, 2);
        let suggested_agents = assign_pastry_names(sampled);

        // Step 3: Build protocol with graduated mode
        let protocol = build_judge_protocol(
            &suggested_agents,
            &format!("{}/dialogue.md", test_dir),
            "sonnet",
            &[],
            test_dir,
            Some(&pool),
            RotationMode::Graduated,
        );

        // Verify suggestion semantics
        assert!(protocol.get("suggested_panel").is_some());
        assert!(protocol.get("agents").is_none());
        assert_eq!(protocol["panel_is_suggestion"], true);

        let suggested = protocol["suggested_panel"].as_array().unwrap();
        println!("\n=== SUGGESTED PANEL ===");
        for agent in suggested {
            println!("  {}: {}", agent["name"], agent["role"]);
        }

        // Step 4: Override Round 0 to ensure Data Architect is included
        let override_args = json!({
            "output_dir": test_dir,
            "round": 0,
            "panel": [
                { "name": "Muffin", "role": "API Architect", "source": "pool" },
                { "name": "Cupcake", "role": "Data Architect", "source": "pool" }
            ]
        });

        let result = handle_evolve_panel(&override_args).unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(result["round"], 0);
        assert_eq!(result["from_pool"], 2);

        println!("\n=== OVERRIDDEN ROUND 0 PANEL ===");
        for agent in result["agents"].as_array().unwrap() {
            println!("  {} {}: {}", agent["emoji"], agent["name"], agent["role"]);
        }

        // Verify panel file created for round 0
        let panel_path = format!("{}/round-0/panel.json", test_dir);
        assert!(std::path::Path::new(&panel_path).exists());

        // Step 5: Get round prompt
        let prompt_args = json!({
            "output_dir": test_dir,
            "agent_name": "Cupcake",
            "agent_emoji": "🧁",
            "agent_role": "Data Architect",
            "round": 0,
            "expert_source": "pool"
        });

        let prompt_result = handle_round_prompt(&prompt_args).unwrap();
        assert_eq!(prompt_result["status"], "success");
        assert!(prompt_result["prompt"]
            .as_str()
            .unwrap()
            .contains("Data Architect"));

        // Step 6: Simulate Round 1 evolution
        fs::write(
            format!("{}/tensions.md", test_dir),
            "| ID | Tension |\n|---|---|\n| T01 | Schema flexibility vs performance |",
        )
        .unwrap();
        fs::write(
            format!("{}/round-0.summary.md", test_dir),
            "Debate on design approach.",
        )
        .unwrap();

        let evolve_args = json!({
            "output_dir": test_dir,
            "round": 1,
            "panel": [
                { "name": "Muffin", "role": "API Architect", "source": "retained" },
                { "name": "Cupcake", "role": "Data Architect", "source": "retained" },
                { "name": "Scone", "role": "Performance Engineer", "source": "pool" }
            ]
        });

        let evolve_result = handle_evolve_panel(&evolve_args).unwrap();
        assert_eq!(evolve_result["retained"], 2);
        assert_eq!(evolve_result["from_pool"], 1);
        assert!(!evolve_result["context_brief"].is_null());

        println!("\n=== ROUND 1 EVOLVED PANEL ===");
        println!("  Retained: {}", evolve_result["retained"]);
        println!("  From pool: {}", evolve_result["from_pool"]);
        for agent in evolve_result["agents"].as_array().unwrap() {
            println!("  {} {}: {}", agent["emoji"], agent["name"], agent["role"]);
        }

        // Step 7: Fresh expert gets context brief
        let fresh_args = json!({
            "output_dir": test_dir,
            "agent_name": "Scone",
            "agent_emoji": "🧁",
            "agent_role": "Performance Engineer",
            "round": 1,
            "expert_source": "pool"
        });

        let fresh_result = handle_round_prompt(&fresh_args).unwrap();
        assert_eq!(fresh_result["has_context_brief"], true);
        assert!(fresh_result["prompt"]
            .as_str()
            .unwrap()
            .contains("Context for Round 1"));

        println!("\n=== FRESH EXPERT CONTEXT ===");
        println!("  ✓ Context brief included for Scone");
        println!("  ✓ Can see prior tensions in prompt");

        // Cleanup
        fs::remove_dir_all(test_dir).ok();
        println!("\n=== TEST PASSED ===\n");
    }

    #[test]
    fn test_is_valid_pastry_name() {
        assert!(is_valid_pastry_name("Muffin"));
        assert!(is_valid_pastry_name("Cupcake"));
        assert!(is_valid_pastry_name("Sfogliatella"));
        assert!(is_valid_pastry_name("Pastry21"));
        assert!(is_valid_pastry_name("Pastry99"));
        assert!(!is_valid_pastry_name("KETTLE"));
        assert!(!is_valid_pastry_name("TEAPOT"));
        assert!(!is_valid_pastry_name("LOCKBOX"));
        assert!(!is_valid_pastry_name("Backend Architect"));
        assert!(!is_valid_pastry_name("Pastry")); // no digits
        assert!(!is_valid_pastry_name("muffin")); // case sensitive
    }

    #[test]
    fn test_handle_round_verify_missing_files() {
        let dir = std::env::temp_dir().join("blue-test-round-verify");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("round-0")).unwrap();
        // Only create one agent file
        std::fs::write(dir.join("round-0/muffin.md"), "test").unwrap();

        let args = json!({
            "output_dir": dir.to_str().unwrap(),
            "round": 0,
            "agents": ["Muffin", "Cupcake"]
        });

        let result = handle_round_verify(&args).unwrap();
        assert_eq!(result["status"], "missing_files");
        let missing = result["missing"].as_array().unwrap();
        assert!(missing.iter().any(|m| m.as_str().unwrap().contains("cupcake.md")));
        assert!(missing.iter().any(|m| m.as_str().unwrap().contains("scoreboard.md")));
        assert!(missing.iter().any(|m| m.as_str().unwrap().contains("tensions.md")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_handle_round_verify_all_present() {
        let dir = std::env::temp_dir().join("blue-test-round-verify-ok");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("round-0")).unwrap();
        std::fs::write(dir.join("round-0/muffin.md"), "test").unwrap();
        std::fs::write(dir.join("round-0/cupcake.md"), "test").unwrap();
        std::fs::write(dir.join("scoreboard.md"), "test").unwrap();
        std::fs::write(dir.join("tensions.md"), "test").unwrap();
        std::fs::write(dir.join("round-0.summary.md"), "test").unwrap();

        let args = json!({
            "output_dir": dir.to_str().unwrap(),
            "round": 0,
            "agents": ["Muffin", "Cupcake"]
        });

        let result = handle_round_verify(&args).unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result["missing"].as_array().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
