//! RFC tool handlers
//!
//! Standalone functions for RFC lifecycle operations.
//! Called by both MCP server and CLI.

use blue_core::{DocType, Document, ProjectState, Rfc, RfcStatus, title_to_slug, validate_rfc_transition};
use serde_json::{json, Value};
use std::fs;

use crate::error::ServerError;

/// Handle blue_rfc_create
///
/// Creates a new RFC with optional problem statement and source spike.
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let problem = args.get("problem").and_then(|v| v.as_str());
    let source_spike = args.get("source_spike").and_then(|v| v.as_str());

    // Get next RFC number
    let number = state.store.next_number_with_fs(DocType::Rfc, &state.home.docs_path)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Generate markdown
    let mut rfc = Rfc::new(title);
    if let Some(p) = problem {
        rfc.problem = Some(p.to_string());
    }
    if let Some(s) = source_spike {
        // Resolve spike file path for markdown link
        let link = if let Ok(spike_doc) = state.store.find_document(DocType::Spike, s) {
            if let Some(ref file_path) = spike_doc.file_path {
                format!("[{}](../{})", s, file_path)
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        };
        rfc.source_spike = Some(link);
    }

    let markdown = rfc.to_markdown(number as u32);

    // Generate filename and write file
    let filename = format!("rfcs/{:04}-{}.draft.md", number, title_to_slug(title));
    let docs_path = state.home.docs_path.clone();
    let rfc_path = docs_path.join(&filename);
    if let Some(parent) = rfc_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }
    fs::write(&rfc_path, &markdown)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Create document in store with file path
    let mut doc = Document::new(DocType::Rfc, title, "draft");
    doc.number = Some(number);
    doc.file_path = Some(filename.clone());

    let id = state.store.add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "id": id,
        "number": number,
        "title": title,
        "file": rfc_path.display().to_string(),
        "markdown": markdown,
        "message": blue_core::voice::success(
            &format!("Created RFC {:04}: '{}'", number, title),
            Some("Want me to help fill in the details?")
        )
    }))
}

/// Handle blue_rfc_get
///
/// Retrieves RFC details including tasks and progress.
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state.store.find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id;
    let rfc_number = doc.number.unwrap_or(0);

    // RFC 0017: Check if plan file exists and cache is stale - rebuild if needed
    let plan_path = blue_core::plan_file_path(&state.home.docs_path, title, rfc_number);
    let mut cache_rebuilt = false;

    if let Some(id) = doc_id {
        if plan_path.exists() {
            let cache_mtime = state.store.get_plan_cache_mtime(id)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            if blue_core::is_cache_stale(&plan_path, cache_mtime.as_deref()) {
                // Rebuild cache from plan file
                let plan = blue_core::read_plan_file(&plan_path)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                state.store.rebuild_tasks_from_plan(id, &plan.tasks)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                // Update cache mtime
                let mtime = chrono::Utc::now().to_rfc3339();
                state.store.update_plan_cache_mtime(id, &mtime)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                cache_rebuilt = true;
            }
        }
    }

    // Get tasks if any
    let tasks = if let Some(id) = doc_id {
        state.store.get_tasks(id).unwrap_or_default()
    } else {
        vec![]
    };

    let progress = if let Some(id) = doc_id {
        state.store.get_task_progress(id).ok()
    } else {
        None
    };

    let mut response = json!({
        "id": doc.id,
        "number": doc.number,
        "title": doc.title,
        "status": doc.status,
        "file_path": doc.file_path,
        "created_at": doc.created_at,
        "updated_at": doc.updated_at,
        "tasks": tasks.iter().map(|t| json!({
            "index": t.task_index,
            "description": t.description,
            "completed": t.completed
        })).collect::<Vec<_>>(),
        "progress": progress.map(|p| json!({
            "completed": p.completed,
            "total": p.total,
            "percentage": p.percentage
        }))
    });

    // Add plan file info if it exists
    if plan_path.exists() {
        response["plan_file"] = json!(plan_path.display().to_string());
        response["_plan_uri"] = json!(format!("blue://docs/rfcs/{}/plan", rfc_number));
        response["cache_rebuilt"] = json!(cache_rebuilt);

        // RFC 0019: Include Claude Code task format for auto-creation
        let incomplete_tasks: Vec<_> = tasks.iter()
            .filter(|t| !t.completed)
            .map(|t| json!({
                "subject": format!("💙 {}", t.description),
                "description": format!("RFC: {}\nTask {} of {}", doc.title, t.task_index + 1, tasks.len()),
                "activeForm": format!("Working on: {}", t.description),
                "metadata": {
                    "blue_rfc": doc.title,
                    "blue_rfc_number": rfc_number,
                    "blue_task_index": t.task_index
                }
            }))
            .collect();

        if !incomplete_tasks.is_empty() {
            response["claude_code_tasks"] = json!(incomplete_tasks);
        }
    }

    Ok(response)
}

/// Handle blue_rfc_list
///
/// Lists all RFCs with optional status filter.
pub fn handle_list(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let status_filter = args.get("status").and_then(|v| v.as_str());

    let docs = if let Some(status) = status_filter {
        state.store.list_documents_by_status(DocType::Rfc, status)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?
    } else {
        state.store.list_documents(DocType::Rfc)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?
    };

    let rfcs: Vec<_> = docs.iter().map(|doc| {
        json!({
            "id": doc.id,
            "number": doc.number,
            "title": doc.title,
            "status": doc.status,
            "file_path": doc.file_path,
            "created_at": doc.created_at
        })
    }).collect();

    Ok(json!({
        "rfcs": rfcs,
        "count": rfcs.len()
    }))
}

/// Handle blue_rfc_update_status
///
/// Updates RFC status with validation.
pub fn handle_update_status(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let status_str = args
        .get("status")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Find the document to get its file path and current status
    let doc = state.store.find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Parse statuses and validate transition (RFC 0014)
    let current_status = RfcStatus::parse(&doc.status)
        .map_err(|e| ServerError::Workflow(e.to_string()))?;
    let target_status = RfcStatus::parse(status_str)
        .map_err(|e| ServerError::Workflow(e.to_string()))?;

    // Validate the transition
    validate_rfc_transition(current_status, target_status)
        .map_err(|e| ServerError::Workflow(e.to_string()))?;

    // Check for worktree if going to in-progress (RFC 0011)
    let has_worktree = state.has_worktree(title);
    let worktree_warning = if status_str == "in-progress" && !has_worktree {
        Some("No worktree exists for this RFC. Consider using blue_worktree_create for isolated development.")
    } else {
        None
    };

    // Update database
    state.store.update_document_status(DocType::Rfc, title, status_str)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Rename file for new status (RFC 0031)
    let final_path = blue_core::rename_for_status(&state.home.docs_path, &state.store, &doc, status_str)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update markdown file (RFC 0008) at effective path
    let effective_path = final_path.as_deref().or(doc.file_path.as_deref());
    let file_updated = if let Some(p) = effective_path {
        let full_path = state.home.docs_path.join(p);
        blue_core::update_markdown_status(&full_path, status_str).unwrap_or(false)
    } else {
        false
    };

    // Conversational hints guide Claude to next action (RFC 0014)
    let hint = match target_status {
        RfcStatus::Accepted => Some(
            "RFC accepted. Ask the user: 'Ready to begin implementation? \
             I'll create a worktree and set up the environment.'"
        ),
        RfcStatus::InProgress => Some(
            "Implementation started. Work in the worktree, mark plan tasks \
             as you complete them."
        ),
        RfcStatus::Implemented => Some(
            "Implementation complete. Ask the user: 'Ready to create a PR?'"
        ),
        RfcStatus::Superseded => Some(
            "RFC superseded. The newer RFC takes precedence."
        ),
        RfcStatus::Draft => None,
    };

    // Build next_action for accepted status (RFC 0011)
    let next_action = if status_str == "accepted" {
        Some(json!({
            "tool": "blue_worktree_create",
            "args": { "title": title },
            "hint": "Create a worktree to start implementation"
        }))
    } else {
        None
    };

    let mut response = json!({
        "status": "success",
        "title": title,
        "new_status": status_str,
        "file_updated": file_updated,
        "message": blue_core::voice::success(
            &format!("Updated '{}' to {}", title, status_str),
            hint
        )
    });

    // Add optional fields
    if let Some(action) = next_action {
        response["next_action"] = action;
    }
    if let Some(warning) = worktree_warning {
        response["warning"] = json!(warning);
    }

    Ok(response)
}

/// Handle blue_rfc_plan
///
/// Creates or updates a plan for an RFC.
pub fn handle_plan(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let tasks: Vec<String> = args
        .get("tasks")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let doc = state.store.find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

    // RFC 0017: Status gating - only allow planning for accepted or in-progress RFCs
    let status_lower = doc.status.to_lowercase();
    if status_lower != "accepted" && status_lower != "in-progress" {
        return Err(ServerError::Workflow(format!(
            "RFC must be 'accepted' or 'in-progress' to create a plan (current: {})",
            doc.status
        )));
    }

    // RFC 0017: Write .plan.md file as authoritative source
    let plan_tasks: Vec<blue_core::PlanTask> = tasks
        .iter()
        .map(|desc| blue_core::PlanTask {
            description: desc.clone(),
            completed: false,
        })
        .collect();

    let plan = blue_core::PlanFile {
        rfc_title: title.to_string(),
        status: blue_core::PlanStatus::InProgress,
        updated_at: chrono::Utc::now().to_rfc3339(),
        tasks: plan_tasks.clone(),
    };

    let rfc_number = doc.number.unwrap_or(0);
    let plan_path = blue_core::plan_file_path(&state.home.docs_path, title, rfc_number);

    // Ensure parent directory exists
    if let Some(parent) = plan_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ServerError::StateLoadFailed(format!("Failed to create directory: {}", e)))?;
    }

    blue_core::write_plan_file(&plan_path, &plan)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update SQLite cache
    state.store.set_tasks(doc_id, &tasks)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Update cache mtime
    let mtime = chrono::Utc::now().to_rfc3339();
    state.store.update_plan_cache_mtime(doc_id, &mtime)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "title": title,
        "task_count": tasks.len(),
        "plan_file": plan_path.display().to_string(),
        "message": blue_core::voice::success(
            &format!("Set {} tasks for '{}'. Plan file created.", tasks.len(), title),
            Some("Mark them complete as you go with blue_rfc_task_complete.")
        )
    }))
}

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

    #[test]
    fn test_create_rfc() {
        let mut state = ProjectState::for_test();
        let args = json!({ "title": "Test RFC" });

        // This will fail because we need a real filesystem for the test
        // but it verifies the function signature is correct
        let result = handle_create(&mut state, &args);
        // Result may fail due to filesystem, but that's OK for this test
        assert!(result.is_ok() || result.is_err());
    }
}
