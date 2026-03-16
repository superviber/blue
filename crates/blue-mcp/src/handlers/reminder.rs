//! Reminder tool handlers
//!
//! Handles reminder CRUD with gates, snoozing, and clearing.

use blue_core::{DocType, ProjectState, Reminder, ReminderStatus};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_reminder_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let context = args.get("context").and_then(|v| v.as_str());
    let gate = args.get("gate").and_then(|v| v.as_str());
    let due_date = args.get("due_date").and_then(|v| v.as_str());
    let snooze_until = args.get("snooze_until").and_then(|v| v.as_str());
    let link_to = args.get("link_to").and_then(|v| v.as_str());

    // Find linked document if specified
    let linked_doc_id = if let Some(link_title) = link_to {
        // Try RFC first, then spike, then others
        let doc = state
            .store
            .find_document(DocType::Rfc, link_title)
            .or_else(|_| state.store.find_document(DocType::Spike, link_title))
            .or_else(|_| state.store.find_document(DocType::Decision, link_title));

        match doc {
            Ok(d) => d.id,
            Err(_) => {
                return Ok(json!({
                    "status": "error",
                    "message": blue_core::voice::error(
                        &format!("Can't find document '{}'", link_title),
                        "Check the title's spelled right?"
                    )
                }));
            }
        }
    } else {
        None
    };

    let mut reminder = Reminder::new(title);
    reminder.context = context.map(|s| s.to_string());
    reminder.gate = gate.map(|s| s.to_string());
    reminder.due_date = due_date.map(|s| s.to_string());
    reminder.snooze_until = snooze_until.map(|s| s.to_string());
    reminder.linked_doc_id = linked_doc_id;

    if snooze_until.is_some() {
        reminder.status = ReminderStatus::Snoozed;
    }

    match state.store.add_reminder(&reminder) {
        Ok(id) => {
            let hint = match (&reminder.gate, &reminder.due_date) {
                (Some(g), Some(d)) => format!("Gate: '{}', Due: {}", g, d),
                (Some(g), None) => format!("Gate: '{}'", g),
                (None, Some(d)) => format!("Due: {}", d),
                (None, None) => "No gate or due date set".to_string(),
            };

            Ok(json!({
                "status": "success",
                "message": blue_core::voice::success(
                    &format!("Created reminder: '{}'", title),
                    Some(&hint)
                ),
                "reminder": {
                    "id": id,
                    "title": title,
                    "gate": reminder.gate,
                    "due_date": reminder.due_date,
                    "snooze_until": reminder.snooze_until,
                    "linked_to": link_to
                }
            }))
        }
        Err(e) => Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "Couldn't create reminder",
                &e.to_string()
            )
        })),
    }
}

/// Handle blue_reminder_list
pub fn handle_list(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let status_filter = args.get("status").and_then(|v| v.as_str());
    let include_future = args
        .get("include_future")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let status = status_filter.and_then(|s| match s {
        "pending" => Some(ReminderStatus::Pending),
        "snoozed" => Some(ReminderStatus::Snoozed),
        "cleared" => Some(ReminderStatus::Cleared),
        "all" => None,
        _ => Some(ReminderStatus::Pending),
    });

    let reminders = state
        .store
        .list_reminders(status, include_future)
        .unwrap_or_default();

    if reminders.is_empty() {
        let msg = match status_filter {
            Some("cleared") => "No cleared reminders",
            Some("snoozed") => "No snoozed reminders",
            _ => "No pending reminders",
        };
        return Ok(json!({
            "status": "success",
            "message": blue_core::voice::info(msg, Some("Clear skies ahead!")),
            "reminders": []
        }));
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let reminder_list: Vec<Value> = reminders
        .iter()
        .map(|r| {
            let is_due = r.due_date.as_ref().map(|d| d <= &today).unwrap_or(false);
            let is_overdue = r.due_date.as_ref().map(|d| d < &today).unwrap_or(false);

            json!({
                "id": r.id,
                "title": r.title,
                "context": r.context,
                "gate": r.gate,
                "due_date": r.due_date,
                "snooze_until": r.snooze_until,
                "status": r.status.as_str(),
                "is_due": is_due,
                "is_overdue": is_overdue
            })
        })
        .collect();

    let due_count = reminder_list
        .iter()
        .filter(|r| r["is_due"].as_bool().unwrap_or(false))
        .count();
    let overdue_count = reminder_list
        .iter()
        .filter(|r| r["is_overdue"].as_bool().unwrap_or(false))
        .count();

    let hint = if overdue_count > 0 {
        Some(format!("{} overdue!", overdue_count))
    } else if due_count > 0 {
        Some(format!("{} due today", due_count))
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("{} reminder{}", reminders.len(), if reminders.len() == 1 { "" } else { "s" }),
            hint.as_deref()
        ),
        "reminders": reminder_list,
        "due_count": due_count,
        "overdue_count": overdue_count
    }))
}

/// Handle blue_reminder_snooze
pub fn handle_snooze(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let until = args
        .get("until")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    // Find reminder by ID or title
    let reminder = if let Some(id) = args.get("id").and_then(|v| v.as_i64()) {
        state.store.get_reminder(id)
    } else if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
        state.store.find_reminder(title)
    } else {
        return Err(ServerError::InvalidParams);
    };

    let reminder = match reminder {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Can't find that reminder",
                    &e.to_string()
                )
            }));
        }
    };

    let id = reminder.id.unwrap();

    match state.store.snooze_reminder(id, until) {
        Ok(_) => Ok(json!({
            "status": "success",
            "message": blue_core::voice::success(
                &format!("Snoozed '{}' until {}", reminder.title, until),
                Some("I'll remind you then!")
            ),
            "reminder": {
                "id": id,
                "title": reminder.title,
                "snooze_until": until
            }
        })),
        Err(e) => Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "Couldn't snooze reminder",
                &e.to_string()
            )
        })),
    }
}

/// Handle blue_reminder_clear
pub fn handle_clear(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let resolution = args.get("resolution").and_then(|v| v.as_str());

    // Find reminder by ID or title
    let reminder = if let Some(id) = args.get("id").and_then(|v| v.as_i64()) {
        state.store.get_reminder(id)
    } else if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
        state.store.find_reminder(title)
    } else {
        return Err(ServerError::InvalidParams);
    };

    let reminder = match reminder {
        Ok(r) => r,
        Err(e) => {
            return Ok(json!({
                "status": "error",
                "message": blue_core::voice::error(
                    "Can't find that reminder",
                    &e.to_string()
                )
            }));
        }
    };

    let id = reminder.id.unwrap();

    match state.store.clear_reminder(id, resolution) {
        Ok(_) => {
            let hint = resolution.map(|r| format!("Resolution: {}", r));
            Ok(json!({
                "status": "success",
                "message": blue_core::voice::success(
                    &format!("Cleared '{}'", reminder.title),
                    hint.as_deref()
                ),
                "reminder": {
                    "id": id,
                    "title": reminder.title,
                    "resolution": resolution
                }
            }))
        }
        Err(e) => Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "Couldn't clear reminder",
                &e.to_string()
            )
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_reminder() {
        let state = ProjectState::for_test();
        let args = json!({
            "title": "Test reminder",
            "context": "Some context",
            "gate": "Wait for approval"
        });

        let result = handle_create(&state, &args).unwrap();
        assert_eq!(result["status"], "success");
        assert!(result["reminder"]["id"].is_number());
    }

    #[test]
    fn test_list_empty() {
        let state = ProjectState::for_test();
        let args = json!({});

        let result = handle_list(&state, &args).unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(result["reminders"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_snooze_requires_until() {
        let state = ProjectState::for_test();
        let args = json!({
            "id": 1
        });

        let result = handle_snooze(&state, &args);
        assert!(result.is_err());
    }
}
