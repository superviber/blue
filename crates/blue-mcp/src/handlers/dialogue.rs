//! Dialogue tool handlers
//!
//! Handles dialogue document creation, storage, and extraction.
//! Dialogues capture agent conversations and link them to RFCs.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use blue_core::{DocType, Document, LinkType, ProjectState, title_to_slug};
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::ServerError;

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

/// A pastry-themed expert agent for alignment dialogues
#[derive(Debug, Clone, Serialize)]
pub struct PastryAgent {
    pub name: String,
    pub role: String,
    pub emoji: String,
    pub tier: String,
    pub relevance: f64,
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
pub fn handle_extract_dialogue(args: &Value) -> Result<Value, ServerError> {
    let task_id = args.get("task_id").and_then(|v| v.as_str());
    let file_path_arg = args.get("file_path").and_then(|v| v.as_str());

    // Resolve file path
    let file_path = match (task_id, file_path_arg) {
        (Some(id), _) => resolve_task_output(id)?,
        (None, Some(path)) => PathBuf::from(path),
        (None, None) => {
            return Err(ServerError::InvalidParams);
        }
    };

    // Verify file exists
    if !file_path.exists() {
        return Err(ServerError::CommandFailed(format!(
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
        "message": blue_core::voice::info(
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
fn resolve_task_output(task_id: &str) -> Result<PathBuf, ServerError> {
    // Look for task output symlink in /tmp/claude/.../tasks/
    let tmp_claude = PathBuf::from("/tmp/claude");
    if !tmp_claude.exists() {
        return Err(ServerError::CommandFailed(
            "No /tmp/claude directory found. Is Claude Code running?".to_string(),
        ));
    }

    // Search for task output file
    for entry in fs::read_dir(&tmp_claude)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to read /tmp/claude: {}", e)))?
    {
        let entry = entry.map_err(|e| {
            ServerError::CommandFailed(format!("Failed to read directory entry: {}", e))
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

    Err(ServerError::CommandFailed(format!(
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
fn extract_with_jq(file_path: &Path) -> Result<ExtractionResult, ServerError> {
    let output = Command::new("jq")
        .arg("-r")
        .arg(r#"select(.type == "assistant") | .message.content[]? | select(.type == "text") | .text"#)
        .arg(file_path)
        .output()
        .map_err(|e| ServerError::CommandFailed(format!("Failed to run jq: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerError::CommandFailed(format!("jq failed: {}", stderr)));
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
fn extract_with_rust(file_path: &Path) -> Result<ExtractionResult, ServerError> {
    let file = File::open(file_path)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to open file: {}", e)))?;

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
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let rfc_title = args.get("rfc_title").and_then(|v| v.as_str());
    let summary = args.get("summary").and_then(|v| v.as_str());
    let content = args.get("content").and_then(|v| v.as_str());

    // Alignment mode params
    let alignment = args
        .get("alignment")
        .and_then(coerce_bool)
        .unwrap_or(false);
    let agent_count = args
        .get("agents")
        .and_then(|v| v.as_u64())
        .unwrap_or(3) as usize;
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

    // Validate RFC exists if provided
    let rfc_doc = if let Some(rfc) = rfc_title {
        Some(
            state
                .store
                .find_document(DocType::Rfc, rfc)
                .map_err(|_| {
                    ServerError::NotFound(format!("RFC '{}' not found", rfc))
                })?,
        )
    } else {
        None
    };

    // Get next dialogue number
    let dialogue_number = state
        .store
        .next_number_with_fs(DocType::Dialogue, &state.home.docs_path)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Generate file path with ISO 8601 timestamp prefix (RFC 0031)
    let timestamp = blue_core::utc_timestamp();
    let slug = title_to_slug(title);
    let file_name = format!("{}-{}.dialogue.recorded.md", timestamp, slug);
    let file_path = PathBuf::from("dialogues").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let dialogue_path = docs_path.join(&file_path);

    // Generate markdown content — alignment mode gets a different scaffold
    let (markdown, pastry_agents) = if alignment {
        let agents = assign_pastry_agents(agent_count, title);
        let md = generate_alignment_dialogue_markdown(
            title,
            dialogue_number,
            rfc_title,
            &agents,
        );
        (md, Some(agents))
    } else {
        let md = generate_dialogue_markdown(
            title,
            dialogue_number,
            rfc_title,
            summary,
            content,
        );
        (md, None)
    };

    // Create dialogues directory if it doesn't exist
    if let Some(parent) = dialogue_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }

    // Overwrite protection (RFC 0031)
    if dialogue_path.exists() {
        return Err(ServerError::CommandFailed(format!(
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
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Link to RFC if provided
    if let Some(ref rfc) = rfc_doc {
        if let (Some(rfc_id), Some(dialogue_id)) = (rfc.id, Some(dialogue_id)) {
            let _ = state.store.link_documents(
                dialogue_id,
                rfc_id,
                LinkType::DialogueToRfc,
            );
        }
    }

    fs::write(&dialogue_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Build response — RFC 0023: inject protocol as prose in message field
    let (message, judge_protocol) = if let Some(ref agents) = pastry_agents {
        // RFC 0029: Create output directory for file-based subagent output
        let output_dir = format!("/tmp/blue-dialogue/{}", slug);
        fs::create_dir_all(&output_dir).map_err(|e| {
            ServerError::CommandFailed(format!("Failed to create output dir {}: {}", output_dir, e))
        })?;

        let protocol = build_judge_protocol(
            agents,
            &dialogue_path.display().to_string(),
            model,
            &sources,
            &output_dir,
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
        let msg = blue_core::voice::info(
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
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Dialogue, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

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
        "message": blue_core::voice::info(
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
pub fn handle_list(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let rfc_filter = args.get("rfc_title").and_then(|v| v.as_str());

    let all_dialogues = state
        .store
        .list_documents(DocType::Dialogue)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Filter by RFC if specified
    let dialogues: Vec<_> = if let Some(rfc_title) = rfc_filter {
        // First find the RFC
        let rfc_doc = state
            .store
            .find_document(DocType::Rfc, rfc_title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

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
        "message": blue_core::voice::info(
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
pub fn handle_save(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let task_id = args.get("task_id").and_then(|v| v.as_str());
    let file_path_arg = args.get("file_path").and_then(|v| v.as_str());
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;
    let rfc_title = args.get("rfc_title").and_then(|v| v.as_str());
    let summary = args.get("summary").and_then(|v| v.as_str());

    // Resolve and extract content from JSONL
    let jsonl_path = match (task_id, file_path_arg) {
        (Some(id), _) => resolve_task_output(id)?,
        (None, Some(path)) => PathBuf::from(path),
        (None, None) => {
            return Err(ServerError::InvalidParams);
        }
    };

    // Verify file exists
    if !jsonl_path.exists() {
        return Err(ServerError::CommandFailed(format!(
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
        obj.insert("extraction".to_string(), json!({
            "source_file": extraction.source_file,
            "message_count": extraction.message_count,
            "status": format!("{:?}", extraction.status).to_lowercase(),
        }));
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

/// Expert roles keyed by topic keywords
const ROLE_KEYWORDS: &[(&[&str], &str)] = &[
    (&["system", "architect", "infrastructure", "scale"], "Systems Architect"),
    (&["security", "auth", "vulnerability", "trust"], "Security Architect"),
    (&["api", "endpoint", "rest", "grpc", "protocol"], "API Designer"),
    (&["data", "database", "storage", "schema", "model"], "Data Architect"),
    (&["test", "quality", "qa", "reliability"], "Quality Engineer"),
    (&["ux", "ui", "frontend", "user", "interface", "design"], "UX Architect"),
    (&["perf", "performance", "latency", "throughput", "speed"], "Performance Engineer"),
    (&["devops", "deploy", "ci", "cd", "pipeline", "ops"], "DevOps Architect"),
    (&["ml", "ai", "model", "training", "inference"], "ML Engineer"),
    (&["doc", "documentation", "spec", "rfc", "standard"], "Technical Writer"),
];

/// General-purpose roles used when keywords don't match
const GENERAL_ROLES: &[&str] = &[
    "Systems Thinker",
    "Domain Expert",
    "Devil's Advocate",
    "Integration Specialist",
    "Risk Analyst",
    "First Principles Reasoner",
    "Pattern Recognizer",
    "Edge Case Hunter",
];

/// Select a role based on topic keywords
fn select_role_for_topic(topic: &str, index: usize) -> &'static str {
    let topic_lower = topic.to_lowercase();

    // Try keyword matching first — pick the best match for this agent index
    let mut matched_roles: Vec<&str> = Vec::new();
    for (keywords, role) in ROLE_KEYWORDS {
        if keywords.iter().any(|kw| topic_lower.contains(kw)) {
            matched_roles.push(role);
        }
    }

    if index < matched_roles.len() {
        return matched_roles[index];
    }

    // Fall back to general roles
    let general_idx = if matched_roles.is_empty() {
        index
    } else {
        index - matched_roles.len()
    };
    GENERAL_ROLES[general_idx % GENERAL_ROLES.len()]
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

/// Assign pastry-themed agents with expert roles, tiers, and relevance
pub fn assign_pastry_agents(count: usize, topic: &str) -> Vec<PastryAgent> {
    let (core_count, adjacent_count, _wildcard_count) = tier_split(count);

    (0..count)
        .map(|i| {
            let name = if i < PASTRY_NAMES.len() {
                PASTRY_NAMES[i].to_string()
            } else {
                format!("Pastry{}", i + 1)
            };
            let role = select_role_for_topic(topic, i).to_string();
            let (tier, relevance) = if i < core_count {
                ("Core", 0.95 - (i as f64 * 0.05))
            } else if i < core_count + adjacent_count {
                let adj_idx = i - core_count;
                ("Adjacent", 0.70 - (adj_idx as f64 * 0.05))
            } else {
                let wc_idx = i - core_count - adjacent_count;
                ("Wildcard", 0.40 - (wc_idx as f64 * 0.05))
            };
            PastryAgent {
                name,
                role,
                emoji: "🧁".to_string(),
                tier: tier.to_string(),
                relevance,
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

    // Expert Panel table
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
- subagent_type: "alignment-expert" (from task_params in response)
- description: "🧁 Muffin expert deliberation" (from task_params in response)
- max_turns: 5 (from task_params in response)
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
7. CONVERGE: If velocity approaches 0 OR all tensions resolved → declare convergence
   Otherwise, start next round (agents will read Step 5 artifacts via CONTEXT_INSTRUCTIONS).
   Maximum 5 rounds (safety valve)

=== TOKEN BUDGET ===

Your reads per round: ~5KB (scoreboard + tensions + prior summary)
Agent reads per round: ~15KB (tensions + peer files + prior summary)
Both well under 25K limit. Opus usage minimized.

AGENTS: {agent_names}
OUTPUT DIR: {output_dir}

FORMAT RULES — MANDATORY:
- ALWAYS prefix agent names with their emoji (🧁 Muffin) not bare name (Muffin)
- The Judge is 💙 Judge — always include the 💙
- Expert Panel table columns: Agent | Role | Tier | Relevance | Emoji
- Round headers use emoji prefix (### 🧁 Muffin)
- Scores start at 0 — only fill after reading agent returns

NOTE: blue_dialogue_round_prompt handles round-specific context automatically:
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

    json!({
        "instructions": instructions,
        "agent_prompt_template": agent_prompt_template,
        "agents": agent_list,
        "dialogue_file": dialogue_file,
        "model": model,
        "sources": sources,
        "output_dir": output_dir,
        "convergence": {
            "max_rounds": 5,
            "velocity_threshold": 0.1,
            "tension_resolution_gate": true,
        },
    })
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
pub fn handle_round_prompt(args: &Value) -> Result<Value, ServerError> {
    // Required params
    let output_dir = args
        .get("output_dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServerError::InvalidParams)?;
    let agent_name = args
        .get("agent_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServerError::InvalidParams)?;
    let agent_emoji = args
        .get("agent_emoji")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServerError::InvalidParams)?;
    let agent_role = args
        .get("agent_role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ServerError::InvalidParams)?;
    let round = args
        .get("round")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ServerError::InvalidParams)? as usize;

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

    // Build context instructions based on round
    let context_instructions = if round == 0 {
        String::new()
    } else {
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
    };

    // Build the fully-substituted prompt
    let prompt = format!(
        r##"You are {name} {emoji}, a {role} in an ALIGNMENT-seeking dialogue.

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
        context_instructions = context_instructions,
        output_file = output_file,
        source_read_instructions = source_read_instructions,
    );

    Ok(json!({
        "status": "success",
        "prompt": prompt,
        "output_file": output_file,
        "task_params": {
            "subagent_type": "general-purpose",
            "description": format!("{} {} expert deliberation", agent_emoji, agent_name),
            "max_turns": 5,
        }
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
        assert_eq!(title_to_slug("RFC Implementation Discussion"), "rfc-implementation-discussion");
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

    #[test]
    fn test_assign_pastry_agents() {
        let agents = assign_pastry_agents(3, "system design");
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
    fn test_assign_pastry_agents_overflow() {
        let agents = assign_pastry_agents(25, "general topic");
        assert_eq!(agents.len(), 25);
        // First 20 use named pastries
        assert_eq!(agents[0].name, "Muffin");
        assert_eq!(agents[19].name, "Religieuse");
        // Overflow agents get numbered names
        assert_eq!(agents[20].name, "Pastry21");
        assert_eq!(agents[24].name, "Pastry25");
    }

    #[test]
    fn test_select_roles_for_topic() {
        // Security topic should get Security Architect
        let role = select_role_for_topic("security vulnerability assessment", 0);
        assert_eq!(role, "Security Architect");

        // API topic should get API Designer
        let role = select_role_for_topic("api endpoint design", 0);
        assert_eq!(role, "API Designer");

        // Unknown topic falls back to general roles
        let role = select_role_for_topic("something unusual", 0);
        assert_eq!(role, "Systems Thinker");
    }

    #[test]
    fn test_alignment_dialogue_markdown() {
        let agents = assign_pastry_agents(3, "test topic");
        let md = generate_alignment_dialogue_markdown(
            "test-alignment",
            1,
            Some("test-rfc"),
            &agents,
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
    }

    #[test]
    fn test_build_judge_protocol() {
        let agents = assign_pastry_agents(3, "system design");
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &["/tmp/source.rs".to_string()],
            "/tmp/blue-dialogue/system-design",
        );

        // Must have instructions
        let instructions = protocol.get("instructions").unwrap().as_str().unwrap();
        assert!(instructions.contains("alignment-expert"));
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
        let template = protocol.get("agent_prompt_template").unwrap().as_str().unwrap();
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

        // Must have convergence params
        assert_eq!(protocol["convergence"]["max_rounds"], 5);
        assert!(protocol["convergence"]["tension_resolution_gate"].as_bool().unwrap());
    }

    #[test]
    fn test_build_judge_protocol_no_sources() {
        let agents = assign_pastry_agents(2, "quick topic");
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "haiku",
            &[],
            "/tmp/blue-dialogue/quick-topic",
        );

        // Template should NOT contain grounding instructions when no sources
        let template = protocol.get("agent_prompt_template").unwrap().as_str().unwrap();
        assert!(!template.contains("GROUNDING"));
    }

    #[test]
    fn test_build_judge_protocol_output_paths() {
        let agents = assign_pastry_agents(4, "api design");
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/api-design",
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
        let agents = assign_pastry_agents(3, "test artifacts");
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/test-artifacts",
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
        let agents = assign_pastry_agents(3, "context test");
        let protocol = build_judge_protocol(
            &agents,
            "/tmp/test.dialogue.md",
            "sonnet",
            &[],
            "/tmp/blue-dialogue/context-test",
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
        assert_eq!(result["task_params"]["max_turns"], 5);
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
}
