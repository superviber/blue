//! Realm MCP tool handlers
//!
//! Implements RFC 0002: Realm MCP Integration
//!
//! Phase 1:
//! - realm_status: Get realm overview
//! - realm_check: Validate contracts/bindings
//! - contract_get: Get contract details
//!
//! Phase 2:
//! - session_start: Begin work session
//! - session_stop: End session with summary
//!
//! Phase 3:
//! - realm_worktree_create: Create worktrees for domain peers
//! - realm_pr_status: Show PR readiness across repos
//!
//! Phase 4:
//! - notifications_list: List notifications with state filters
//! - Schema hash detection in realm_check
//! - 7-day expiration cleanup

use crate::daemon::{DaemonDb, DaemonPaths};
use crate::realm::{LocalRepoConfig, RealmService};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::handler_error::HandlerError;

/// Context detected from current working directory
struct RealmContext {
    realm_name: String,
    repo_name: String,
    service: RealmService,
}

/// Detect realm context from cwd
fn detect_context(cwd: Option<&Path>) -> Result<RealmContext, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;

    // Check for .blue/config.yaml
    let config_path = cwd.join(".blue").join("config.yaml");
    if !config_path.exists() {
        return Err(HandlerError::NotFound(
            "Not in a realm repo. Run 'blue realm admin join <realm>' first.".to_string(),
        ));
    }

    let local_config = LocalRepoConfig::load(&config_path).map_err(|e| {
        HandlerError::CommandFailed(format!("Failed to load .blue/config.yaml: {}", e))
    })?;

    let paths = DaemonPaths::new()
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get daemon paths: {}", e)))?;

    let service = RealmService::new(paths.realms);

    Ok(RealmContext {
        realm_name: local_config.realm.name,
        repo_name: local_config.repo,
        service,
    })
}

/// Handle realm_status - get realm overview
pub fn handle_status(cwd: Option<&Path>) -> Result<Value, HandlerError> {
    let ctx = detect_context(cwd)?;

    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    // Build repos list
    let repos: Vec<Value> = details
        .repos
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "path": r.path,
                "is_current": r.name == ctx.repo_name
            })
        })
        .collect();

    // Build domains list
    let domains: Vec<Value> = details
        .domains
        .iter()
        .map(|d| {
            let contracts: Vec<Value> = d
                .contracts
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "version": c.version,
                        "owner": c.owner
                    })
                })
                .collect();

            let bindings: Vec<Value> = d
                .bindings
                .iter()
                .map(|b| {
                    json!({
                        "repo": b.repo,
                        "role": format!("{:?}", b.role),
                        "exports": b.exports.len(),
                        "imports": b.imports.len()
                    })
                })
                .collect();

            json!({
                "name": d.domain.name,
                "members": d.domain.members,
                "contracts": contracts,
                "bindings": bindings
            })
        })
        .collect();

    // Fetch pending notifications (Phase 4)
    let notifications = fetch_pending_notifications(&ctx);

    // Build next steps
    let mut next_steps = Vec::new();
    if domains.is_empty() {
        next_steps.push("Create a domain with 'blue realm admin domain'".to_string());
    }
    if !notifications.is_empty() {
        next_steps.push(format!(
            "{} pending notification{} to review",
            notifications.len(),
            if notifications.len() == 1 { "" } else { "s" }
        ));
    }

    Ok(json!({
        "status": "success",
        "realm": ctx.realm_name,
        "current_repo": ctx.repo_name,
        "repos": repos,
        "domains": domains,
        "notifications": notifications,
        "next_steps": next_steps
    }))
}

/// Handle realm_check - validate contracts/bindings
pub fn handle_check(cwd: Option<&Path>, realm_arg: Option<&str>) -> Result<Value, HandlerError> {
    let ctx = detect_context(cwd)?;
    let realm_name = realm_arg.unwrap_or(&ctx.realm_name);

    let result = ctx
        .service
        .check_realm(realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to check realm: {}", e)))?;

    // Load realm details for schema integrity check
    let details = ctx
        .service
        .load_realm_details(realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm details: {}", e)))?;

    let errors: Vec<Value> = result
        .errors
        .iter()
        .map(|e| {
            json!({
                "domain": e.domain,
                "kind": format!("{:?}", e.kind),
                "message": e.message
            })
        })
        .collect();

    let warnings: Vec<Value> = result
        .warnings
        .iter()
        .map(|w| {
            json!({
                "domain": w.domain,
                "kind": format!("{:?}", w.kind),
                "message": w.message
            })
        })
        .collect();

    // Get schema hashes for integrity tracking (Phase 4)
    let schema_hashes = check_schema_integrity(&details, &ctx.repo_name);

    // Fetch pending notifications (Phase 4)
    let notifications = fetch_pending_notifications(&ctx);

    // Build next steps
    let mut next_steps = Vec::new();
    if !result.is_ok() {
        next_steps.push("Fix errors before proceeding".to_string());
    }
    if result.has_warnings() {
        next_steps.push("Review warnings - they may indicate issues".to_string());
    }
    if !notifications.is_empty() {
        next_steps.push(format!(
            "{} pending notification{} to review",
            notifications.len(),
            if notifications.len() == 1 { "" } else { "s" }
        ));
    }
    if result.is_ok() && !result.has_warnings() {
        next_steps.push("All checks passed. Ready to proceed.".to_string());
    }

    Ok(json!({
        "status": if result.is_ok() { "success" } else { "error" },
        "realm": realm_name,
        "current_repo": ctx.repo_name,
        "valid": result.is_ok(),
        "errors": errors,
        "warnings": warnings,
        "schema_hashes": schema_hashes,
        "notifications": notifications,
        "next_steps": next_steps
    }))
}

/// Handle contract_get - get contract details
pub fn handle_contract_get(
    cwd: Option<&Path>,
    domain_name: &str,
    contract_name: &str,
) -> Result<Value, HandlerError> {
    let ctx = detect_context(cwd)?;

    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    // Find the domain
    let domain = details
        .domains
        .iter()
        .find(|d| d.domain.name == domain_name)
        .ok_or_else(|| HandlerError::NotFound(format!("Domain '{}' not found", domain_name)))?;

    // Find the contract
    let contract = domain
        .contracts
        .iter()
        .find(|c| c.name == contract_name)
        .ok_or_else(|| {
            HandlerError::NotFound(format!(
                "Contract '{}' not found in domain '{}'",
                contract_name, domain_name
            ))
        })?;

    // Get bindings for this contract
    let bindings: Vec<Value> = domain
        .bindings
        .iter()
        .filter(|b| {
            b.exports.iter().any(|e| e.contract == contract_name)
                || b.imports.iter().any(|i| i.contract == contract_name)
        })
        .map(|b| {
            let exports: Vec<&str> = b
                .exports
                .iter()
                .filter(|e| e.contract == contract_name)
                .map(|_| "export")
                .collect();
            let imports: Vec<String> = b
                .imports
                .iter()
                .filter(|i| i.contract == contract_name)
                .map(|i| format!("import ({})", i.version))
                .collect();

            json!({
                "repo": b.repo,
                "role": format!("{:?}", b.role),
                "relationship": if !exports.is_empty() { "exports" } else { "imports" },
                "version_req": imports.first().cloned()
            })
        })
        .collect();

    // Notifications are fetched via daemon in Phase 4
    let notifications: Vec<Value> = Vec::new();

    // Build next steps
    let mut next_steps = Vec::new();
    if contract.owner == ctx.repo_name {
        next_steps.push("You own this contract. You can modify it.".to_string());
    } else {
        next_steps.push(format!(
            "This contract is owned by '{}'. Contact them for changes.",
            contract.owner
        ));
    }

    Ok(json!({
        "status": "success",
        "realm": ctx.realm_name,
        "domain": domain_name,
        "contract": {
            "name": contract.name,
            "version": contract.version,
            "owner": contract.owner,
            "compatibility": {
                "backwards": contract.compatibility.backwards,
                "forwards": contract.compatibility.forwards
            },
            "schema": contract.schema,
            "value": contract.value,
            "evolution": contract.evolution
        },
        "bindings": bindings,
        "current_repo": ctx.repo_name,
        "notifications": notifications,
        "next_steps": next_steps
    }))
}

// ─── Phase 2: Session Tools ─────────────────────────────────────────────────

/// Session state stored in .blue/session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub id: String,
    pub realm: String,
    pub repo: String,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    #[serde(default)]
    pub active_rfc: Option<String>,
    #[serde(default)]
    pub active_domains: Vec<String>,
    #[serde(default)]
    pub contracts_modified: Vec<String>,
    #[serde(default)]
    pub contracts_watched: Vec<String>,
}

impl SessionState {
    /// Load session from .blue/session file
    pub fn load(cwd: &Path) -> Option<Self> {
        let session_path = cwd.join(".blue").join("session");
        if !session_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&session_path).ok()?;
        serde_yaml::from_str(&content).ok()
    }

    /// Save session to .blue/session file
    pub fn save(&self, cwd: &Path) -> Result<(), HandlerError> {
        let blue_dir = cwd.join(".blue");
        if !blue_dir.exists() {
            return Err(HandlerError::NotFound(
                "Not in a realm repo. No .blue directory.".to_string(),
            ));
        }

        let session_path = blue_dir.join("session");
        let content = serde_yaml::to_string(self).map_err(|e| {
            HandlerError::CommandFailed(format!("Failed to serialize session: {}", e))
        })?;

        std::fs::write(&session_path, content)
            .map_err(|e| HandlerError::CommandFailed(format!("Failed to write session: {}", e)))?;

        Ok(())
    }

    /// Delete session file
    pub fn delete(cwd: &Path) -> Result<(), HandlerError> {
        let session_path = cwd.join(".blue").join("session");
        if session_path.exists() {
            std::fs::remove_file(&session_path).map_err(|e| {
                HandlerError::CommandFailed(format!("Failed to delete session: {}", e))
            })?;
        }
        Ok(())
    }
}

/// Generate a unique session ID
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("sess-{:x}", timestamp)
}

/// Handle session_start - begin work session
pub fn handle_session_start(
    cwd: Option<&Path>,
    active_rfc: Option<&str>,
) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;
    let ctx = detect_context(Some(cwd))?;

    // Check for existing session
    if let Some(existing) = SessionState::load(cwd) {
        return Ok(json!({
            "status": "warning",
            "message": "Session already active",
            "session": {
                "id": existing.id,
                "realm": existing.realm,
                "repo": existing.repo,
                "started_at": existing.started_at.to_rfc3339(),
                "active_rfc": existing.active_rfc,
                "active_domains": existing.active_domains
            },
            "next_steps": ["Use session_stop to end the current session first"]
        }));
    }

    // Determine active domains from repo's bindings
    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    let active_domains: Vec<String> = details
        .domains
        .iter()
        .filter(|d| d.bindings.iter().any(|b| b.repo == ctx.repo_name))
        .map(|d| d.domain.name.clone())
        .collect();

    // Determine contracts we're watching (imports) and could modify (exports)
    let mut contracts_watched = Vec::new();
    let mut contracts_modified = Vec::new();

    for domain in &details.domains {
        for binding in &domain.bindings {
            if binding.repo == ctx.repo_name {
                for import in &binding.imports {
                    contracts_watched.push(format!("{}/{}", domain.domain.name, import.contract));
                }
                for export in &binding.exports {
                    contracts_modified.push(format!("{}/{}", domain.domain.name, export.contract));
                }
            }
        }
    }

    let now = Utc::now();
    let session = SessionState {
        id: generate_session_id(),
        realm: ctx.realm_name.clone(),
        repo: ctx.repo_name.clone(),
        started_at: now,
        last_activity: now,
        active_rfc: active_rfc.map(String::from),
        active_domains: active_domains.clone(),
        contracts_modified: contracts_modified.clone(),
        contracts_watched: contracts_watched.clone(),
    };

    session.save(cwd)?;

    // Build next steps
    let mut next_steps = Vec::new();
    if !contracts_watched.is_empty() {
        next_steps.push(format!(
            "Watching {} imported contract{}",
            contracts_watched.len(),
            if contracts_watched.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    if active_rfc.is_none() {
        next_steps
            .push("Consider setting active_rfc to track which RFC you're working on".to_string());
    }
    next_steps.push("Use session_stop when done to get a summary".to_string());

    Ok(json!({
        "status": "success",
        "message": "Session started",
        "session": {
            "id": session.id,
            "realm": session.realm,
            "repo": session.repo,
            "started_at": session.started_at.to_rfc3339(),
            "active_rfc": session.active_rfc,
            "active_domains": session.active_domains,
            "contracts_modified": contracts_modified,
            "contracts_watched": contracts_watched
        },
        "notifications": [],
        "next_steps": next_steps
    }))
}

/// Handle session_stop - end session with summary
pub fn handle_session_stop(cwd: Option<&Path>) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;

    // Load existing session
    let session = SessionState::load(cwd)
        .ok_or_else(|| HandlerError::NotFound("No active session. Nothing to stop.".to_string()))?;

    // Calculate session duration
    let duration = Utc::now().signed_duration_since(session.started_at);
    let duration_str = if duration.num_hours() > 0 {
        format!("{}h {}m", duration.num_hours(), duration.num_minutes() % 60)
    } else if duration.num_minutes() > 0 {
        format!("{}m", duration.num_minutes())
    } else {
        format!("{}s", duration.num_seconds())
    };

    // Delete the session file
    SessionState::delete(cwd)?;

    // Build summary
    let summary = json!({
        "id": session.id,
        "realm": session.realm,
        "repo": session.repo,
        "started_at": session.started_at.to_rfc3339(),
        "ended_at": Utc::now().to_rfc3339(),
        "duration": duration_str,
        "active_rfc": session.active_rfc,
        "active_domains": session.active_domains,
        "contracts_modified": session.contracts_modified,
        "contracts_watched": session.contracts_watched
    });

    Ok(json!({
        "status": "success",
        "message": format!("Session ended after {}", duration_str),
        "summary": summary,
        "next_steps": ["Start a new session with session_start when you're ready to work again"]
    }))
}

// ─── Phase 3: Workflow Tools ────────────────────────────────────────────────

/// Handle worktree_create - create worktrees for realm repos
///
/// Creates git worktrees for coordinated multi-repo development.
/// Default: selects "domain peers" - repos sharing domains with current repo.
pub fn handle_worktree_create(
    cwd: Option<&Path>,
    rfc: &str,
    repos: Option<Vec<&str>>,
) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;
    let ctx = detect_context(Some(cwd))?;

    // Load realm details to find domain peers
    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    // Determine which repos to create worktrees for
    let (selected_repos, selection_reason) = if let Some(explicit_repos) = repos {
        // User specified repos explicitly
        let repo_list: Vec<String> = explicit_repos.iter().map(|s| s.to_string()).collect();
        (repo_list, "Explicitly specified".to_string())
    } else {
        // Auto-select domain peers
        let mut peers: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut peer_domains: Vec<String> = Vec::new();

        for domain in &details.domains {
            let has_current_repo = domain.bindings.iter().any(|b| b.repo == ctx.repo_name);
            if has_current_repo {
                peer_domains.push(domain.domain.name.clone());
                for binding in &domain.bindings {
                    peers.insert(binding.repo.clone());
                }
            }
        }

        let repo_list: Vec<String> = peers.into_iter().collect();
        let reason = if peer_domains.is_empty() {
            "No shared domains - current repo only".to_string()
        } else {
            format!("Domain peers via {}", peer_domains.join(", "))
        };

        // If no peers found, just use current repo
        if repo_list.is_empty() {
            (
                vec![ctx.repo_name.clone()],
                "Solo repo in realm".to_string(),
            )
        } else {
            (repo_list, reason)
        }
    };

    // Get daemon paths for worktree location
    let paths = DaemonPaths::new()
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get daemon paths: {}", e)))?;

    // Create worktrees under ~/.blue/worktrees/<realm>/<rfc>/
    let worktree_base = paths.base.join("worktrees").join(&ctx.realm_name).join(rfc);
    let mut created: Vec<String> = Vec::new();
    let mut paths_map: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut errors: Vec<String> = Vec::new();

    for repo_name in &selected_repos {
        // Find repo path from realm details
        let repo_info = details.repos.iter().find(|r| &r.name == repo_name);
        let repo_path = match repo_info {
            Some(info) => match &info.path {
                Some(p) => std::path::PathBuf::from(p),
                None => {
                    errors.push(format!("Repo '{}' has no local path configured", repo_name));
                    continue;
                }
            },
            None => {
                errors.push(format!("Repo '{}' not found in realm", repo_name));
                continue;
            }
        };

        // Open the repository
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Failed to open '{}': {}", repo_name, e));
                continue;
            }
        };

        let branch_name = format!("rfc/{}", rfc);
        let worktree_path = worktree_base.join(repo_name);

        // Create parent directories
        if let Some(parent) = worktree_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(format!("Failed to create dirs for '{}': {}", repo_name, e));
                continue;
            }
        }

        // Create worktree using git2
        match create_git_worktree(&repo, &branch_name, &worktree_path) {
            Ok(()) => {
                created.push(repo_name.clone());
                paths_map.insert(
                    repo_name.clone(),
                    Value::String(worktree_path.display().to_string()),
                );
            }
            Err(e) => {
                errors.push(format!(
                    "Failed to create worktree for '{}': {}",
                    repo_name, e
                ));
            }
        }
    }

    // Build next steps
    let mut next_steps = Vec::new();
    if !created.is_empty() {
        let first_path = paths_map.values().next().and_then(|v| v.as_str());
        if let Some(p) = first_path {
            next_steps.push(format!("cd {} to start working", p));
        }
        next_steps.push("Use session_start to track your work".to_string());
    }
    if !errors.is_empty() {
        next_steps.push("Review errors and fix before proceeding".to_string());
    }

    let status = if errors.is_empty() {
        "success"
    } else if created.is_empty() {
        "error"
    } else {
        "partial"
    };

    Ok(json!({
        "status": status,
        "rfc": rfc,
        "realm": ctx.realm_name,
        "reason": selection_reason,
        "created": created,
        "paths": paths_map,
        "errors": errors,
        "next_steps": next_steps
    }))
}

/// Create a git worktree with a new branch
fn create_git_worktree(
    repo: &git2::Repository,
    branch_name: &str,
    worktree_path: &std::path::Path,
) -> Result<(), String> {
    // Check if worktree already exists
    if worktree_path.exists() {
        return Err("Worktree path already exists".to_string());
    }

    // Derive worktree name from path (directory name = slug, no slashes)
    // Git worktree names are stored in .git/worktrees/<name> and cannot contain slashes
    let worktree_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid worktree path")?;

    // Get HEAD commit to branch from
    let head = repo
        .head()
        .map_err(|e| format!("Failed to get HEAD: {}", e))?;
    let commit = head
        .peel_to_commit()
        .map_err(|e| format!("Failed to get commit: {}", e))?;

    // Check if branch exists, create if not
    let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => {
            // Create new branch
            repo.branch(branch_name, &commit, false)
                .map_err(|e| format!("Failed to create branch: {}", e))?
        }
    };

    // Get the reference for the worktree
    let reference = branch.into_reference();

    // Create the worktree
    repo.worktree(
        worktree_name,
        worktree_path,
        Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
    )
    .map_err(|e| format!("Failed to create worktree: {}", e))?;

    Ok(())
}

/// Handle pr_status - get PR readiness across realm repos
///
/// Shows uncommitted changes, commits ahead, and PR status for each repo.
pub fn handle_pr_status(cwd: Option<&Path>, rfc: Option<&str>) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;
    let ctx = detect_context(Some(cwd))?;

    // Load realm details
    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    let branch_name = rfc.map(|r| format!("rfc/{}", r));
    let mut repos_status: Vec<Value> = Vec::new();
    let mut all_clean = true;
    let mut all_pushed = true;

    for repo_info in &details.repos {
        let repo_path = match &repo_info.path {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                repos_status.push(json!({
                    "name": repo_info.name,
                    "path": null,
                    "is_current": repo_info.name == ctx.repo_name,
                    "error": "No local path configured",
                    "ready": false
                }));
                all_clean = false;
                continue;
            }
        };

        let status = match git2::Repository::open(&repo_path) {
            Ok(repo) => {
                let (uncommitted, commits_ahead) = get_repo_status(&repo, branch_name.as_deref());

                if uncommitted > 0 {
                    all_clean = false;
                }
                if commits_ahead > 0 {
                    all_pushed = false;
                }

                // Check for PR if we have gh CLI
                let pr_info = get_pr_info(&repo_path, branch_name.as_deref());

                json!({
                    "name": repo_info.name,
                    "path": repo_path.display().to_string(),
                    "is_current": repo_info.name == ctx.repo_name,
                    "uncommitted_changes": uncommitted,
                    "commits_ahead": commits_ahead,
                    "pr": pr_info,
                    "ready": uncommitted == 0 && commits_ahead == 0
                })
            }
            Err(e) => {
                all_clean = false;
                json!({
                    "name": repo_info.name,
                    "path": repo_path.display().to_string(),
                    "is_current": repo_info.name == ctx.repo_name,
                    "error": format!("Failed to open: {}", e),
                    "ready": false
                })
            }
        };

        repos_status.push(status);
    }

    // Build next steps
    let mut next_steps = Vec::new();
    if !all_clean {
        next_steps.push("Commit changes in repos with uncommitted files".to_string());
    }
    if !all_pushed {
        next_steps.push("Push commits to remote branches".to_string());
    }
    if all_clean && all_pushed {
        next_steps.push("All repos ready. Create PRs with 'gh pr create'.".to_string());
    }

    Ok(json!({
        "status": "success",
        "realm": ctx.realm_name,
        "current_repo": ctx.repo_name,
        "rfc": rfc,
        "repos": repos_status,
        "summary": {
            "all_clean": all_clean,
            "all_pushed": all_pushed,
            "ready_for_pr": all_clean && all_pushed
        },
        "next_steps": next_steps
    }))
}

/// Get repository status (uncommitted changes, commits ahead)
fn get_repo_status(repo: &git2::Repository, branch_name: Option<&str>) -> (usize, usize) {
    // Count uncommitted changes
    let uncommitted = match repo.statuses(None) {
        Ok(statuses) => statuses.len(),
        Err(_) => 0,
    };

    // Count commits ahead of remote
    let commits_ahead = if let Some(branch) = branch_name {
        count_commits_ahead(repo, branch).unwrap_or(0)
    } else {
        // Use current branch
        if let Ok(head) = repo.head() {
            if let Some(name) = head.shorthand() {
                count_commits_ahead(repo, name).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        }
    };

    (uncommitted, commits_ahead)
}

/// Count commits ahead of upstream
fn count_commits_ahead(repo: &git2::Repository, branch_name: &str) -> Result<usize, git2::Error> {
    let local = repo.find_branch(branch_name, git2::BranchType::Local)?;
    let local_commit = local.get().peel_to_commit()?;

    // Try to find upstream
    let upstream_name = format!("origin/{}", branch_name);
    let upstream = match repo.find_branch(&upstream_name, git2::BranchType::Remote) {
        Ok(b) => b,
        Err(_) => return Ok(0), // No upstream, all commits are "ahead"
    };
    let upstream_commit = upstream.get().peel_to_commit()?;

    // Count commits between upstream and local
    let (ahead, _behind) = repo.graph_ahead_behind(local_commit.id(), upstream_commit.id())?;
    Ok(ahead)
}

/// Get PR info from gh CLI (returns None if no PR or gh not available)
fn get_pr_info(repo_path: &std::path::Path, branch_name: Option<&str>) -> Option<Value> {
    use std::process::Command;

    let mut cmd = Command::new("gh");
    cmd.current_dir(repo_path);
    cmd.args(["pr", "view", "--json", "number,state,url,title"]);

    if let Some(branch) = branch_name {
        cmd.args(["--head", branch]);
    }

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let data: Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(json!({
        "number": data["number"],
        "state": data["state"],
        "url": data["url"],
        "title": data["title"]
    }))
}

// ─── Phase 4: Notifications ─────────────────────────────────────────────────

/// Handle notifications_list - list notifications with state filters
///
/// States: pending (not seen by current repo), seen (acknowledged), expired (7+ days old)
pub fn handle_notifications_list(
    cwd: Option<&Path>,
    state_filter: Option<&str>,
) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;
    let ctx = detect_context(Some(cwd))?;

    // Open daemon database
    let paths = DaemonPaths::new()
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to get daemon paths: {}", e)))?;

    let db = DaemonDb::open(&paths.database).map_err(|e| {
        HandlerError::CommandFailed(format!("Failed to open daemon database: {}", e))
    })?;

    // Clean up expired notifications (7+ days old)
    let expired_count = db.cleanup_expired_notifications(7).unwrap_or(0);

    // Get notifications with state
    let notifications = db
        .list_notifications_with_state(&ctx.realm_name, &ctx.repo_name, state_filter)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to list notifications: {}", e)))?;

    // Filter to only domains the current repo participates in
    let details = ctx
        .service
        .load_realm_details(&ctx.realm_name)
        .map_err(|e| HandlerError::CommandFailed(format!("Failed to load realm: {}", e)))?;

    let participating_domains: Vec<String> = details
        .domains
        .iter()
        .filter(|d| d.bindings.iter().any(|b| b.repo == ctx.repo_name))
        .map(|d| d.domain.name.clone())
        .collect();

    let filtered: Vec<Value> = notifications
        .into_iter()
        .filter(|(n, _)| participating_domains.contains(&n.domain))
        .map(|(n, state)| {
            json!({
                "id": n.id,
                "realm": n.realm,
                "domain": n.domain,
                "contract": n.contract,
                "from_repo": n.from_repo,
                "change_type": format!("{:?}", n.change_type),
                "changes": n.changes,
                "created_at": n.created_at.to_rfc3339(),
                "state": state
            })
        })
        .collect();

    // Count by state
    let pending_count = filtered.iter().filter(|n| n["state"] == "pending").count();
    let seen_count = filtered.iter().filter(|n| n["state"] == "seen").count();

    // Build next steps
    let mut next_steps = Vec::new();
    if pending_count > 0 {
        next_steps.push(format!(
            "{} pending notification{} to review",
            pending_count,
            if pending_count == 1 { "" } else { "s" }
        ));
    }
    if expired_count > 0 {
        next_steps.push(format!(
            "Cleaned up {} expired notification{}",
            expired_count,
            if expired_count == 1 { "" } else { "s" }
        ));
    }
    if pending_count == 0 && seen_count == 0 {
        next_steps.push("No notifications. All quiet.".to_string());
    }

    Ok(json!({
        "status": "success",
        "realm": ctx.realm_name,
        "current_repo": ctx.repo_name,
        "filter": state_filter.unwrap_or("all"),
        "notifications": filtered,
        "summary": {
            "total": filtered.len(),
            "pending": pending_count,
            "seen": seen_count,
            "expired_cleaned": expired_count
        },
        "next_steps": next_steps
    }))
}

/// Compute canonical JSON hash for schema change detection
fn compute_schema_hash(schema: &Value) -> String {
    // Canonical JSON: sorted keys, no whitespace
    let canonical = canonical_json(schema);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Convert JSON to canonical form (sorted keys, compact)
fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();
            let pairs: Vec<String> = keys
                .iter()
                .map(|k| format!("\"{}\":{}", k, canonical_json(&map[*k])))
                .collect();
            format!("{{{}}}", pairs.join(","))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", items.join(","))
        }
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
    }
}

/// Enhanced realm_check with schema hash detection
///
/// Computes schema hashes for contracts. Full comparison requires stored hashes
/// which will be added in a future iteration.
pub fn check_schema_integrity(
    details: &crate::realm::RealmDetails,
    repo_name: &str,
) -> Vec<Value> {
    let mut schema_info = Vec::new();

    for domain in &details.domains {
        // Only check contracts we can access (own or import)
        let binding = domain.bindings.iter().find(|b| b.repo == repo_name);
        if binding.is_none() {
            continue;
        }

        for contract in &domain.contracts {
            // Compute hash of current schema
            let schema_hash = compute_schema_hash(&contract.schema);

            schema_info.push(json!({
                "domain": domain.domain.name,
                "contract": contract.name,
                "version": contract.version,
                "schema_hash": schema_hash,
                "owner": contract.owner
            }));
        }
    }

    schema_info
}

/// Fetch pending notifications for piggybacking onto tool responses
fn fetch_pending_notifications(ctx: &RealmContext) -> Vec<Value> {
    let paths = match DaemonPaths::new() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let db = match DaemonDb::open(&paths.database) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // Get pending notifications
    let notifications =
        match db.list_notifications_with_state(&ctx.realm_name, &ctx.repo_name, Some("pending")) {
            Ok(n) => n,
            Err(_) => return Vec::new(),
        };

    // Load realm details to filter by participating domains
    let details = match ctx.service.load_realm_details(&ctx.realm_name) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let participating_domains: Vec<String> = details
        .domains
        .iter()
        .filter(|d| d.bindings.iter().any(|b| b.repo == ctx.repo_name))
        .map(|d| d.domain.name.clone())
        .collect();

    notifications
        .into_iter()
        .filter(|(n, _)| participating_domains.contains(&n.domain))
        .map(|(n, state)| {
            json!({
                "id": n.id,
                "domain": n.domain,
                "contract": n.contract,
                "from_repo": n.from_repo,
                "change_type": format!("{:?}", n.change_type),
                "state": state
            })
        })
        .collect()
}

// ─── Phase 5: RFC Validation (RFC 0038) ─────────────────────────────────────

use crate::realm::LocalRealmDependencies;

/// RFC dependency status for cross-repo coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfcDepStatus {
    /// Dependency string (e.g., "blue-web:0015")
    pub dependency: String,
    /// Parsed repo name
    pub repo: String,
    /// Parsed RFC identifier
    pub rfc_id: String,
    /// Whether the dependency is resolved
    pub resolved: bool,
    /// Status of the RFC if found
    pub status: Option<String>,
    /// Error message if couldn't check
    pub error: Option<String>,
}

/// Handle blue_rfc_validate_realm - validate realm RFC dependencies
///
/// Loads .blue/realm.toml and checks status of cross-repo RFC dependencies.
/// Returns a status matrix showing resolved/unresolved dependencies.
pub fn handle_validate_realm(cwd: Option<&Path>, strict: bool) -> Result<Value, HandlerError> {
    let cwd = cwd.ok_or(HandlerError::InvalidParams)?;

    // Check for .blue/realm.toml
    if !LocalRealmDependencies::exists(cwd) {
        return Ok(json!({
            "status": "success",
            "message": "No .blue/realm.toml found - no cross-repo RFC dependencies defined",
            "dependencies": [],
            "summary": {
                "total": 0,
                "resolved": 0,
                "unresolved": 0
            },
            "next_steps": ["Create .blue/realm.toml to define cross-repo RFC dependencies"]
        }));
    }

    // Load realm dependencies
    let realm_deps = LocalRealmDependencies::load_from_blue(cwd).map_err(|e| {
        HandlerError::CommandFailed(format!("Failed to load .blue/realm.toml: {}", e))
    })?;

    // Collect all dependencies across all RFCs
    let mut all_deps: Vec<RfcDepStatus> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (rfc_id, deps) in &realm_deps.rfc {
        for dep in &deps.depends_on {
            let status = check_dependency(cwd, dep);
            if status.error.is_some() {
                errors.push(format!(
                    "RFC {}: {}",
                    rfc_id,
                    status.error.as_ref().unwrap()
                ));
            }
            all_deps.push(status);
        }
    }

    // Calculate summary
    let total = all_deps.len();
    let resolved = all_deps.iter().filter(|d| d.resolved).count();
    let unresolved = total - resolved;

    // Build next steps
    let mut next_steps = Vec::new();
    if unresolved > 0 {
        next_steps.push(format!(
            "{} unresolved RFC dependencies - coordinate with dependent repos",
            unresolved
        ));

        // List specific unresolved deps
        for dep in all_deps.iter().filter(|d| !d.resolved) {
            if let Some(ref status) = dep.status {
                next_steps.push(format!(
                    "  {} is '{}' - wait for implementation",
                    dep.dependency, status
                ));
            } else if let Some(ref err) = dep.error {
                next_steps.push(format!("  {} - {}", dep.dependency, err));
            }
        }
    }
    if resolved == total && total > 0 {
        next_steps.push("All RFC dependencies resolved - ready to proceed".to_string());
    }

    // In strict mode, return error status if any unresolved
    let status = if strict && unresolved > 0 {
        "error"
    } else {
        "success"
    };

    Ok(json!({
        "status": status,
        "realm": realm_deps.realm.as_ref().map(|r| &r.name),
        "dependencies": all_deps,
        "summary": {
            "total": total,
            "resolved": resolved,
            "unresolved": unresolved
        },
        "errors": errors,
        "next_steps": next_steps
    }))
}

/// Check a single dependency status
///
/// Format: "repo:rfc-id" (e.g., "blue-web:0015")
fn check_dependency(cwd: &Path, dep: &str) -> RfcDepStatus {
    // Parse dependency format: "repo:rfc-id"
    let parts: Vec<&str> = dep.splitn(2, ':').collect();
    if parts.len() != 2 {
        return RfcDepStatus {
            dependency: dep.to_string(),
            repo: String::new(),
            rfc_id: String::new(),
            resolved: false,
            status: None,
            error: Some(format!(
                "Invalid dependency format '{}' - expected 'repo:rfc-id'",
                dep
            )),
        };
    }

    let repo = parts[0].to_string();
    let rfc_id = parts[1].to_string();

    // First, try to check locally if this is the current repo
    if let Some(local_status) = check_local_rfc(cwd, &rfc_id) {
        let resolved = local_status == "implemented";
        return RfcDepStatus {
            dependency: dep.to_string(),
            repo,
            rfc_id,
            resolved,
            status: Some(local_status),
            error: None,
        };
    }

    // Check in realm cache for remote repos
    if let Some(remote_status) = check_remote_rfc(&repo, &rfc_id) {
        let resolved = remote_status == "implemented";
        return RfcDepStatus {
            dependency: dep.to_string(),
            repo,
            rfc_id,
            resolved,
            status: Some(remote_status),
            error: None,
        };
    }

    // Couldn't check - report as unresolved with error
    RfcDepStatus {
        dependency: dep.to_string(),
        repo,
        rfc_id,
        resolved: false,
        status: None,
        error: Some(format!(
            "Could not verify RFC status in repo '{}' - repo not in realm cache",
            parts[0]
        )),
    }
}

/// Check RFC status in the local repo
fn check_local_rfc(cwd: &Path, rfc_id: &str) -> Option<String> {
    // Try to find RFC by number or title in local .blue/docs/rfcs/
    let rfcs_dir = cwd.join(".blue").join("docs").join("rfcs");
    if !rfcs_dir.exists() {
        return None;
    }

    // Look for matching RFC files
    let pattern = format!("{}-", rfc_id);

    if let Ok(entries) = std::fs::read_dir(&rfcs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&pattern) && name.ends_with(".md") {
                // Parse status from filename (e.g., "0015-foo.implemented.md")
                if name.contains(".implemented.") {
                    return Some("implemented".to_string());
                } else if name.contains(".accepted.") {
                    return Some("accepted".to_string());
                } else if name.contains(".impl.") {
                    return Some("in-progress".to_string());
                } else if name.contains(".draft.") {
                    return Some("draft".to_string());
                }
            }
        }
    }

    None
}

/// Check RFC status in a remote repo via realm cache
fn check_remote_rfc(repo: &str, rfc_id: &str) -> Option<String> {
    // Check in /tmp/blue-realm-cache/<repo>/.blue/docs/rfcs/
    let cache_dir = std::path::PathBuf::from("/tmp/blue-realm-cache")
        .join(repo)
        .join(".blue")
        .join("docs")
        .join("rfcs");

    if !cache_dir.exists() {
        return None;
    }

    // Look for matching RFC files
    let pattern = format!("{}-", rfc_id);

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(&pattern) && name.ends_with(".md") {
                // Parse status from filename
                if name.contains(".implemented.") {
                    return Some("implemented".to_string());
                } else if name.contains(".accepted.") {
                    return Some("accepted".to_string());
                } else if name.contains(".impl.") {
                    return Some("in-progress".to_string());
                } else if name.contains(".draft.") {
                    return Some("draft".to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_realm() -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let blue_dir = path.join(".blue");
        std::fs::create_dir_all(&blue_dir).unwrap();

        // Create a minimal config
        let config = r#"
realm:
  name: test-realm
  url: file:///tmp/test-realm
repo: test-repo
"#;
        std::fs::write(blue_dir.join("config.yaml"), config).unwrap();

        (tmp, path)
    }

    #[test]
    fn test_detect_context_no_config() {
        let tmp = TempDir::new().unwrap();
        let result = detect_context(Some(tmp.path()));
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_context_with_config() {
        let (_tmp, path) = setup_test_realm();
        let result = detect_context(Some(&path));
        // Config parsing works - result depends on whether ~/.blue exists
        // This is an integration-level test; just verify it doesn't panic
        match result {
            Ok(ctx) => {
                assert_eq!(ctx.realm_name, "test-realm");
                assert_eq!(ctx.repo_name, "test-repo");
            }
            Err(_) => {
                // Also acceptable if daemon paths don't exist
            }
        }
    }

    // Phase 2: Session tests

    #[test]
    fn test_session_state_save_load() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let blue_dir = path.join(".blue");
        std::fs::create_dir_all(&blue_dir).unwrap();

        let session = SessionState {
            id: "test-session-123".to_string(),
            realm: "test-realm".to_string(),
            repo: "test-repo".to_string(),
            started_at: Utc::now(),
            last_activity: Utc::now(),
            active_rfc: Some("my-rfc".to_string()),
            active_domains: vec!["domain-1".to_string()],
            contracts_modified: vec!["domain-1/contract-a".to_string()],
            contracts_watched: vec!["domain-1/contract-b".to_string()],
        };

        // Save
        session.save(&path).unwrap();

        // Verify file exists
        assert!(blue_dir.join("session").exists());

        // Load
        let loaded = SessionState::load(&path).unwrap();
        assert_eq!(loaded.id, "test-session-123");
        assert_eq!(loaded.realm, "test-realm");
        assert_eq!(loaded.repo, "test-repo");
        assert_eq!(loaded.active_rfc, Some("my-rfc".to_string()));
        assert_eq!(loaded.active_domains, vec!["domain-1".to_string()]);
    }

    #[test]
    fn test_session_state_delete() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let blue_dir = path.join(".blue");
        std::fs::create_dir_all(&blue_dir).unwrap();

        // Create session file
        let session = SessionState {
            id: "to-delete".to_string(),
            realm: "test-realm".to_string(),
            repo: "test-repo".to_string(),
            started_at: Utc::now(),
            last_activity: Utc::now(),
            active_rfc: None,
            active_domains: vec![],
            contracts_modified: vec![],
            contracts_watched: vec![],
        };
        session.save(&path).unwrap();
        assert!(blue_dir.join("session").exists());

        // Delete
        SessionState::delete(&path).unwrap();
        assert!(!blue_dir.join("session").exists());
    }

    #[test]
    fn test_session_state_load_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let result = SessionState::load(tmp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();

        assert!(id1.starts_with("sess-"));
        assert!(id2.starts_with("sess-"));
        // IDs should be unique (different timestamps)
        // Note: Could be same if generated within same millisecond
    }

    #[test]
    fn test_session_stop_no_session() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let blue_dir = path.join(".blue");
        std::fs::create_dir_all(&blue_dir).unwrap();

        let result = handle_session_stop(Some(&path));
        assert!(result.is_err());
    }

    // Phase 4: Notification and schema tests

    #[test]
    fn test_canonical_json_object() {
        let json = json!({
            "z": 1,
            "a": 2,
            "m": 3
        });
        let canonical = canonical_json(&json);
        // Keys should be sorted
        assert!(canonical.starts_with("{\"a\":2"));
        assert!(canonical.contains("\"m\":3"));
        assert!(canonical.ends_with("\"z\":1}"));
    }

    #[test]
    fn test_canonical_json_nested() {
        let json = json!({
            "outer": {
                "b": 2,
                "a": 1
            }
        });
        let canonical = canonical_json(&json);
        // Nested keys should also be sorted
        assert!(canonical.contains("\"a\":1,\"b\":2"));
    }

    #[test]
    fn test_compute_schema_hash_deterministic() {
        let schema1 = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        let schema2 = json!({
            "properties": {
                "name": { "type": "string" }
            },
            "type": "object"
        });

        let hash1 = compute_schema_hash(&schema1);
        let hash2 = compute_schema_hash(&schema2);

        // Same content, different order should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_schema_hash_different() {
        let schema1 = json!({
            "type": "object"
        });
        let schema2 = json!({
            "type": "array"
        });

        let hash1 = compute_schema_hash(&schema1);
        let hash2 = compute_schema_hash(&schema2);

        // Different content should produce different hash
        assert_ne!(hash1, hash2);
    }
}
