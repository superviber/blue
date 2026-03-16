//! Post-Mortem tool handlers
//!
//! Handles post-mortem creation and action item tracking.

use std::fs;
use std::path::PathBuf;

use blue_core::{title_to_slug, DocType, Document, ProjectState, Rfc};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Severity levels for post-mortems
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    P1, // Critical - major outage
    P2, // High - significant impact
    P3, // Medium - moderate impact
    P4, // Low - minor impact
}

impl Severity {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "P1" | "CRITICAL" => Some(Severity::P1),
            "P2" | "HIGH" => Some(Severity::P2),
            "P3" | "MEDIUM" => Some(Severity::P3),
            "P4" | "LOW" => Some(Severity::P4),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::P1 => "P1",
            Severity::P2 => "P2",
            Severity::P3 => "P3",
            Severity::P4 => "P4",
        }
    }
}

/// Handle blue_postmortem_create
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let severity_str = args
        .get("severity")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let severity = Severity::parse(severity_str).ok_or_else(|| {
        ServerError::CommandFailed(format!(
            "Invalid severity '{}'. Use P1, P2, P3, or P4.",
            severity_str
        ))
    })?;

    let summary = args.get("summary").and_then(|v| v.as_str());
    let root_cause = args.get("root_cause").and_then(|v| v.as_str());
    let duration = args.get("duration").and_then(|v| v.as_str());

    let impact: Vec<String> = args
        .get("impact")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Get next postmortem number
    let pm_number = state
        .store
        .next_number_with_fs(DocType::Postmortem, &state.home.docs_path)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Generate file path with ISO 8601 timestamp prefix (RFC 0031)
    let timestamp = blue_core::utc_timestamp();
    let file_name = format!("{}-{}.open.md", timestamp, title_to_slug(title));
    let file_path = PathBuf::from("postmortems").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let pm_path = docs_path.join(&file_path);

    // Generate markdown content
    let markdown =
        generate_postmortem_markdown(title, severity, summary, root_cause, duration, &impact);

    // Create postmortems directory if it doesn't exist
    if let Some(parent) = pm_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }

    // Overwrite protection (RFC 0031)
    if pm_path.exists() {
        return Err(ServerError::CommandFailed(format!(
            "File already exists: {}",
            pm_path.display()
        )));
    }

    // Create document in SQLite store
    let doc = Document {
        id: None,
        doc_type: DocType::Postmortem,
        number: Some(pm_number),
        title: title.to_string(),
        status: "open".to_string(),
        file_path: Some(file_path.to_string_lossy().to_string()),
        created_at: None,
        updated_at: None,
        deleted_at: None,
        content_hash: Some(blue_core::store::hash_content(&markdown)),
        indexed_at: None,
    };
    state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    fs::write(&pm_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    let hint = "Post-mortem created. Fill in the timeline and lessons learned sections.";

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Post-mortem created: {}", title),
            Some(hint)
        ),
        "title": title,
        "severity": severity.as_str(),
        "file": pm_path.display().to_string(),
        "content": markdown,
    }))
}

/// Handle blue_postmortem_action_to_rfc
pub fn handle_action_to_rfc(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let postmortem_title = args
        .get("postmortem_title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let rfc_title_override = args.get("rfc_title").and_then(|v| v.as_str());

    // Find the post-mortem
    let pm_doc = state
        .store
        .find_document(DocType::Postmortem, postmortem_title)
        .map_err(|_| {
            ServerError::NotFound(format!("Post-mortem '{}' not found", postmortem_title))
        })?;

    let pm_file_path = pm_doc
        .file_path
        .as_ref()
        .ok_or_else(|| ServerError::CommandFailed("Post-mortem has no file path".to_string()))?;

    let docs_path = state.home.docs_path.clone();
    let pm_path = docs_path.join(pm_file_path);
    let pm_content = fs::read_to_string(&pm_path)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to read post-mortem: {}", e)))?;

    // Find the action item
    let (action_idx, action_description) = find_action_item(&pm_content, action)?;

    // Generate RFC title from action item if not provided
    let rfc_title = rfc_title_override.map(String::from).unwrap_or_else(|| {
        action_description
            .chars()
            .take(50)
            .collect::<String>()
            .trim()
            .to_string()
    });

    // Create RFC with post-mortem reference
    let mut rfc = Rfc::new(&rfc_title);
    rfc.problem = Some(format!(
        "From post-mortem: {}\n\nAction item: {}",
        postmortem_title, action_description
    ));

    // Get next RFC number
    let rfc_number = state
        .store
        .next_number_with_fs(DocType::Rfc, &state.home.docs_path)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Generate file path
    let rfc_file_name = format!("{:04}-{}.draft.md", rfc_number, title_to_slug(&rfc_title));
    let rfc_file_path = PathBuf::from("rfcs").join(&rfc_file_name);
    let rfc_path = docs_path.join(&rfc_file_path);

    // Generate RFC markdown with post-mortem link
    let mut markdown = rfc.to_markdown(rfc_number as u32);

    // Add source post-mortem link
    let pm_link = format!(
        "| **Source Post-Mortem** | [{}](../postmortems/{}) |",
        postmortem_title,
        pm_file_path.replace("postmortems/", "")
    );
    markdown = markdown.replace(
        "| **Status** | Draft |",
        &format!("| **Status** | Draft |\n{}", pm_link),
    );

    // Create RFC document in store
    let rfc_doc = Document {
        id: None,
        doc_type: DocType::Rfc,
        number: Some(rfc_number),
        title: rfc_title.clone(),
        status: "draft".to_string(),
        file_path: Some(rfc_file_path.to_string_lossy().to_string()),
        created_at: None,
        updated_at: None,
        deleted_at: None,
        content_hash: Some(blue_core::store::hash_content(&markdown)),
        indexed_at: None,
    };
    state
        .store
        .add_document(&rfc_doc)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Write RFC file
    if let Some(parent) = rfc_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }
    fs::write(&rfc_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Update post-mortem action item with RFC link
    let updated_pm_content = update_action_item_with_rfc(
        &pm_content,
        action_idx,
        &format!("RFC {:04}: {}", rfc_number, rfc_title),
    );
    fs::write(&pm_path, updated_pm_content)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to update post-mortem: {}", e)))?;

    let hint = format!(
        "RFC created from post-mortem action item. Review and expand the design: {}",
        rfc_path.display()
    );

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("RFC {:04} created from post-mortem", rfc_number),
            Some(&hint)
        ),
        "rfc_title": rfc_title,
        "rfc_number": rfc_number,
        "rfc_file": rfc_path.display().to_string(),
        "source_postmortem": postmortem_title,
        "action_item": action_description,
        "action_index": action_idx,
    }))
}

/// Find action item by index or substring
fn find_action_item(content: &str, identifier: &str) -> Result<(usize, String), ServerError> {
    let actions = parse_all_actions(content);

    // Try to parse as index first
    if let Ok(idx) = identifier.parse::<usize>() {
        if idx > 0 && idx <= actions.len() {
            return Ok((idx, actions[idx - 1].clone()));
        }
        return Err(ServerError::NotFound(format!(
            "Action item #{} not found. Found {} action items.",
            idx,
            actions.len()
        )));
    }

    // Try substring match
    for (i, action) in actions.iter().enumerate() {
        if action.to_lowercase().contains(&identifier.to_lowercase()) {
            return Ok((i + 1, action.clone()));
        }
    }

    Err(ServerError::NotFound(format!(
        "No action item matching '{}' found",
        identifier
    )))
}

/// Parse all action items from post-mortem content
fn parse_all_actions(content: &str) -> Vec<String> {
    let mut actions = Vec::new();
    let mut in_actions_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('#') && trimmed.to_lowercase().contains("action item") {
            in_actions_section = true;
            continue;
        }

        if in_actions_section && trimmed.starts_with('#') {
            break;
        }

        if in_actions_section && trimmed.starts_with('|') && !trimmed.contains("---") {
            let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 2 && !parts[1].is_empty() && parts[1] != "Item" {
                actions.push(parts[1].to_string());
            }
        }
    }

    actions
}

/// Update action item row with RFC link
fn update_action_item_with_rfc(content: &str, action_idx: usize, rfc_ref: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(String::from).collect();
    let mut in_actions_section = false;
    let mut current_action = 0;

    for line in lines.iter_mut() {
        let trimmed = line.trim();

        if trimmed.starts_with('#') && trimmed.to_lowercase().contains("action item") {
            in_actions_section = true;
            continue;
        }

        if in_actions_section && trimmed.starts_with('#') {
            break;
        }

        if in_actions_section && trimmed.starts_with('|') && !trimmed.contains("---") {
            let parts: Vec<&str> = trimmed.split('|').map(|s| s.trim()).collect();
            if parts.len() >= 2 && !parts[1].is_empty() && parts[1] != "Item" {
                current_action += 1;
                if current_action == action_idx {
                    // Update the RFC column (last column)
                    let mut new_parts: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
                    if new_parts.len() > 5 {
                        new_parts[5] = rfc_ref.to_string();
                    } else {
                        while new_parts.len() <= 5 {
                            new_parts.push(String::new());
                        }
                        new_parts[5] = rfc_ref.to_string();
                    }
                    *line = format!("| {} |", new_parts[1..].join(" | "));
                }
            }
        }
    }

    lines.join("\n")
}

/// Generate post-mortem markdown content
fn generate_postmortem_markdown(
    title: &str,
    severity: Severity,
    summary: Option<&str>,
    root_cause: Option<&str>,
    duration: Option<&str>,
    impact: &[String],
) -> String {
    let mut md = String::new();
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Title
    md.push_str(&format!("# Post-Mortem: {}\n\n", to_title_case(title)));

    // Metadata table
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!("| **Date** | {} |\n", date));
    md.push_str(&format!("| **Severity** | {} |\n", severity.as_str()));
    if let Some(dur) = duration {
        md.push_str(&format!("| **Duration** | {} |\n", dur));
    }
    md.push_str("| **Author** | [Name] |\n");
    md.push_str("\n---\n\n");

    // Summary
    md.push_str("## Summary\n\n");
    if let Some(sum) = summary {
        md.push_str(sum);
    } else {
        md.push_str("[One paragraph summary of the incident]");
    }
    md.push_str("\n\n");

    // Timeline
    md.push_str("## Timeline\n\n");
    md.push_str("| Time | Event |\n");
    md.push_str("|------|-------|\n");
    md.push_str("| HH:MM | [Event] |\n");
    md.push('\n');

    // Root Cause
    md.push_str("## Root Cause\n\n");
    if let Some(rc) = root_cause {
        md.push_str(rc);
    } else {
        md.push_str("[What actually caused the incident]");
    }
    md.push_str("\n\n");

    // Impact
    md.push_str("## Impact\n\n");
    if !impact.is_empty() {
        for item in impact {
            md.push_str(&format!("- {}\n", item));
        }
    } else {
        md.push_str("- [Impact 1]\n");
    }
    md.push('\n');

    // What Went Well
    md.push_str("## What Went Well\n\n");
    md.push_str("- [Item 1]\n\n");

    // What Went Wrong
    md.push_str("## What Went Wrong\n\n");
    md.push_str("- [Item 1]\n\n");

    // Action Items
    md.push_str("## Action Items\n\n");
    md.push_str("| Item | Owner | Due | Status | RFC |\n");
    md.push_str("|------|-------|-----|--------|-----|\n");
    md.push_str("| [Action 1] | [Name] | [Date] | Open | |\n");
    md.push('\n');

    // Lessons Learned
    md.push_str("## Lessons Learned\n\n");
    md.push_str("[Key takeaways from this incident]\n");

    md
}

/// Convert slug to title case
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_from_str() {
        assert_eq!(Severity::parse("P1"), Some(Severity::P1));
        assert_eq!(Severity::parse("critical"), Some(Severity::P1));
        assert_eq!(Severity::parse("P4"), Some(Severity::P4));
        assert_eq!(Severity::parse("invalid"), None);
    }

    #[test]
    fn test_title_to_slug() {
        assert_eq!(title_to_slug("Database Outage"), "database-outage");
        assert_eq!(title_to_slug("API failure"), "api-failure");
    }
}
