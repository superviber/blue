//! Status and next handlers
//!
//! Standalone functions for project status.
//! Called by both MCP server and CLI.

use blue_core::{DocType, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_status
///
/// Returns project status summary including active, ready, stalled, and draft items.
pub fn handle_status(state: &ProjectState, _args: &Value) -> Result<Value, ServerError> {
    let summary = state.status_summary();

    // Check for index drift across all doc types
    let mut total_drift = 0;
    let mut drift_details = serde_json::Map::new();

    for doc_type in &[DocType::Rfc, DocType::Spike, DocType::Adr, DocType::Decision] {
        if let Ok(result) = state.store.reconcile(&state.home.docs_path, Some(*doc_type), true) {
            if result.has_drift() {
                total_drift += result.drift_count();
                drift_details.insert(
                    format!("{:?}", doc_type).to_lowercase(),
                    json!({
                        "unindexed": result.unindexed.len(),
                        "orphaned": result.orphaned.len(),
                        "stale": result.stale.len()
                    })
                );
            }
        }
    }

    let mut response = json!({
        "project": state.project,
        "active": summary.active,
        "ready": summary.ready,
        "stalled": summary.stalled,
        "drafts": summary.drafts,
        "hint": summary.hint
    });

    if total_drift > 0 {
        response["index_drift"] = json!({
            "total": total_drift,
            "by_type": drift_details,
            "hint": "Run blue_sync to reconcile."
        });
    }

    Ok(response)
}

/// Handle blue_next
///
/// Returns recommendations for what to do next.
pub fn handle_next(state: &ProjectState, _args: &Value) -> Result<Value, ServerError> {
    let summary = state.status_summary();

    let recommendations = if !summary.stalled.is_empty() {
        vec![format!(
            "'{}' might be stalled. Check if work is still in progress.",
            summary.stalled[0].title
        )]
    } else if !summary.ready.is_empty() {
        vec![format!(
            "'{}' is ready to implement. Use blue_worktree_create to begin.",
            summary.ready[0].title
        )]
    } else if !summary.active.is_empty() {
        vec![format!(
            "{} item(s) in progress. Keep going!",
            summary.active.len()
        )]
    } else if !summary.drafts.is_empty() {
        vec![format!(
            "'{}' is still in draft. Review and accept it to begin implementation.",
            summary.drafts[0].title
        )]
    } else {
        vec!["Nothing in flight. Use blue_rfc_create to start something new.".to_string()]
    };

    Ok(json!({
        "recommendations": recommendations,
        "active_count": summary.active_count,
        "ready_count": summary.ready_count,
        "stalled_count": summary.stalled_count,
        "draft_count": summary.draft_count
    }))
}
