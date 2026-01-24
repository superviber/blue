//! Worktree tool handlers
//!
//! Handles git worktree operations for isolated feature development.

use blue_core::{DocType, ProjectState, Worktree as StoreWorktree};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_worktree_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Find the RFC
    let doc = state
        .store
        .find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Check RFC is accepted or in-progress
    if doc.status != "accepted" && doc.status != "in-progress" {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("RFC '{}' is {} - can't create worktree", title, doc.status),
                "Accept the RFC first with blue_rfc_update_status"
            )
        }));
    }

    // Check if worktree already exists
    if let Some(id) = doc.id {
        if let Ok(Some(_existing)) = state.store.get_worktree(id) {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    &format!("Worktree for '{}' already exists", title),
                    "Use blue_worktree_list to see active worktrees"
                )
            }));
        }
    }

    // Create branch name and worktree path
    let branch_name = format!("rfc/{}", title);
    let worktree_path = state.home.worktrees_path.join(title);

    // Try to create the git worktree
    let repo_path = state.home.root.clone();
    match git2::Repository::open(&repo_path) {
        Ok(repo) => {
            match blue_core::repo::create_worktree(&repo, &branch_name, &worktree_path) {
                Ok(()) => {
                    // Record in store
                    if let Some(doc_id) = doc.id {
                        let wt = StoreWorktree {
                            id: None,
                            document_id: doc_id,
                            branch_name: branch_name.clone(),
                            worktree_path: worktree_path.display().to_string(),
                            created_at: None,
                        };
                        let _ = state.store.add_worktree(&wt);
                    }

                    // Update RFC status to in-progress if accepted
                    if doc.status == "accepted" {
                        let _ = state.store.update_document_status(DocType::Rfc, title, "in-progress");
                    }

                    Ok(json!({
                        "status": "success",
                        "title": title,
                        "branch": branch_name,
                        "path": worktree_path.display().to_string(),
                        "message": blue_core::voice::success(
                            &format!("Created worktree for '{}'", title),
                            Some(&format!("cd {} to start working", worktree_path.display()))
                        )
                    }))
                }
                Err(e) => Ok(json!({
                    "status": "error",
                    "message": blue_core::voice::error(
                        &format!("Couldn't create worktree: {}", e),
                        "Check git status and try again"
                    )
                })),
            }
        }
        Err(e) => Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Couldn't open repository: {}", e),
                "Make sure you're in a git repository"
            )
        })),
    }
}

/// Handle blue_worktree_list
pub fn handle_list(state: &ProjectState) -> Result<Value, ServerError> {
    // Get worktrees from store
    let worktrees = state
        .store
        .list_worktrees()
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Enrich with document info
    let enriched: Vec<Value> = worktrees
        .iter()
        .filter_map(|wt| {
            state.store.get_document_by_id(wt.document_id).ok().map(|doc| {
                json!({
                    "title": doc.title,
                    "status": doc.status,
                    "branch": wt.branch_name,
                    "path": wt.worktree_path,
                    "created_at": wt.created_at
                })
            })
        })
        .collect();

    Ok(json!({
        "count": enriched.len(),
        "worktrees": enriched,
        "message": if enriched.is_empty() {
            "No active worktrees."
        } else {
            ""
        }
    }))
}

/// Handle blue_worktree_cleanup
///
/// Full cleanup after PR merge:
/// 1. Verify PR is merged
/// 2. Remove worktree
/// 3. Delete local branch
/// 4. Return commands for switching to develop
pub fn handle_cleanup(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let branch_name = format!("rfc/{}", title);

    // Find the RFC to get worktree info
    let doc = state
        .store
        .find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

    // Get worktree info
    let worktree = state.store.get_worktree(doc_id).ok().flatten();

    // Try to open the repository
    let repo_path = state.home.root.clone();
    let repo = match git2::Repository::open(&repo_path) {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    &format!("Couldn't open repository: {}", e),
                    "Make sure you're in a git repository"
                )
            }));
        }
    };

    // Check if branch is merged
    let is_merged = blue_core::repo::is_branch_merged(&repo, &branch_name, "develop")
        .or_else(|_| blue_core::repo::is_branch_merged(&repo, &branch_name, "main"))
        .unwrap_or(false);

    if !is_merged {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "PR doesn't appear to be merged yet",
                "Complete the merge first with blue_pr_merge"
            )
        }));
    }

    // Remove worktree from git
    let worktree_removed = if worktree.is_some() {
        blue_core::repo::remove_worktree(&repo, &branch_name).is_ok()
    } else {
        false
    };

    // Delete local branch
    let branch_deleted = if let Ok(mut branch) = repo.find_branch(&branch_name, git2::BranchType::Local) {
        branch.delete().is_ok()
    } else {
        false
    };

    // Remove from store
    if worktree.is_some() {
        let _ = state.store.remove_worktree(doc_id);
    }

    let hint = format!(
        "Worktree {}removed, branch {}deleted. Run the commands to complete cleanup.",
        if worktree_removed { "" } else { "not " },
        if branch_deleted { "" } else { "not " }
    );

    Ok(json!({
        "status": "success",
        "title": title,
        "worktree_removed": worktree_removed,
        "branch_deleted": branch_deleted,
        "message": blue_core::voice::success(
            &format!("Cleaned up after '{}'", title),
            Some(&hint)
        ),
        "commands": [
            "git checkout develop",
            "git pull"
        ],
        "next_action": "Execute the commands to sync with develop"
    }))
}

/// Handle blue_worktree_remove
pub fn handle_remove(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);

    // Find the RFC
    let doc = state
        .store
        .find_document(DocType::Rfc, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

    // Get worktree info
    let worktree = state
        .store
        .get_worktree(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?
        .ok_or_else(|| ServerError::StateLoadFailed(format!("No worktree for '{}'", title)))?;

    // Check if branch is merged (unless force)
    if !force {
        let repo_path = state.home.root.clone();
        if let Ok(repo) = git2::Repository::open(&repo_path) {
            match blue_core::repo::is_branch_merged(&repo, &worktree.branch_name, "main") {
                Ok(false) => {
                    // Also check develop
                    match blue_core::repo::is_branch_merged(&repo, &worktree.branch_name, "develop") {
                        Ok(false) => {
                            return Ok(json!({
                                "status": "error",
                                "message": blue_core::voice::error(
                                    &format!("Branch '{}' isn't merged yet", worktree.branch_name),
                                    "Merge first, or use force=true to remove anyway"
                                )
                            }));
                        }
                        _ => {} // Merged into develop, ok
                    }
                }
                _ => {} // Merged into main, ok
            }
        }
    }

    // Remove from git
    let repo_path = state.home.root.clone();
    if let Ok(repo) = git2::Repository::open(&repo_path) {
        if let Err(e) = blue_core::repo::remove_worktree(&repo, &worktree.branch_name) {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    &format!("Couldn't remove worktree: {}", e),
                    "Try removing manually with 'git worktree remove'"
                )
            }));
        }
    }

    // Remove from store
    state
        .store
        .remove_worktree(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "title": title,
        "branch": worktree.branch_name,
        "message": blue_core::voice::success(
            &format!("Removed worktree for '{}'", title),
            None
        )
    }))
}
