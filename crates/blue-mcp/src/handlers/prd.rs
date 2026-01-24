//! PRD tool handlers
//!
//! Handles PRD (Product Requirements Document) creation, approval, and completion.
//! PRDs capture "What & Why" before RFCs define "How".

use std::fs;

use blue_core::{DocType, Document, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Handle blue_prd_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let problem = args.get("problem").and_then(|v| v.as_str());
    let users = args.get("users").and_then(|v| v.as_str());
    let goals: Option<Vec<String>> = args
        .get("goals")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
    let non_goals: Option<Vec<String>> = args
        .get("non_goals")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
    let stakeholders: Option<Vec<String>> = args
        .get("stakeholders")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

    // Get next PRD number
    let prd_number = state
        .store
        .next_number(DocType::Prd)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Generate file path
    let file_name = format!("{:04}-{}.md", prd_number, to_kebab_case(title));
    let file_path = state.home.docs_path(&state.project).join("prds").join(&file_name);

    // Ensure directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }

    // Generate markdown
    let markdown = generate_prd_markdown(title, prd_number as i64, problem, users, &goals, &non_goals, &stakeholders);

    // Write file
    fs::write(&file_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Add to store
    let doc = Document::new(DocType::Prd, title, "draft");
    state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Created PRD {:04} '{}'", prd_number, title),
            Some("Add user stories with acceptance criteria, then use blue_prd_approve when ready.")
        ),
        "prd": {
            "title": title,
            "number": prd_number,
            "path": file_path.display().to_string(),
            "status": "draft"
        }
    }))
}

/// Handle blue_prd_get
pub fn handle_get(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Prd, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Read file content
    let rel_path = doc.file_path.as_ref()
        .ok_or_else(|| ServerError::CommandFailed("PRD file path not set".to_string()))?;
    let file_path = state.home.docs_path(&state.project).join(rel_path);
    let content = fs::read_to_string(&file_path)
        .map_err(|e| ServerError::CommandFailed(format!("Couldn't read PRD: {}", e)))?;

    // Parse acceptance criteria
    let criteria = parse_acceptance_criteria(&content);
    let checked = criteria.iter().filter(|(_, c)| *c).count();
    let total = criteria.len();

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("PRD '{}' - {}", doc.title, doc.status),
            Some(&format!("{}/{} acceptance criteria met", checked, total))
        ),
        "prd": {
            "title": doc.title,
            "number": doc.number,
            "status": doc.status,
            "path": doc.file_path,
            "content": content
        },
        "acceptance_criteria": {
            "total": total,
            "checked": checked,
            "items": criteria.iter().map(|(text, checked)| json!({
                "text": text,
                "checked": checked
            })).collect::<Vec<_>>()
        }
    }))
}

/// Handle blue_prd_approve
pub fn handle_approve(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Prd, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if doc.status != "draft" {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Can't approve - PRD is '{}'", doc.status),
                "Can only approve from 'draft' status"
            )
        }));
    }

    state
        .store
        .update_document_status(DocType::Prd, title, "approved")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Approved PRD '{}'", title),
            Some("Create RFC(s) to implement: blue_rfc_create with source_prd")
        ),
        "prd": {
            "title": title,
            "status": "approved"
        }
    }))
}

/// Handle blue_prd_complete
pub fn handle_complete(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let doc = state
        .store
        .find_document(DocType::Prd, title)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    if doc.status != "approved" {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                &format!("Can't complete - PRD is '{}'", doc.status),
                "Can only complete from 'approved' status"
            )
        }));
    }

    // Check acceptance criteria
    let empty_path = String::new();
    let rel_path = doc.file_path.as_ref().unwrap_or(&empty_path);
    let file_path = state.home.docs_path(&state.project).join(rel_path);
    let content = fs::read_to_string(&file_path).unwrap_or_default();
    let criteria = parse_acceptance_criteria(&content);
    let unchecked: Vec<_> = criteria.iter().filter(|(_, c)| !c).collect();

    if !unchecked.is_empty() {
        return Ok(json!({
            "status": "blocked",
            "message": blue_core::voice::error(
                &format!("{} unchecked acceptance criteria", unchecked.len()),
                "Mark them complete before finishing"
            ),
            "unchecked": unchecked.iter().map(|(text, _)| text).collect::<Vec<_>>()
        }));
    }

    state
        .store
        .update_document_status(DocType::Prd, title, "implemented")
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Completed PRD '{}'", title),
            Some("All acceptance criteria verified!")
        ),
        "prd": {
            "title": title,
            "status": "implemented"
        }
    }))
}

/// Handle blue_prd_list
pub fn handle_list(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let status_filter = args.get("status").and_then(|v| v.as_str());

    let docs = state
        .store
        .list_documents(DocType::Prd)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let filtered: Vec<_> = docs
        .into_iter()
        .filter(|d| {
            status_filter
                .map(|s| d.status.eq_ignore_ascii_case(s))
                .unwrap_or(true)
        })
        .collect();

    let hint = if filtered.is_empty() {
        "No PRDs found. Create one with blue_prd_create."
    } else {
        "Use blue_prd_get to view details."
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("{} PRD(s)", filtered.len()),
            Some(hint)
        ),
        "prds": filtered.iter().map(|d| json!({
            "title": d.title,
            "number": d.number,
            "status": d.status,
            "path": d.file_path
        })).collect::<Vec<_>>(),
        "count": filtered.len()
    }))
}

// ===== Helper Functions =====

fn to_kebab_case(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn generate_prd_markdown(
    title: &str,
    number: i64,
    problem: Option<&str>,
    users: Option<&str>,
    goals: &Option<Vec<String>>,
    non_goals: &Option<Vec<String>>,
    stakeholders: &Option<Vec<String>>,
) -> String {
    let stakeholders_str = stakeholders
        .as_ref()
        .map(|s| s.join(", "))
        .unwrap_or_else(|| "[Stakeholders]".to_string());

    let problem_str = problem.unwrap_or("[What problem are users experiencing?]");
    let users_str = users.unwrap_or("[Who are the target users?]");

    let goals_str = goals
        .as_ref()
        .map(|g| g.iter().map(|x| format!("- {}", x)).collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|| "- [Business goal 1]\n- [Business goal 2]".to_string());

    let non_goals_str = non_goals
        .as_ref()
        .map(|g| g.iter().map(|x| format!("- {}", x)).collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|| "- [What this feature won't do]".to_string());

    format!(
        r#"# PRD {:04}: {}

| | |
|---|---|
| **Status** | Draft |
| **Author** | [Author] |
| **Created** | {} |
| **Stakeholders** | {} |

---

## Problem

{}

## Users

{}

## Goals

{}

## Non-Goals

{}

## User Stories

### Story 1: [Title]

**As a** [user type]
**I want to** [action]
**So that** [benefit]

**Acceptance Criteria**:
- [ ] [Criterion 1]
- [ ] [Criterion 2]

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| [Metric 1] | [Value] | [Value] |

## Open Questions

- [Question 1]

---

*"Right then. What are we building, and why?"*

— Blue
"#,
        number,
        title,
        chrono::Utc::now().format("%Y-%m-%d"),
        stakeholders_str,
        problem_str,
        users_str,
        goals_str,
        non_goals_str,
    )
}

fn parse_acceptance_criteria(content: &str) -> Vec<(String, bool)> {
    let mut criteria = Vec::new();
    let mut in_section = false;

    for line in content.lines() {
        if line.contains("**Acceptance Criteria**") {
            in_section = true;
            continue;
        }
        if in_section && (line.starts_with("##") || line.starts_with("### Story")) {
            in_section = false;
        }
        if in_section {
            if let Some(text) = line.strip_prefix("- [x] ").or_else(|| line.strip_prefix("- [X] ")) {
                criteria.push((text.trim().to_string(), true));
            } else if let Some(text) = line.strip_prefix("- [ ] ") {
                criteria.push((text.trim().to_string(), false));
            }
        }
    }
    criteria
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("Hello World"), "hello-world");
        assert_eq!(to_kebab_case("user-auth"), "user-auth");
    }

    #[test]
    fn test_parse_acceptance_criteria() {
        let content = r#"
**Acceptance Criteria**:
- [x] Done item
- [ ] Pending item
## Next Section
"#;
        let criteria = parse_acceptance_criteria(content);
        assert_eq!(criteria.len(), 2);
        assert!(criteria[0].1);
        assert!(!criteria[1].1);
    }
}
