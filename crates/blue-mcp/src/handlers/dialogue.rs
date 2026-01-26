//! Dialogue tool handlers
//!
//! Handles dialogue document creation, storage, and extraction.
//! Dialogues capture agent conversations and link them to RFCs.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use blue_core::{DocType, Document, LinkType, ProjectState};
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::ServerError;

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

    // Generate file path with date prefix
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let file_name = format!("{}-{}.dialogue.md", date, to_kebab_case(title));
    let file_path = PathBuf::from("dialogues").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let dialogue_path = docs_path.join(&file_path);

    // Generate markdown content
    let markdown = generate_dialogue_markdown(
        title,
        dialogue_number,
        rfc_title,
        summary,
        content,
    );

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

    // Create dialogues directory if it doesn't exist
    if let Some(parent) = dialogue_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }
    fs::write(&dialogue_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    let hint = if rfc_title.is_some() {
        "Dialogue recorded and linked to RFC."
    } else {
        "Dialogue recorded. Consider linking to an RFC."
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Dialogue recorded: {}", title),
            Some(hint)
        ),
        "dialogue": {
            "id": dialogue_id,
            "number": dialogue_number,
            "title": title,
            "file": dialogue_path.display().to_string(),
            "linked_rfc": rfc_title,
        },
        "content": markdown,
    }))
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
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let time = chrono::Local::now().format("%H:%M").to_string();

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

/// Convert a string to kebab-case for filenames
fn to_kebab_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jq_check() {
        // Just verify this doesn't panic
        let _ = jq_available();
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("RFC Implementation Discussion"), "rfc-implementation-discussion");
        assert_eq!(to_kebab_case("quick-chat"), "quick-chat");
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
}
