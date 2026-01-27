//! Decision tool handlers
//!
//! Handles lightweight Decision Note creation.

use std::fs;

use blue_core::{Decision, DocType, Document, ProjectState, title_to_slug};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_decision_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let decision_text = args
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let rationale = args.get("rationale").and_then(|v| v.as_str());
    let alternatives: Vec<String> = args
        .get("alternatives")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Create decision
    let mut decision = Decision::new(title, decision_text);
    if let Some(rat) = rationale {
        decision.rationale = Some(rat.to_string());
    }
    decision.alternatives = alternatives;

    // Generate markdown
    let markdown = decision.to_markdown();

    // Compute file path with ISO 8601 timestamp (RFC 0031)
    let today = blue_core::utc_timestamp();
    let file_name = format!("{}-{}.recorded.md", today, title_to_slug(title));
    let file_path = format!("decisions/{}", file_name);

    // Write the file
    let docs_path = state.home.docs_path.clone();
    let decision_path = docs_path.join(&file_path);

    // Check if already exists
    if decision_path.exists() {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Decision '{}' already exists for today", title),
                "Use a different title or update the existing one"
            )
        }));
    }

    if let Some(parent) = decision_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }
    fs::write(&decision_path, &markdown).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Add to store
    let mut doc = Document::new(DocType::Decision, title, "recorded");
    doc.file_path = Some(file_path.clone());

    let id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "id": id,
        "title": title,
        "file": decision_path.display().to_string(),
        "markdown": markdown,
        "message": blue_core::voice::success(
            &format!("Recorded decision: '{}'", title),
            None
        )
    }))
}

