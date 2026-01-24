//! Document deletion handlers for Blue MCP
//!
//! Implements soft-delete with 7-day retention and restore capability.

use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use blue_core::store::DocType;
use blue_core::ProjectState;

use crate::ServerError;

/// Check what would be deleted (dry run)
pub fn handle_delete_dry_run(
    state: &ProjectState,
    doc_type: DocType,
    title: &str,
) -> Result<Value, ServerError> {
    let doc = state
        .store
        .find_document(doc_type, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.unwrap();

    // Check for ADR dependents
    let adr_dependents = state
        .store
        .has_adr_dependents(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Check for active sessions
    let active_session = state
        .store
        .get_active_session(&doc.title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Check for worktree
    let worktree = state
        .store
        .get_worktree(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Find companion files
    let mut companion_files = Vec::new();
    if let Some(ref file_path) = doc.file_path {
        let base_path = Path::new(file_path);
        if let Some(stem) = base_path.file_stem() {
            if let Some(parent) = base_path.parent() {
                let stem_str = stem.to_string_lossy();
                // Check for .plan.md, .dialogue.md
                for suffix in &[".plan.md", ".dialogue.md", ".draft.md"] {
                    let companion = parent.join(format!("{}{}", stem_str, suffix));
                    if companion.exists() {
                        companion_files.push(companion.display().to_string());
                    }
                }
            }
        }
    }

    let mut warnings = Vec::new();
    let mut blockers = Vec::new();

    // ADR dependents are permanent blockers
    if !adr_dependents.is_empty() {
        let adr_titles: Vec<_> = adr_dependents.iter().map(|d| d.title.clone()).collect();
        blockers.push(format!(
            "Has ADR dependents: {}. ADRs are permanent records and cannot be cascade-deleted.",
            adr_titles.join(", ")
        ));
    }

    // Non-draft status requires force
    if doc.status != "draft" {
        warnings.push(format!(
            "Status is '{}'. Use force=true to delete non-draft documents.",
            doc.status
        ));
    }

    // Active session requires force
    if let Some(session) = &active_session {
        warnings.push(format!(
            "Has active {} session started at {}. Use force=true to override.",
            session.session_type.as_str(),
            session.started_at
        ));
    }

    Ok(json!({
        "dry_run": true,
        "document": {
            "type": doc_type.as_str(),
            "title": doc.title,
            "status": doc.status,
            "file_path": doc.file_path,
        },
        "would_delete": {
            "primary_file": doc.file_path,
            "companion_files": companion_files,
            "worktree": worktree.map(|w| w.worktree_path),
        },
        "blockers": blockers,
        "warnings": warnings,
        "can_proceed": blockers.is_empty(),
    }))
}

/// Delete a document with safety checks
pub fn handle_delete(
    state: &mut ProjectState,
    doc_type: DocType,
    title: &str,
    force: bool,
    permanent: bool,
) -> Result<Value, ServerError> {
    // Find the document
    let doc = state
        .store
        .find_document(doc_type, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.unwrap();

    // Check for ADR dependents - this is a permanent blocker
    let adr_dependents = state
        .store
        .has_adr_dependents(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if !adr_dependents.is_empty() {
        let adr_titles: Vec<_> = adr_dependents.iter().map(|d| d.title.clone()).collect();
        return Ok(json!({
            "status": "blocked",
            "message": format!(
                "Cannot delete {} '{}'.\n\nThis document has ADR dependents: {}.\nADRs are permanent architectural records and cannot be cascade-deleted.\n\nTo proceed:\n1. Update the ADR(s) to remove the reference, or\n2. Mark this document as 'superseded' instead of deleting",
                doc_type.as_str(),
                doc.title,
                adr_titles.join(", ")
            ),
            "adr_dependents": adr_titles,
        }));
    }

    // Check status - non-draft requires force
    if doc.status != "draft" && !force {
        let status_msg = match doc.status.as_str() {
            "accepted" => "This document has been accepted.",
            "in-progress" => "This document has active work.",
            "implemented" => "This document is a historical record.",
            _ => "This document is not in draft status.",
        };

        return Ok(json!({
            "status": "requires_force",
            "message": format!(
                "Cannot delete {} '{}'.\n\nStatus: {}\n{}\n\nUse force=true to delete anyway.",
                doc_type.as_str(),
                doc.title,
                doc.status,
                status_msg
            ),
            "current_status": doc.status,
        }));
    }

    // Check for active session - requires force
    let active_session = state
        .store
        .get_active_session(&doc.title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if active_session.is_some() && !force {
        let session = active_session.unwrap();
        return Ok(json!({
            "status": "requires_force",
            "message": format!(
                "Cannot delete {} '{}'.\n\nHas active {} session started at {}.\n\nUse force=true to delete anyway, which will end the session.",
                doc_type.as_str(),
                doc.title,
                session.session_type.as_str(),
                session.started_at
            ),
            "active_session": {
                "type": session.session_type.as_str(),
                "started_at": session.started_at,
            },
        }));
    }

    // End any active session
    if active_session.is_some() {
        let _ = state.store.end_session(&doc.title);
    }

    // Remove worktree if exists
    let mut worktree_removed = false;
    if let Ok(Some(worktree)) = state.store.get_worktree(doc_id) {
        // Remove from filesystem
        let worktree_path = Path::new(&worktree.worktree_path);
        if worktree_path.exists() {
            // Use git worktree remove
            let _ = std::process::Command::new("git")
                .args(["worktree", "remove", "--force", &worktree.worktree_path])
                .output();
        }
        // Remove from database
        let _ = state.store.remove_worktree(doc_id);
        worktree_removed = true;
    }

    // Delete companion files
    let mut files_deleted = Vec::new();
    if let Some(ref file_path) = doc.file_path {
        let base_path = Path::new(file_path);
        if let Some(stem) = base_path.file_stem() {
            if let Some(parent) = base_path.parent() {
                let stem_str = stem.to_string_lossy();
                for suffix in &[".plan.md", ".dialogue.md", ".draft.md"] {
                    let companion = parent.join(format!("{}{}", stem_str, suffix));
                    if companion.exists() {
                        if fs::remove_file(&companion).is_ok() {
                            files_deleted.push(companion.display().to_string());
                        }
                    }
                }
            }
        }

        // Delete primary file
        if base_path.exists() {
            if fs::remove_file(base_path).is_ok() {
                files_deleted.push(file_path.clone());
            }
        }
    }

    // Soft or permanent delete
    if permanent {
        state
            .store
            .delete_document(doc_type, &doc.title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    } else {
        state
            .store
            .soft_delete_document(doc_type, &doc.title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
    }

    let action = if permanent {
        "permanently deleted"
    } else {
        "soft-deleted (recoverable for 7 days)"
    };

    Ok(json!({
        "status": "success",
        "message": format!("{} '{}' {}.", doc_type.as_str().to_uppercase(), doc.title, action),
        "doc_type": doc_type.as_str(),
        "title": doc.title,
        "permanent": permanent,
        "files_deleted": files_deleted,
        "worktree_removed": worktree_removed,
        "restore_command": if !permanent {
            Some(format!("blue restore {} {}", doc_type.as_str(), doc.title))
        } else {
            None
        },
    }))
}

/// Restore a soft-deleted document
pub fn handle_restore(
    state: &mut ProjectState,
    doc_type: DocType,
    title: &str,
) -> Result<Value, ServerError> {
    // Check if document exists and is soft-deleted
    let doc = state
        .store
        .get_deleted_document(doc_type, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Restore the document
    state
        .store
        .restore_document(doc_type, &doc.title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "message": format!("{} '{}' restored.", doc_type.as_str().to_uppercase(), doc.title),
        "doc_type": doc_type.as_str(),
        "title": doc.title,
        "note": "Files were deleted and will need to be recreated if needed.",
    }))
}

/// List soft-deleted documents
pub fn handle_list_deleted(
    state: &ProjectState,
    doc_type: Option<DocType>,
) -> Result<Value, ServerError> {
    let deleted = state
        .store
        .list_deleted_documents(doc_type)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let docs: Vec<_> = deleted
        .iter()
        .map(|d| {
            json!({
                "type": d.doc_type.as_str(),
                "title": d.title,
                "status": d.status,
                "deleted_at": d.deleted_at,
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "count": docs.len(),
        "deleted_documents": docs,
        "note": "Documents are auto-purged 7 days after deletion. Use blue_restore to recover.",
    }))
}

/// Purge old soft-deleted documents
pub fn handle_purge_deleted(state: &mut ProjectState, days: i64) -> Result<Value, ServerError> {
    let purged = state
        .store
        .purge_old_deleted_documents(days)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "message": format!("Purged {} documents older than {} days.", purged, days),
        "purged_count": purged,
    }))
}
