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
