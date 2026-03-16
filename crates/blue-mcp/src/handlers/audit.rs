//! Audit tool handler
//!
//! Checks project health and finds issues.

use blue_core::{DocType, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Issue found during audit
#[derive(Debug)]
struct AuditIssue {
    category: &'static str,
    title: String,
    issue: String,
    severity: &'static str,
}

/// Handle blue_audit
pub fn handle_audit(state: &ProjectState) -> Result<Value, ServerError> {
    let mut issues: Vec<AuditIssue> = Vec::new();
    let mut recommendations: Vec<String> = Vec::new();

    // Check 1: In-progress RFCs without worktrees (stalled)
    if let Ok(docs) = state
        .store
        .list_documents_by_status(DocType::Rfc, "in-progress")
    {
        let worktrees = state.store.list_worktrees().unwrap_or_default();
        for doc in docs {
            let has_worktree = worktrees
                .iter()
                .any(|wt| wt.document_id == doc.id.unwrap_or(0));
            if !has_worktree {
                issues.push(AuditIssue {
                    category: "rfc",
                    title: doc.title.clone(),
                    issue: "In-progress but no active worktree (possibly stalled)".into(),
                    severity: "warning",
                });
                recommendations.push(format!(
                    "Check on '{}' - marked in-progress but no worktree found",
                    doc.title
                ));
            }
        }
    }

    // Check 2: Implemented RFCs without ADRs
    if let Ok(implemented) = state
        .store
        .list_documents_by_status(DocType::Rfc, "implemented")
    {
        if let Ok(adrs) = state.store.list_documents(DocType::Adr) {
            for rfc in implemented {
                let has_adr = adrs
                    .iter()
                    .any(|adr| adr.title == rfc.title || adr.title.contains(&rfc.title));
                if !has_adr {
                    issues.push(AuditIssue {
                        category: "rfc",
                        title: rfc.title.clone(),
                        issue: "Implemented but no ADR created".into(),
                        severity: "info",
                    });
                }
            }
        }
    }

    // Check 3: Draft RFCs (potential backlog)
    if let Ok(drafts) = state.store.list_documents_by_status(DocType::Rfc, "draft") {
        let draft_count = drafts.len();
        if draft_count > 5 {
            recommendations.push(format!(
                "{} draft RFCs - consider reviewing and accepting or archiving",
                draft_count
            ));
        }
    }

    // Check 4: Stale reminders (overdue by more than 7 days)
    if let Ok(reminders) = state
        .store
        .list_reminders(Some(blue_core::ReminderStatus::Pending), false)
    {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        for reminder in reminders {
            if let Some(due) = &reminder.due_date {
                if due < &today {
                    issues.push(AuditIssue {
                        category: "reminder",
                        title: reminder.title.clone(),
                        issue: format!("Overdue since {}", due),
                        severity: "warning",
                    });
                }
            }
        }
    }

    // Check 5: Expired staging locks
    if let Ok(locks) = state.store.list_staging_locks() {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        for lock in locks {
            if lock.expires_at < now {
                issues.push(AuditIssue {
                    category: "staging",
                    title: lock.resource.clone(),
                    issue: format!(
                        "Lock expired at {} (held by '{}')",
                        lock.expires_at, lock.locked_by
                    ),
                    severity: "warning",
                });
                recommendations.push(format!(
                    "Run blue_staging_cleanup to clear expired lock on '{}'",
                    lock.resource
                ));
            }
        }
    }

    // Generate summary
    let error_count = issues.iter().filter(|i| i.severity == "error").count();
    let warning_count = issues.iter().filter(|i| i.severity == "warning").count();
    let info_count = issues.iter().filter(|i| i.severity == "info").count();

    let hint = if error_count > 0 {
        format!(
            "{} errors, {} warnings found - attention needed",
            error_count, warning_count
        )
    } else if warning_count > 0 {
        format!("{} warnings found - review recommended", warning_count)
    } else if info_count > 0 {
        format!("{} items noted - project is healthy", info_count)
    } else {
        "No issues found - project is healthy".into()
    };

    // Format issues for response
    let issues_json: Vec<_> = issues
        .iter()
        .map(|i| {
            json!({
                "category": i.category,
                "title": i.title,
                "issue": i.issue,
                "severity": i.severity,
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("{} issues found", issues.len()),
            Some(&hint)
        ),
        "issues": issues_json,
        "recommendations": recommendations,
        "summary": {
            "errors": error_count,
            "warnings": warning_count,
            "info": info_count,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_empty_project() {
        let state = ProjectState::for_test();
        let result = handle_audit(&state).unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(result["issues"].as_array().unwrap().len(), 0);
    }
}
