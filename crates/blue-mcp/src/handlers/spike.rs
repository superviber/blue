//! Spike tool handlers
//!
//! Handles spike creation and completion.

use std::fs;

use blue_core::{DocType, Document, ProjectState, Spike, SpikeOutcome};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_spike_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let question = args
        .get("question")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let time_box = args.get("time_box").and_then(|v| v.as_str());

    // Create the spike
    let mut spike = Spike::new(title, question);
    if let Some(tb) = time_box {
        spike.time_box = Some(tb.to_string());
    }

    // Generate filename with date
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let filename = format!("spikes/{}-{}.md", date, to_kebab_case(title));

    // Generate markdown
    let markdown = spike.to_markdown();

    // Write the file
    let docs_path = state.home.docs_path.clone();
    let spike_path = docs_path.join(&filename);
    if let Some(parent) = spike_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }
    fs::write(&spike_path, &markdown).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Add to store
    let mut doc = Document::new(DocType::Spike, title, "in-progress");
    doc.file_path = Some(filename.clone());

    let id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "id": id,
        "title": title,
        "date": date,
        "file": spike_path.display().to_string(),
        "markdown": markdown,
        "message": blue_core::voice::success(
            &format!("Started spike '{}'", title),
            Some("Time to investigate.")
        )
    }))
}

/// Handle blue_spike_complete
pub fn handle_complete(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let outcome_str = args
        .get("outcome")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let summary = args.get("summary").and_then(|v| v.as_str());

    // Parse outcome
    let outcome = match outcome_str {
        "no-action" => SpikeOutcome::NoAction,
        "decision-made" => SpikeOutcome::DecisionMade,
        "recommends-implementation" => SpikeOutcome::RecommendsImplementation,
        _ => {
            return Err(ServerError::InvalidParams);
        }
    };

    // Check if recommends-implementation - require RFC creation
    if matches!(outcome, SpikeOutcome::RecommendsImplementation) {
        return Ok(json!({
            "status": "rfc_required",
            "title": title,
            "outcome": outcome_str,
            "message": blue_core::voice::ask(
                "This spike recommends building something",
                &format!("Create an RFC with source_spike='{}' first", title)
            ),
            "suggested_tool": "blue_rfc_create",
            "suggested_args": {
                "source_spike": title,
                "problem": summary.unwrap_or("(from spike investigation)")
            }
        }));
    }

    // Find the spike
    let doc = state
        .store
        .find_document(DocType::Spike, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update status
    state
        .store
        .update_document_status(DocType::Spike, title, "complete")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update markdown file (RFC 0008: use shared helper)
    if let Some(ref file_path) = doc.file_path {
        let full_path = state.home.docs_path.join(file_path);
        let _ = blue_core::update_markdown_status(&full_path, "complete");
    }

    let hint = match outcome {
        SpikeOutcome::NoAction => "No action needed. Moving on.",
        SpikeOutcome::DecisionMade => "Decision recorded.",
        SpikeOutcome::RecommendsImplementation => unreachable!(),
    };

    Ok(json!({
        "status": "success",
        "title": title,
        "outcome": outcome_str,
        "message": blue_core::voice::success(
            &format!("Completed spike '{}'", title),
            Some(hint)
        )
    }))
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
