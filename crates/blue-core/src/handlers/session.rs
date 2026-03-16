//! Session tool handlers
//!
//! Handles session management for multi-agent coordination.

use crate::{DocType, ProjectState, Session, SessionType};
use serde_json::{json, Value};

use crate::handler_error::HandlerError;

/// Handle blue_session_ping
pub fn handle_ping(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let session_type_str = args
        .get("session_type")
        .and_then(|v| v.as_str())
        .unwrap_or("implementation");

    let session_type = SessionType::parse(session_type_str).unwrap_or(SessionType::Implementation);

    // Verify the RFC exists
    let rfc = match state.store.find_document(DocType::Rfc, title) {
        Ok(doc) => doc,
        Err(_) => {
            return Ok(json!({
                "status": "error",
                "message": crate::voice::error(
                    &format!("Can't find RFC '{}'", title),
                    "Check the title's spelled right?"
                )
            }));
        }
    };

    match action {
        "start" => handle_start(state, &rfc.title, session_type),
        "heartbeat" => handle_heartbeat(state, &rfc.title, session_type),
        "end" => handle_end(state, &rfc.title),
        _ => Ok(json!({
            "status": "error",
            "message": crate::voice::error(
                &format!("Unknown action '{}'", action),
                "Use 'start', 'heartbeat', or 'end'"
            )
        })),
    }
}

fn handle_start(
    state: &ProjectState,
    title: &str,
    session_type: SessionType,
) -> Result<Value, HandlerError> {
    // Check for existing session
    match state.store.get_active_session(title) {
        Ok(Some(existing)) => {
            return Ok(json!({
                "status": "warning",
                "message": crate::voice::info(
                    &format!("Session already active for '{}'", title),
                    Some(&format!("Started at {}, type: {}", existing.started_at, existing.session_type.as_str()))
                ),
                "session": {
                    "rfc_title": existing.rfc_title,
                    "session_type": existing.session_type.as_str(),
                    "started_at": existing.started_at,
                    "last_heartbeat": existing.last_heartbeat
                }
            }));
        }
        Ok(None) => {}
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": crate::voice::error(
                    "Couldn't check for existing sessions",
                    &e.to_string()
                )
            }));
        }
    }

    let session = Session {
        id: None,
        rfc_title: title.to_string(),
        session_type,
        started_at: String::new(),
        last_heartbeat: String::new(),
        ended_at: None,
    };

    match state.store.upsert_session(&session) {
        Ok(_) => Ok(json!({
            "status": "success",
            "message": crate::voice::success(
                &format!("Started {} session for '{}'", session_type.as_str(), title),
                Some("I'll keep an eye on things. Remember to send heartbeats!")
            ),
            "session": {
                "rfc_title": title,
                "session_type": session_type.as_str()
            }
        })),
        Err(e) => Ok(json!({
            "status": "error",
            "message": crate::voice::error(
                "Couldn't start session",
                &e.to_string()
            )
        })),
    }
}

fn handle_heartbeat(
    state: &ProjectState,
    title: &str,
    session_type: SessionType,
) -> Result<Value, HandlerError> {
    let session = Session {
        id: None,
        rfc_title: title.to_string(),
        session_type,
        started_at: String::new(),
        last_heartbeat: String::new(),
        ended_at: None,
    };

    match state.store.upsert_session(&session) {
        Ok(_) => Ok(json!({
            "status": "success",
            "message": crate::voice::success(
                &format!("Heartbeat recorded for '{}'", title),
                None::<&str>
            ),
            "session": {
                "rfc_title": title,
                "session_type": session_type.as_str()
            }
        })),
        Err(e) => Ok(json!({
            "status": "error",
            "message": crate::voice::error(
                "Couldn't record heartbeat",
                &e.to_string()
            )
        })),
    }
}

fn handle_end(state: &ProjectState, title: &str) -> Result<Value, HandlerError> {
    match state.store.end_session(title) {
        Ok(_) => Ok(json!({
            "status": "success",
            "message": crate::voice::success(
                &format!("Session ended for '{}'", title),
                Some("Good work! The RFC is free for others now.")
            )
        })),
        Err(e) => Ok(json!({
            "status": "error",
            "message": crate::voice::error(
                &format!("No active session for '{}'", title),
                &e.to_string()
            )
        })),
    }
}

/// Handle blue_session_list (list active sessions)
pub fn handle_list(state: &ProjectState, _args: &Value) -> Result<Value, HandlerError> {
    // First, clean up stale sessions (older than 5 minutes)
    let cleaned = state.store.cleanup_stale_sessions(5).unwrap_or(0);

    let sessions = state.store.list_active_sessions().unwrap_or_default();

    if sessions.is_empty() {
        return Ok(json!({
            "status": "success",
            "message": crate::voice::info(
                "No active sessions",
                Some("The workspace is quiet. Good time to start something!")
            ),
            "sessions": [],
            "stale_cleaned": cleaned
        }));
    }

    let session_list: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "rfc_title": s.rfc_title,
                "session_type": s.session_type.as_str(),
                "started_at": s.started_at,
                "last_heartbeat": s.last_heartbeat
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "message": crate::voice::info(
            &format!("{} active session{}", sessions.len(), if sessions.len() == 1 { "" } else { "s" }),
            None::<&str>
        ),
        "sessions": session_list,
        "stale_cleaned": cleaned
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_requires_rfc() {
        let state = ProjectState::for_test();
        let args = json!({
            "title": "nonexistent-rfc",
            "action": "start"
        });

        let result = handle_ping(&state, &args).unwrap();
        assert_eq!(result["status"], "error");
    }

    #[test]
    fn test_session_invalid_action() {
        let state = ProjectState::for_test();

        // Create an RFC first
        let doc = crate::Document::new(DocType::Rfc, "test-rfc", "draft");
        state.store.add_document(&doc).unwrap();

        let args = json!({
            "title": "test-rfc",
            "action": "invalid"
        });

        let result = handle_ping(&state, &args).unwrap();
        assert_eq!(result["status"], "error");
    }
}
