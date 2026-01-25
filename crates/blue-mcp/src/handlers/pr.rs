//! Pull Request tool handlers
//!
//! Handles PR creation, verification, and merge with workflow enforcement.
//! Supports both GitHub and Forgejo/Gitea via the forge abstraction (RFC 0013).
//!
//! Enforces:
//! - Base branch must be `develop` (not `main`)
//! - Test plan checkboxes must be verified before merge
//! - User must approve PR before merge
//!
//! PR title convention (RFC 0007):
//! - Format: `RFC NNNN: Feature Description`
//! - Example: `RFC 0007: Consistent Branch Naming`

use std::process::Command;

use blue_core::{CreatePrOpts, MergeStrategy, ProjectState, create_forge, detect_forge_type, parse_git_url};
use serde_json::{json, Value};

use crate::error::ServerError;
use crate::handlers::worktree::strip_rfc_number_prefix;

/// Task category for test plan items
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskCategory {
    /// Can be automated via CLI (run tests, build, lint)
    CliAutomatable,
    /// Can be automated via browser (visual verification)
    BrowserAutomatable,
    /// Requires human verification
    TrulyManual,
}


/// Handle blue_pr_create
///
/// If `rfc` is provided (e.g., "0007-consistent-branch-naming"), the title
/// will be formatted as "RFC 0007: Consistent Branch Naming" per RFC 0007.
///
/// Uses native REST API for the detected forge (GitHub or Forgejo/Gitea).
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let rfc = args.get("rfc").and_then(|v| v.as_str());

    // If RFC is provided, format title as "RFC NNNN: Title Case Name"
    let title = if let Some(rfc_title) = rfc {
        let (stripped, number) = strip_rfc_number_prefix(rfc_title);
        let title_case = to_title_case(&stripped);
        if let Some(n) = number {
            format!("RFC {:04}: {}", n, title_case)
        } else {
            title_case
        }
    } else {
        args.get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?
            .to_string()
    };

    let base = args
        .get("base")
        .and_then(|v| v.as_str())
        .unwrap_or("develop");

    let body = args.get("body").and_then(|v| v.as_str());
    let draft = args.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);

    // Enforce base branch
    if base == "main" || base == "master" {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "Can't target main directly",
                "Use 'develop' as base branch, then release to main"
            )
        }));
    }

    // Get remote URL and detect forge type
    let remote_url = match get_remote_url(&state.home.root) {
        Ok(url) => url,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Couldn't detect git remote",
                    &e
                )
            }));
        }
    };

    let git_url = parse_git_url(&remote_url);
    let forge_type = detect_forge_type(&remote_url);

    // Get current branch for head
    let head = match get_current_branch(&state.home.root) {
        Ok(branch) => branch,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Couldn't get current branch",
                    &e
                )
            }));
        }
    };

    // Create forge client and make PR
    let forge = match create_forge(&remote_url) {
        Ok(f) => f,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Couldn't create forge client",
                    &format!("{}", e)
                )
            }));
        }
    };

    let opts = CreatePrOpts {
        owner: git_url.owner.clone(),
        repo: git_url.repo.clone(),
        head,
        base: base.to_string(),
        title: title.clone(),
        body: body.map(|s| s.to_string()),
        draft,
    };

    match forge.create_pr(opts) {
        Ok(pr) => {
            Ok(json!({
                "status": "success",
                "pr_url": pr.url,
                "pr_number": pr.number,
                "forge": forge_type.to_string(),
                "base_branch": base,
                "title": title,
                "message": blue_core::voice::success(
                    &format!("Created PR #{}", pr.number),
                    Some(&pr.url)
                ),
                "next_steps": [
                    "Run blue_pr_verify to check test plan items"
                ]
            }))
        }
        Err(e) => {
            Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Failed to create PR",
                    &format!("{}", e)
                )
            }))
        }
    }
}

/// Handle blue_pr_verify
pub fn handle_verify(_state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let pr_number = args.get("pr_number").and_then(|v| v.as_u64()).map(|n| n as u32);

    // Fetch PR via gh CLI
    let pr_data = fetch_pr_data(pr_number)?;

    // Parse test plan from PR body
    let items = parse_test_plan(&pr_data.body);

    let checked_count = items.iter().filter(|(_, checked, _)| *checked).count();
    let unchecked: Vec<_> = items
        .iter()
        .filter(|(_, checked, _)| !*checked)
        .collect();

    let cli_tasks: Vec<_> = unchecked
        .iter()
        .filter(|(_, _, cat)| matches!(cat, TaskCategory::CliAutomatable))
        .map(|(desc, _, _)| desc.clone())
        .collect();

    let browser_tasks: Vec<_> = unchecked
        .iter()
        .filter(|(_, _, cat)| matches!(cat, TaskCategory::BrowserAutomatable))
        .map(|(desc, _, _)| desc.clone())
        .collect();

    let manual_tasks: Vec<_> = unchecked
        .iter()
        .filter(|(_, _, cat)| matches!(cat, TaskCategory::TrulyManual))
        .map(|(desc, _, _)| desc.clone())
        .collect();

    let all_verified = unchecked.is_empty();

    Ok(json!({
        "status": "success",
        "pr_number": pr_data.number,
        "pr_state": pr_data.state,
        "test_plan": {
            "total": items.len(),
            "checked": checked_count,
            "unchecked": unchecked.len(),
            "all_verified": all_verified
        },
        "unchecked_by_category": {
            "cli_automatable": cli_tasks,
            "browser_automatable": browser_tasks,
            "truly_manual": manual_tasks
        },
        "message": if all_verified {
            blue_core::voice::success(
                &format!("PR #{}: All {} items verified", pr_data.number, items.len()),
                Some("Ready to check approvals with blue_pr_check_approvals.")
            )
        } else {
            format!(
                "PR #{}: {}/{} verified. CLI: {}, Browser: {}, Manual: {}",
                pr_data.number, checked_count, items.len(),
                cli_tasks.len(), browser_tasks.len(), manual_tasks.len()
            )
        }
    }))
}

/// Handle blue_pr_check_item
pub fn handle_check_item(_state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let item = args
        .get("item")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let pr_number = args.get("pr_number").and_then(|v| v.as_u64()).map(|n| n as u32);
    let verified_by = args.get("verified_by").and_then(|v| v.as_str());

    // Fetch current PR body
    let pr_data = fetch_pr_data(pr_number)?;

    // Find and update the item
    let (updated_body, matched_item) = update_checkbox_in_body(&pr_data.body, item)?;

    // Update PR via gh CLI
    update_pr_body(pr_data.number, &updated_body)?;

    // Re-parse to get updated status
    let items = parse_test_plan(&updated_body);
    let unchecked_count = items.iter().filter(|(_, checked, _)| !*checked).count();

    Ok(json!({
        "status": "success",
        "item_checked": matched_item,
        "verified_by": verified_by,
        "remaining_unchecked": unchecked_count,
        "all_verified": unchecked_count == 0,
        "message": blue_core::voice::success(
            &format!("Checked: '{}'", matched_item),
            Some(&format!("{} items remaining.", unchecked_count))
        )
    }))
}

/// Handle blue_pr_check_approvals
pub fn handle_check_approvals(_state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let pr_number = args.get("pr_number").and_then(|v| v.as_u64()).map(|n| n as u32);

    let (approved, approved_by) = fetch_pr_approvals(pr_number)?;
    let pr_data = fetch_pr_data(pr_number)?;

    // Check test plan completion
    let items = parse_test_plan(&pr_data.body);
    let all_items_checked = items.iter().all(|(_, checked, _)| *checked);

    let ready_to_merge = approved && all_items_checked;

    let mut blocking_reasons = Vec::new();
    if !approved {
        blocking_reasons.push("Waiting for user approval on GitHub".to_string());
    }
    if !all_items_checked {
        let unchecked = items.iter().filter(|(_, checked, _)| !*checked).count();
        blocking_reasons.push(format!("{} test plan items unchecked", unchecked));
    }

    Ok(json!({
        "status": "success",
        "pr_number": pr_data.number,
        "approved": approved,
        "approved_by": approved_by,
        "test_plan_complete": all_items_checked,
        "ready_to_merge": ready_to_merge,
        "blocking_reasons": blocking_reasons,
        "message": if ready_to_merge {
            blue_core::voice::success(
                "PR approved and verified",
                Some("Ready to merge with blue_pr_merge.")
            )
        } else {
            blue_core::voice::error(
                "Not ready to merge",
                &blocking_reasons.join(". ")
            )
        }
    }))
}

/// Handle blue_pr_merge
pub fn handle_merge(_state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let pr_number = args.get("pr_number").and_then(|v| v.as_u64()).map(|n| n as u32);
    let squash = args.get("squash").and_then(|v| v.as_bool()).unwrap_or(true);

    // Fetch PR and check preconditions
    let pr_data = fetch_pr_data(pr_number)?;
    let (approved, _) = fetch_pr_approvals(pr_number)?;
    let items = parse_test_plan(&pr_data.body);
    let all_items_checked = items.iter().all(|(_, checked, _)| *checked);

    // Enforce preconditions
    if !approved {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "Can't merge without approval",
                "Get user approval on GitHub first"
            )
        }));
    }

    if !all_items_checked {
        let unchecked = items.iter().filter(|(_, checked, _)| !*checked).count();
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("{} test plan items still unchecked", unchecked),
                "Run blue_pr_verify to complete verification"
            )
        }));
    }

    let merge_cmd = format!(
        "gh pr merge {} {}--delete-branch",
        pr_data.number,
        if squash { "--squash " } else { "" }
    );

    Ok(json!({
        "status": "success",
        "command": merge_cmd,
        "pr_number": pr_data.number,
        "squash": squash,
        "next_steps": [
            format!("Run: {}", merge_cmd),
            "Run blue_worktree_remove to clean up"
        ],
        "message": blue_core::voice::success(
            &format!("PR #{} ready to merge", pr_data.number),
            Some("Run the command to merge.")
        )
    }))
}

// =============================================================================
// Helper functions
// =============================================================================

struct PrData {
    number: u32,
    body: String,
    state: String,
}

fn fetch_pr_data(pr_number: Option<u32>) -> Result<PrData, ServerError> {
    let mut args = vec!["pr", "view", "--json", "number,body,state"];

    let pr_num_str;
    if let Some(n) = pr_number {
        pr_num_str = n.to_string();
        args.insert(2, &pr_num_str);
    }

    let output = Command::new("gh")
        .args(&args)
        .output()
        .map_err(|e| ServerError::CommandFailed(format!("Failed to run gh: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerError::CommandFailed(format!(
            "gh pr view failed: {}",
            stderr
        )));
    }

    let data: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to parse PR data: {}", e)))?;

    Ok(PrData {
        number: data["number"].as_u64().unwrap_or(0) as u32,
        body: data["body"].as_str().unwrap_or("").to_string(),
        state: data["state"].as_str().unwrap_or("").to_string(),
    })
}

fn fetch_pr_approvals(pr_number: Option<u32>) -> Result<(bool, Vec<String>), ServerError> {
    let mut args = vec!["pr", "view", "--json", "reviews"];

    let pr_num_str;
    if let Some(n) = pr_number {
        pr_num_str = n.to_string();
        args.insert(2, &pr_num_str);
    }

    let output = Command::new("gh")
        .args(&args)
        .output()
        .map_err(|e| ServerError::CommandFailed(format!("Failed to run gh: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerError::CommandFailed(format!(
            "gh pr view failed: {}",
            stderr
        )));
    }

    let data: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to parse reviews: {}", e)))?;

    let reviews = data["reviews"].as_array();
    let approved_by: Vec<String> = reviews
        .map(|arr| {
            arr.iter()
                .filter(|r| r["state"].as_str() == Some("APPROVED"))
                .filter_map(|r| r["author"]["login"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok((!approved_by.is_empty(), approved_by))
}

fn update_pr_body(pr_number: u32, new_body: &str) -> Result<(), ServerError> {
    let output = Command::new("gh")
        .args(["pr", "edit", &pr_number.to_string(), "--body", new_body])
        .output()
        .map_err(|e| ServerError::CommandFailed(format!("Failed to run gh: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerError::CommandFailed(format!(
            "gh pr edit failed: {}",
            stderr
        )));
    }

    Ok(())
}

/// Parse test plan checkboxes from PR body
fn parse_test_plan(body: &str) -> Vec<(String, bool, TaskCategory)> {
    let mut items = Vec::new();
    let mut in_test_plan = false;

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("## Test") || trimmed.starts_with("## test") {
            in_test_plan = true;
            continue;
        }

        if in_test_plan && trimmed.starts_with("## ") {
            break;
        }

        if in_test_plan {
            if let Some((desc, checked)) = parse_checkbox_line(trimmed) {
                let category = categorize_task(&desc);
                items.push((desc, checked, category));
            }
        }
    }

    items
}

fn parse_checkbox_line(line: &str) -> Option<(String, bool)> {
    if line.starts_with("- [x]") || line.starts_with("- [X]") {
        Some((line[5..].trim().to_string(), true))
    } else if line.starts_with("- [ ]") {
        Some((line[5..].trim().to_string(), false))
    } else {
        None
    }
}

fn categorize_task(description: &str) -> TaskCategory {
    let lower = description.to_lowercase();

    // CLI-automatable patterns
    let cli_patterns = [
        "run tests", "run test", "unit test", "npm test", "cargo test",
        "build", "compile", "lint", "format", "install", "type check",
        "pytest", "make",
    ];

    if cli_patterns.iter().any(|p| lower.contains(p)) {
        return TaskCategory::CliAutomatable;
    }

    // Truly manual patterns (check before browser patterns)
    let manual_patterns = [
        "physical device", "screen reader", "voiceover", "nvda",
        "subjective", "intuitive", "usability", "production",
        "accessibility audit", "manual",
    ];

    if manual_patterns.iter().any(|p| lower.contains(p)) {
        return TaskCategory::TrulyManual;
    }

    // Browser-automatable patterns
    let browser_patterns = [
        "verify", "check", "confirm", "displays", "shows", "click",
        "navigate", "form", "modal", "dropdown", "responsive", "login",
        "error message", "validation", "visual",
    ];

    if browser_patterns.iter().any(|p| lower.contains(p)) {
        return TaskCategory::BrowserAutomatable;
    }

    // Default to manual for unknown
    TaskCategory::TrulyManual
}

fn update_checkbox_in_body(body: &str, item_selector: &str) -> Result<(String, String), ServerError> {
    let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
    let mut matched_item = None;
    let mut matched_line_idx = None;
    let mut in_test_plan = false;
    let mut item_index = 0usize;

    // Try to parse as index first
    let target_index: Option<usize> = item_selector.parse().ok();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("## Test") || trimmed.starts_with("## test") {
            in_test_plan = true;
            continue;
        }

        if in_test_plan && trimmed.starts_with("## ") {
            break;
        }

        if in_test_plan && trimmed.starts_with("- [ ]") {
            item_index += 1;
            let description = trimmed[5..].trim();

            let matches = target_index.map(|idx| idx == item_index).unwrap_or(false)
                || description.to_lowercase().contains(&item_selector.to_lowercase());

            if matches {
                matched_item = Some(description.to_string());
                matched_line_idx = Some(i);
                break;
            }
        }
    }

    if let Some(idx) = matched_line_idx {
        lines[idx] = lines[idx].replace("- [ ]", "- [x]");
    }

    match matched_item {
        Some(item) => Ok((lines.join("\n"), item)),
        None => Err(ServerError::NotFound(format!(
            "No matching unchecked item for: {}",
            item_selector
        ))),
    }
}

/// Convert kebab-case to Title Case
///
/// Example: "consistent-branch-naming" -> "Consistent Branch Naming"
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

/// Get the remote URL from git config
///
/// Tries 'origin' first, then falls back to any other remote.
fn get_remote_url(repo_path: &std::path::Path) -> Result<String, String> {
    let repo = git2::Repository::discover(repo_path)
        .map_err(|e| format!("Not a git repository: {}", e))?;

    // Try origin first
    if let Ok(remote) = repo.find_remote("origin") {
        if let Some(url) = remote.url() {
            return Ok(url.to_string());
        }
    }

    // Try forgejo remote (common in Blue repos)
    if let Ok(remote) = repo.find_remote("forgejo") {
        if let Some(url) = remote.url() {
            return Ok(url.to_string());
        }
    }

    // Fall back to any remote
    let remotes = repo.remotes()
        .map_err(|e| format!("Couldn't list remotes: {}", e))?;

    for name in remotes.iter().flatten() {
        if let Ok(remote) = repo.find_remote(name) {
            if let Some(url) = remote.url() {
                return Ok(url.to_string());
            }
        }
    }

    Err("No remotes configured".to_string())
}

/// Get the current branch name
fn get_current_branch(repo_path: &std::path::Path) -> Result<String, String> {
    let repo = git2::Repository::discover(repo_path)
        .map_err(|e| format!("Not a git repository: {}", e))?;

    let head = repo.head()
        .map_err(|e| format!("Couldn't get HEAD: {}", e))?;

    head.shorthand()
        .map(|s| s.to_string())
        .ok_or_else(|| "HEAD is not a branch".to_string())
}
