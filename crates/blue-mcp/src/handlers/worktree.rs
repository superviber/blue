//! Worktree tool handlers
//!
//! Handles git worktree operations for isolated feature development.
//!
//! Branch naming convention (RFC 0007):
//! - RFC file: `NNNN-feature-description.md`
//! - Branch: `feature-description` (number prefix stripped)
//! - Worktree: `feature-description`

use std::path::Path;

use blue_core::{DocType, ProjectState, Worktree as StoreWorktree};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Detect the appropriate install command for a project
///
/// Checks for package manager lock files and project files in priority order.
/// Returns None if a setup script exists (takes precedence) or no package manager detected.
fn detect_install_command(path: &Path) -> Option<String> {
    // Custom setup script takes precedence
    if path.join("scripts/setup-worktree.sh").exists() {
        return None;
    }

    // Node.js - check lock files for package manager
    if path.join("package.json").exists() {
        if path.join("bun.lockb").exists() {
            return Some("bun install".into());
        }
        if path.join("pnpm-lock.yaml").exists() {
            return Some("pnpm install".into());
        }
        if path.join("yarn.lock").exists() {
            return Some("yarn install".into());
        }
        return Some("npm install".into());
    }

    // Python
    if path.join("pyproject.toml").exists() {
        if path.join("uv.lock").exists() {
            return Some("uv sync".into());
        }
        if path.join("poetry.lock").exists() {
            return Some("poetry install".into());
        }
        return Some("pip install -e .".into());
    }
    if path.join("requirements.txt").exists() {
        return Some("pip install -r requirements.txt".into());
    }

    // Rust
    if path.join("Cargo.toml").exists() {
        return Some("cargo build".into());
    }

    // Go
    if path.join("go.mod").exists() {
        return Some("go mod download".into());
    }

    // Generic Makefile
    if path.join("Makefile").exists() {
        return Some("make".into());
    }

    None
}

/// Check for a custom setup script
fn detect_setup_script(path: &Path) -> Option<String> {
    let script_path = path.join("scripts/setup-worktree.sh");
    if script_path.exists() {
        Some("./scripts/setup-worktree.sh".into())
    } else {
        None
    }
}

/// Strip RFC number prefix from title
///
/// Converts `0007-consistent-branch-naming` to `consistent-branch-naming`
/// Returns (stripped_name, rfc_number) if pattern matches, otherwise (original, None)
pub fn strip_rfc_number_prefix(title: &str) -> (String, Option<u32>) {
    // Match pattern: NNNN-rest-of-title
    if title.len() > 5 && title.chars().take(4).all(|c| c.is_ascii_digit()) && title.chars().nth(4) == Some('-') {
        let number: Option<u32> = title[..4].parse().ok();
        let stripped = title[5..].to_string();
        (stripped, number)
    } else {
        (title.to_string(), None)
    }
}

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

    // Check RFC has a plan (RFC 0014: plan enforcement)
    let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;
    let tasks = state
        .store
        .get_tasks(doc_id)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if tasks.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("RFC '{}' needs a plan before creating worktree", title),
                "Create a plan first with blue_rfc_plan"
            ),
            "next_action": {
                "tool": "blue_rfc_plan",
                "args": { "title": title },
                "hint": "Create implementation tasks before starting work"
            }
        }));
    }

    // Check if worktree already exists
    if let Ok(Some(_existing)) = state.store.get_worktree(doc_id) {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Worktree for '{}' already exists", title),
                "Use blue_worktree_list to see active worktrees"
            )
        }));
    }

    // Create branch name and worktree path (RFC 0007: strip number prefix)
    let (stripped_name, _rfc_number) = strip_rfc_number_prefix(title);
    let branch_name = stripped_name.clone();
    let worktree_path = state.home.worktrees_path.join(&stripped_name);

    // Try to create the git worktree
    let repo_path = state.home.root.clone();
    match git2::Repository::open(&repo_path) {
        Ok(repo) => {
            match blue_core::repo::create_worktree(&repo, &branch_name, &worktree_path) {
                Ok(()) => {
                    // Record in store
                    let wt = StoreWorktree {
                        id: None,
                        document_id: doc_id,
                        branch_name: branch_name.clone(),
                        worktree_path: worktree_path.display().to_string(),
                        created_at: None,
                    };
                    let _ = state.store.add_worktree(&wt);

                    // Update RFC status to in-progress if accepted
                    if doc.status == "accepted" {
                        let _ = state.store.update_document_status(DocType::Rfc, title, "in-progress");
                    }

                    // Detect install command and setup script
                    let install_command = detect_install_command(&worktree_path);
                    let setup_script = detect_setup_script(&worktree_path);

                    // Build hint message
                    let setup_hint = if let Some(ref script) = setup_script {
                        format!("Run `{}` to set up.", script)
                    } else if let Some(ref cmd) = install_command {
                        format!("Run `{}` to install dependencies.", cmd)
                    } else {
                        String::new()
                    };

                    let hint = format!(
                        "cd {} to start working. {}",
                        worktree_path.display(),
                        setup_hint
                    );

                    Ok(json!({
                        "status": "success",
                        "title": title,
                        "branch": branch_name,
                        "path": worktree_path.display().to_string(),
                        "install_command": install_command,
                        "setup_script": setup_script,
                        "message": blue_core::voice::success(
                            &format!("Created worktree for '{}'", title),
                            Some(hint.trim())
                        ),
                        "next_action": {
                            "tool": if setup_script.is_some() || install_command.is_some() {
                                "Bash"
                            } else {
                                "blue_rfc_validate"
                            },
                            "hint": if setup_script.is_some() {
                                "Run setup script to configure the worktree"
                            } else if install_command.is_some() {
                                "Install dependencies before starting work"
                            } else {
                                "Check RFC plan progress as you implement"
                            }
                        }
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

    // Support both old (rfc/title) and new (stripped) naming conventions
    let (stripped_name, _) = strip_rfc_number_prefix(title);
    let branch_name = stripped_name.clone();

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
            if let Ok(false) = blue_core::repo::is_branch_merged(&repo, &worktree.branch_name, "main") {
                if let Ok(false) = blue_core::repo::is_branch_merged(&repo, &worktree.branch_name, "develop") {
                    return Ok(json!({
                        "status": "error",
                        "message": blue_core::voice::error(
                            &format!("Branch '{}' isn't merged yet", worktree.branch_name),
                            "Merge first, or use force=true to remove anyway"
                        )
                    }));
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_rfc_number_prefix() {
        // Standard RFC title with number
        let (stripped, number) = strip_rfc_number_prefix("0007-consistent-branch-naming");
        assert_eq!(stripped, "consistent-branch-naming");
        assert_eq!(number, Some(7));

        // Another example
        let (stripped, number) = strip_rfc_number_prefix("0001-some-feature");
        assert_eq!(stripped, "some-feature");
        assert_eq!(number, Some(1));

        // High number
        let (stripped, number) = strip_rfc_number_prefix("9999-last-rfc");
        assert_eq!(stripped, "last-rfc");
        assert_eq!(number, Some(9999));
    }

    #[test]
    fn test_strip_rfc_number_prefix_no_number() {
        // No number prefix
        let (stripped, number) = strip_rfc_number_prefix("some-feature");
        assert_eq!(stripped, "some-feature");
        assert_eq!(number, None);

        // Too few digits
        let (stripped, number) = strip_rfc_number_prefix("007-james-bond");
        assert_eq!(stripped, "007-james-bond");
        assert_eq!(number, None);

        // No hyphen after number
        let (stripped, number) = strip_rfc_number_prefix("0007feature");
        assert_eq!(stripped, "0007feature");
        assert_eq!(number, None);
    }

    #[test]
    fn test_worktree_requires_plan() {
        use blue_core::{Document, ProjectState};

        let state = ProjectState::for_test();

        // Create an accepted RFC without a plan
        let mut doc = Document::new(DocType::Rfc, "test-rfc", "accepted");
        doc.number = Some(1);
        state.store.add_document(&doc).unwrap();

        // Try to create worktree - should fail due to missing plan
        let args = serde_json::json!({ "title": "test-rfc" });
        let result = handle_create(&state, &args).unwrap();

        assert_eq!(result["status"], "error");
        assert!(result["message"].as_str().unwrap().contains("needs a plan"));
    }
}
