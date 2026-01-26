//! RFC tool handlers
//!
//! Handles RFC lifecycle operations like marking complete.

use blue_core::{DocType, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_rfc_complete
///
/// Marks an RFC as implemented based on plan progress.
/// - 100%: Plan complete, ready for PR
/// - 70-99%: Core complete, follow-up tasks identified
/// - <70%: Not ready - complete more tasks first
pub fn handle_complete(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Find the RFC
    let doc = state
        .store
        .find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

    // Check current status
    match doc.status.as_str() {
        "draft" => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Can't complete a draft RFC",
                    "Accept it first with blue_rfc_update_status"
                )
            }));
        }
        "implemented" => {
            return Ok(json!({
                "status": "success",
                "title": title,
                "already_implemented": true,
                "message": blue_core::voice::info(
                    &format!("'{}' is already implemented", title),
                    None::<&str>
                )
            }));
        }
        "superseded" => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Can't complete a superseded RFC",
                    "This RFC was replaced by another"
                )
            }));
        }
        _ => {} // accepted or in-progress - continue
    }

    // Check plan progress
    let progress = state
        .store
        .get_task_progress(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // No tasks = assume complete
    let (completed, total, percentage) = if progress.total == 0 {
        (1, 1, 100)
    } else {
        (progress.completed, progress.total, progress.percentage)
    };

    // Check progress thresholds
    if percentage < 70 {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Only {}/{} tasks done ({}%)", completed, total, percentage),
                "Need at least 70% to mark as implemented"
            ),
            "progress": {
                "completed": completed,
                "total": total,
                "percentage": percentage
            }
        }));
    }

    // Auto-advance from accepted to in-progress if needed
    let status_auto_advanced = doc.status == "accepted";
    if status_auto_advanced {
        state
            .store
            .update_document_status(DocType::Rfc, title, "in-progress")
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }

    // Update to implemented
    state
        .store
        .update_document_status(DocType::Rfc, title, "implemented")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Rename file for new status (RFC 0031)
    let final_path = blue_core::rename_for_status(&state.home.docs_path, &state.store, &doc, "implemented")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update markdown at effective path
    let effective_path = final_path.as_deref().or(doc.file_path.as_deref());
    if let Some(p) = effective_path {
        let _ = blue_core::update_markdown_status(&state.home.docs_path.join(p), "implemented");
    }

    // Determine follow-up needs
    let followup_needed = percentage < 100;
    let remaining_count = total - completed;

    // Get remaining tasks if any
    let remaining_tasks: Vec<String> = if followup_needed {
        state
            .store
            .get_tasks(doc_id)
            .unwrap_or_default()
            .iter()
            .filter(|t| !t.completed)
            .map(|t| t.description.clone())
            .collect()
    } else {
        vec![]
    };

    // Check for ADR potential
    let adr_candidate = check_adr_potential(state, title);

    let hint = if followup_needed {
        format!(
            "Core work done ({}%). {} tasks remain for follow-up.",
            percentage, remaining_count
        )
    } else {
        "All tasks complete. Ready for PR.".to_string()
    };

    let adr_hint = if adr_candidate {
        Some(format!(
            "This RFC may warrant an ADR. Use blue_adr_create with rfc='{}' to graduate.",
            title
        ))
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "title": title,
        "new_status": "implemented",
        "message": blue_core::voice::success(
            &format!("Marked '{}' as implemented", title),
            Some(&hint)
        ),
        "status_auto_advanced": status_auto_advanced,
        "followup_needed": followup_needed,
        "remaining_tasks": remaining_tasks,
        "progress": {
            "completed": completed,
            "total": total,
            "percentage": percentage
        },
        "adr_candidate": adr_candidate,
        "adr_hint": adr_hint,
        "next_steps": [
            "Create PR: blue_pr_create",
            "After merge: blue_worktree_cleanup"
        ]
    }))
}

/// Check if an RFC is a good ADR candidate based on architectural indicators
fn check_adr_potential(state: &ProjectState, title: &str) -> bool {
    // Look for architectural keywords in the RFC title/metadata
    let indicators = [
        "architecture",
        "pattern",
        "framework",
        "infrastructure",
        "system",
        "design",
        "structure",
    ];

    let title_lower = title.to_lowercase();
    let score = indicators
        .iter()
        .filter(|&ind| title_lower.contains(ind))
        .count();

    // Also check if there are linked ADRs already
    if let Ok(adrs) = state.store.list_documents(DocType::Adr) {
        let has_adr = adrs.iter().any(|adr| adr.title.contains(title));
        if has_adr {
            return false; // Already has an ADR
        }
    }

    score >= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use blue_core::Document;

    #[test]
    fn test_complete_requires_title() {
        let state = ProjectState::for_test();
        let args = json!({});

        let result = handle_complete(&state, &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_draft_fails() {
        let state = ProjectState::for_test();

        // Create a draft RFC
        let mut doc = Document::new(DocType::Rfc, "test-rfc", "draft");
        doc.number = Some(1);
        state.store.add_document(&doc).unwrap();

        let args = json!({ "title": "test-rfc" });
        let result = handle_complete(&state, &args).unwrap();

        assert_eq!(result["status"], "error");
    }

    #[test]
    fn test_complete_accepted_rfc() {
        let state = ProjectState::for_test();

        // Create an accepted RFC
        let mut doc = Document::new(DocType::Rfc, "test-rfc", "accepted");
        doc.number = Some(1);
        state.store.add_document(&doc).unwrap();

        let args = json!({ "title": "test-rfc" });
        let result = handle_complete(&state, &args).unwrap();

        assert_eq!(result["status"], "success");
        assert_eq!(result["new_status"], "implemented");
        assert_eq!(result["status_auto_advanced"], true);
    }
}
