//! Staging lock tool handlers
//!
//! Handles staging environment isolation through resource locking.
//! Ensures single-writer access to staging resources like migrations.

use blue_core::{ProjectState, StagingLockResult};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_staging_lock
pub fn handle_lock(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let resource = args
        .get("resource")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let locked_by = args
        .get("locked_by")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let agent_id = args.get("agent_id").and_then(|v| v.as_str());
    let duration_minutes = args
        .get("duration_minutes")
        .and_then(|v| v.as_i64())
        .unwrap_or(30);

    let result = state
        .store
        .acquire_staging_lock(resource, locked_by, agent_id, duration_minutes)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    match result {
        StagingLockResult::Acquired { expires_at } => Ok(json!({
            "status": "acquired",
            "message": blue_core::voice::success(
                &format!("Acquired staging lock for '{}'", resource),
                Some(&format!("Expires at {}. Release with blue_staging_unlock when done.", expires_at))
            ),
            "resource": resource,
            "locked_by": locked_by,
            "expires_at": expires_at
        })),
        StagingLockResult::Queued {
            position,
            current_holder,
            expires_at,
        } => Ok(json!({
            "status": "queued",
            "message": blue_core::voice::info(
                &format!("Resource '{}' is locked by '{}'", resource, current_holder),
                Some(&format!("You're #{} in queue. Lock expires at {}.", position, expires_at))
            ),
            "resource": resource,
            "queue_position": position,
            "current_holder": current_holder,
            "holder_expires_at": expires_at
        })),
    }
}

/// Handle blue_staging_unlock
pub fn handle_unlock(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let resource = args
        .get("resource")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let locked_by = args
        .get("locked_by")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let next_in_queue = state
        .store
        .release_staging_lock(resource, locked_by)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    let hint = match &next_in_queue {
        Some(next) => format!("Next in queue: '{}' can now acquire the lock.", next),
        None => "No one waiting in queue.".to_string(),
    };

    Ok(json!({
        "status": "released",
        "message": blue_core::voice::success(
            &format!("Released staging lock for '{}'", resource),
            Some(&hint)
        ),
        "resource": resource,
        "next_in_queue": next_in_queue
    }))
}

/// Handle blue_staging_status
pub fn handle_status(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let resource = args.get("resource").and_then(|v| v.as_str());

    if let Some(resource) = resource {
        // Get status for specific resource
        let lock = state
            .store
            .get_staging_lock(resource)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

        let queue = state
            .store
            .get_staging_lock_queue(resource)
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

        if let Some(lock) = lock {
            Ok(json!({
                "status": "locked",
                "message": blue_core::voice::info(
                    &format!("Resource '{}' is locked by '{}'", resource, lock.locked_by),
                    Some(&format!("Expires at {}. {} waiting in queue.", lock.expires_at, queue.len()))
                ),
                "resource": resource,
                "lock": {
                    "locked_by": lock.locked_by,
                    "agent_id": lock.agent_id,
                    "locked_at": lock.locked_at,
                    "expires_at": lock.expires_at
                },
                "queue": queue.iter().map(|q| json!({
                    "requester": q.requester,
                    "agent_id": q.agent_id,
                    "requested_at": q.requested_at
                })).collect::<Vec<_>>()
            }))
        } else {
            Ok(json!({
                "status": "available",
                "message": blue_core::voice::info(
                    &format!("Resource '{}' is available", resource),
                    None::<&str>
                ),
                "resource": resource,
                "lock": null,
                "queue": []
            }))
        }
    } else {
        // List all locks
        let locks = state
            .store
            .list_staging_locks()
            .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

        let hint = if locks.is_empty() {
            "No active staging locks. Resources are available."
        } else {
            "Use blue_staging_unlock to release locks when done."
        };

        Ok(json!({
            "status": "success",
            "message": blue_core::voice::info(
                &format!("{} active staging lock{}", locks.len(), if locks.len() == 1 { "" } else { "s" }),
                Some(hint)
            ),
            "locks": locks.iter().map(|l| json!({
                "resource": l.resource,
                "locked_by": l.locked_by,
                "agent_id": l.agent_id,
                "locked_at": l.locked_at,
                "expires_at": l.expires_at
            })).collect::<Vec<_>>()
        }))
    }
}

/// Handle blue_staging_cleanup
pub fn handle_cleanup(state: &ProjectState, _args: &Value) -> Result<Value, ServerError> {
    let (locks_cleaned, queue_cleaned) = state
        .store
        .cleanup_expired_staging()
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    let total = locks_cleaned + queue_cleaned;

    let hint = if total == 0 {
        "No expired staging resources found. All clean."
    } else {
        "Cleaned up expired resources."
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Cleaned {} expired lock{}, {} orphaned queue entr{}",
                locks_cleaned,
                if locks_cleaned == 1 { "" } else { "s" },
                queue_cleaned,
                if queue_cleaned == 1 { "y" } else { "ies" }
            ),
            Some(hint)
        ),
        "locks_cleaned": locks_cleaned,
        "queue_entries_cleaned": queue_cleaned,
        "total_cleaned": total
    }))
}

/// Handle blue_staging_create
///
/// Detects IaC in the project and generates staging deployment commands.
pub fn handle_create(args: &Value, repo_path: &std::path::Path) -> Result<Value, ServerError> {
    let path = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_path.to_path_buf());

    let ttl_hours = args
        .get("ttl_hours")
        .and_then(|v| v.as_u64())
        .unwrap_or(24);

    let stack = args.get("stack").and_then(|v| v.as_str());

    // Detect IaC type
    let (iac_type, deploy_command, stacks) = if path.join("cdk.json").exists() {
        let cmd = if let Some(s) = stack {
            format!("cdk deploy {} --context stage=staging", s)
        } else {
            "cdk deploy --all --context stage=staging".to_string()
        };
        ("cdk", cmd, detect_cdk_stacks(&path))
    } else if path.join("main.tf").exists() || path.join("terraform").is_dir() {
        let cmd = if let Some(s) = stack {
            format!("terraform apply -var=\"environment=staging\" -target=module.{}", s)
        } else {
            "terraform apply -var=\"environment=staging\"".to_string()
        };
        ("terraform", cmd, vec![])
    } else if path.join("Pulumi.yaml").exists() {
        let cmd = if let Some(s) = stack {
            format!("pulumi up --stack {}", s)
        } else {
            "pulumi up --stack staging".to_string()
        };
        ("pulumi", cmd, vec![])
    } else {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "No IaC detected",
                "Need cdk.json, main.tf, or Pulumi.yaml"
            )
        }));
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Staging deployment ready ({})", iac_type.to_uppercase()),
            Some(&format!("TTL: {} hours. Run the command to deploy.", ttl_hours))
        ),
        "iac_type": iac_type,
        "deploy_command": deploy_command,
        "stacks": stacks,
        "ttl_hours": ttl_hours,
        "instructions": format!(
            "To deploy:\n  {}\n\nAcquire staging lock first if running migrations.",
            deploy_command
        )
    }))
}

/// Handle blue_staging_destroy
pub fn handle_destroy(args: &Value, repo_path: &std::path::Path) -> Result<Value, ServerError> {
    let path = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_path.to_path_buf());

    let (iac_type, destroy_command) = if path.join("cdk.json").exists() {
        ("cdk", "cdk destroy --all --context stage=staging --force")
    } else if path.join("main.tf").exists() || path.join("terraform").is_dir() {
        ("terraform", "terraform destroy -var=\"environment=staging\" -auto-approve")
    } else if path.join("Pulumi.yaml").exists() {
        ("pulumi", "pulumi destroy --stack staging --yes")
    } else {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "No IaC detected",
                "Need cdk.json, main.tf, or Pulumi.yaml"
            )
        }));
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Staging destruction ready ({})", iac_type.to_uppercase()),
            Some("Run the command to destroy resources")
        ),
        "iac_type": iac_type,
        "destroy_command": destroy_command,
        "instructions": format!("To destroy:\n  {}", destroy_command)
    }))
}

/// Handle blue_staging_cost
pub fn handle_cost(args: &Value, repo_path: &std::path::Path) -> Result<Value, ServerError> {
    let path = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| repo_path.to_path_buf());

    let duration_hours = args
        .get("duration_hours")
        .and_then(|v| v.as_u64())
        .unwrap_or(24);

    let (iac_type, cost_command, instructions) = if path.join("cdk.json").exists() {
        (
            "cdk",
            "cdk synth && infracost breakdown --path cdk.out",
            "Install infracost and run cdk synth first"
        )
    } else if path.join("main.tf").exists() || path.join("terraform").is_dir() {
        (
            "terraform",
            "infracost breakdown --path .",
            "Install infracost: brew install infracost"
        )
    } else if path.join("Pulumi.yaml").exists() {
        (
            "pulumi",
            "# Pulumi doesn't have built-in cost estimation",
            "Use cloud provider cost calculators"
        )
    } else {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "No IaC detected",
                "Need cdk.json, main.tf, or Pulumi.yaml"
            )
        }));
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Cost estimation for {} hours", duration_hours),
            Some(instructions)
        ),
        "iac_type": iac_type,
        "cost_command": cost_command,
        "duration_hours": duration_hours,
        "instructions": instructions
    }))
}

fn detect_cdk_stacks(path: &std::path::Path) -> Vec<String> {
    let mut stacks = Vec::new();

    // Check lib/ for TypeScript stacks
    let lib_dir = path.join("lib");
    if lib_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                if file_path.extension().map(|e| e == "ts").unwrap_or(false) {
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        for line in content.lines() {
                            if line.contains("extends") && line.contains("Stack") {
                                if let Some(class_name) = line.split_whitespace()
                                    .skip_while(|&w| w != "class")
                                    .nth(1)
                                {
                                    let name = class_name.trim_end_matches('{').to_string();
                                    if !stacks.contains(&name) {
                                        stacks.push(name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    stacks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_requires_resource() {
        let state = ProjectState::for_test();
        let args = json!({
            "locked_by": "test-agent"
        });

        let result = handle_lock(&state, &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_lock_acquire_and_release() {
        let state = ProjectState::for_test();

        // Acquire lock
        let args = json!({
            "resource": "migration",
            "locked_by": "agent-1",
            "duration_minutes": 5
        });
        let result = handle_lock(&state, &args).unwrap();
        assert_eq!(result["status"], "acquired");

        // Try to acquire again - should queue
        let args2 = json!({
            "resource": "migration",
            "locked_by": "agent-2"
        });
        let result2 = handle_lock(&state, &args2).unwrap();
        assert_eq!(result2["status"], "queued");
        assert_eq!(result2["queue_position"], 1);

        // Release
        let release_args = json!({
            "resource": "migration",
            "locked_by": "agent-1"
        });
        let release_result = handle_unlock(&state, &release_args).unwrap();
        assert_eq!(release_result["status"], "released");
        assert_eq!(release_result["next_in_queue"], "agent-2");
    }

    #[test]
    fn test_status_no_locks() {
        let state = ProjectState::for_test();
        let args = json!({});

        let result = handle_status(&state, &args).unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(result["locks"].as_array().unwrap().len(), 0);
    }
}
