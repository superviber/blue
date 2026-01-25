//! Runbook tool handlers
//!
//! Handles runbook creation, updates, and action-based lookup with RFC linking.
//! Implements RFC 0002: Runbook Action Lookup.

use std::fs;
use std::path::PathBuf;

use blue_core::{DocType, Document, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

/// Metadata key for storing runbook actions
const ACTION_KEY: &str = "action";

/// Handle blue_runbook_create
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let source_rfc = args.get("source_rfc").and_then(|v| v.as_str());
    let service_name = args.get("service_name").and_then(|v| v.as_str());
    let owner = args.get("owner").and_then(|v| v.as_str());

    let operations: Vec<String> = args
        .get("operations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Parse actions array for runbook lookup (RFC 0002)
    let actions: Vec<String> = args
        .get("actions")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Validate source RFC exists if provided
    let source_rfc_doc = if let Some(rfc_title) = source_rfc {
        Some(
            state
                .store
                .find_document(DocType::Rfc, rfc_title)
                .map_err(|_| {
                    ServerError::NotFound(format!("Source RFC '{}' not found", rfc_title))
                })?,
        )
    } else {
        None
    };

    // Get next runbook number
    let runbook_number = state
        .store
        .next_number(DocType::Runbook)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Generate file path
    let file_name = format!("{}.md", to_kebab_case(title));
    let file_path = PathBuf::from("runbooks").join(&file_name);
    let docs_path = state.home.docs_path.clone();
    let runbook_path = docs_path.join(&file_path);

    // Generate markdown content (with actions for RFC 0002)
    let markdown = generate_runbook_markdown(title, &source_rfc_doc, service_name, owner, &operations, &actions);

    // Create document in SQLite store
    let doc = Document {
        id: None,
        doc_type: DocType::Runbook,
        number: Some(runbook_number),
        title: title.to_string(),
        status: "active".to_string(),
        file_path: Some(file_path.to_string_lossy().to_string()),
        created_at: None,
        updated_at: None,
        deleted_at: None,
    };
    let doc_id = state
        .store
        .add_document(&doc)
        .map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Store actions in metadata table (RFC 0002)
    for action in &actions {
        let _ = state.store.conn().execute(
            "INSERT OR IGNORE INTO metadata (document_id, key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![doc_id, ACTION_KEY, action.to_lowercase()],
        );
    }

    // Write the markdown file
    if let Some(parent) = runbook_path.parent() {
        fs::create_dir_all(parent).map_err(|e| ServerError::CommandFailed(e.to_string()))?;
    }
    fs::write(&runbook_path, &markdown).map_err(|e| ServerError::CommandFailed(e.to_string()))?;

    // Update source RFC with runbook link if provided
    if let Some(ref rfc_doc) = source_rfc_doc {
        if let Some(ref rfc_file_path) = rfc_doc.file_path {
            let rfc_path = docs_path.join(rfc_file_path);
            if rfc_path.exists() {
                if let Ok(rfc_content) = fs::read_to_string(&rfc_path) {
                    let runbook_link = format!(
                        "| **Runbook** | [{}](../runbooks/{}) |",
                        title, file_name
                    );

                    // Insert after Status line if not already present
                    if !rfc_content.contains("| **Runbook** |") {
                        let updated_rfc = if rfc_content.contains("| **Status** | Implemented |") {
                            rfc_content.replace(
                                "| **Status** | Implemented |",
                                &format!("| **Status** | Implemented |\n{}", runbook_link),
                            )
                        } else {
                            rfc_content
                        };
                        let _ = fs::write(&rfc_path, updated_rfc);
                    }
                }
            }
        }
    }

    let hint = "Runbook created. Fill in the operation procedures and troubleshooting sections.";

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Runbook created: {}", title),
            Some(hint)
        ),
        "title": title,
        "file": runbook_path.display().to_string(),
        "source_rfc": source_rfc,
        "content": markdown,
    }))
}

/// Handle blue_runbook_update
pub fn handle_update(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    let title = args
        .get("title")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let add_operation = args.get("add_operation").and_then(|v| v.as_str());
    let add_troubleshooting = args.get("add_troubleshooting").and_then(|v| v.as_str());

    // Find the runbook
    let doc = state
        .store
        .find_document(DocType::Runbook, title)
        .map_err(|_| ServerError::NotFound(format!("Runbook '{}' not found", title)))?;

    let runbook_file_path = doc.file_path.as_ref().ok_or_else(|| {
        ServerError::CommandFailed("Runbook has no file path".to_string())
    })?;

    let docs_path = state.home.docs_path.clone();
    let runbook_path = docs_path.join(runbook_file_path);
    let mut content = fs::read_to_string(&runbook_path)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to read runbook: {}", e)))?;

    let mut changes = Vec::new();

    // Add new operation if provided
    if let Some(operation) = add_operation {
        let operation_section = format!(
            "\n### Operation: {}\n\n**When to use**: [Describe trigger condition]\n\n**Steps**:\n1. [Step 1]\n\n**Verification**:\n```bash\n# Verify success\n```\n\n**Rollback**:\n```bash\n# Rollback if needed\n```\n",
            operation
        );

        // Insert before Troubleshooting section or at end
        if content.contains("## Troubleshooting") {
            content = content.replace(
                "## Troubleshooting",
                &format!("{}\n## Troubleshooting", operation_section),
            );
        } else {
            content.push_str(&operation_section);
        }
        changes.push(format!("Added operation: {}", operation));
    }

    // Add troubleshooting if provided
    if let Some(troubleshooting) = add_troubleshooting {
        let troubleshooting_section = format!(
            "\n### Symptom: {}\n\n**Possible causes**:\n1. [Cause 1]\n\n**Resolution**:\n1. [Step 1]\n",
            troubleshooting
        );

        // Insert into Troubleshooting section or create one
        if content.contains("## Troubleshooting") {
            if content.contains("## Escalation") {
                content = content.replace(
                    "## Escalation",
                    &format!("{}\n## Escalation", troubleshooting_section),
                );
            } else {
                content.push_str(&troubleshooting_section);
            }
        } else {
            content.push_str(&format!(
                "\n## Troubleshooting\n{}",
                troubleshooting_section
            ));
        }
        changes.push(format!("Added troubleshooting: {}", troubleshooting));
    }

    // Write updated content
    fs::write(&runbook_path, &content)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to write runbook: {}", e)))?;

    let hint = "Runbook updated. Review the changes and fill in details.";

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("Runbook updated: {}", title),
            Some(hint)
        ),
        "title": title,
        "file": runbook_path.display().to_string(),
        "changes": changes,
    }))
}

/// Generate runbook markdown content
fn generate_runbook_markdown(
    title: &str,
    source_rfc: &Option<Document>,
    service_name: Option<&str>,
    owner: Option<&str>,
    operations: &[String],
    actions: &[String],
) -> String {
    let mut md = String::new();

    // Title
    md.push_str(&format!(
        "# Runbook: {}\n\n",
        to_title_case(title)
    ));

    // Metadata table
    md.push_str("| | |\n|---|---|\n");
    md.push_str("| **Status** | Active |\n");

    // Actions field (RFC 0002)
    if !actions.is_empty() {
        md.push_str(&format!("| **Actions** | {} |\n", actions.join(", ")));
    }

    if let Some(o) = owner {
        md.push_str(&format!("| **Owner** | {} |\n", o));
    }

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    md.push_str(&format!("| **Created** | {} |\n", date));

    if let Some(ref rfc_doc) = source_rfc {
        if let Some(ref rfc_file_path) = rfc_doc.file_path {
            md.push_str(&format!(
                "| **Source RFC** | [{}](../rfcs/{}) |\n",
                rfc_doc.title, rfc_file_path.replace("rfcs/", "")
            ));
        }
    }

    md.push_str("\n---\n\n");

    // Overview
    md.push_str("## Overview\n\n");
    if let Some(svc) = service_name {
        md.push_str(&format!(
            "This runbook covers operational procedures for **{}**.\n\n",
            svc
        ));
    } else {
        md.push_str("[Describe what this runbook covers]\n\n");
    }

    // Prerequisites
    md.push_str("## Prerequisites\n\n");
    md.push_str("- [ ] Access to [system]\n");
    md.push_str("- [ ] Permissions for [action]\n\n");

    // Common Operations
    md.push_str("## Common Operations\n\n");

    if !operations.is_empty() {
        for op in operations {
            md.push_str(&format!(
                "### Operation: {}\n\n**When to use**: [Trigger condition]\n\n**Steps**:\n1. [Step 1]\n\n**Verification**:\n```bash\n# Command to verify success\n```\n\n**Rollback**:\n```bash\n# Command to rollback if needed\n```\n\n",
                op
            ));
        }
    } else {
        md.push_str("### Operation 1: [Name]\n\n**When to use**: [Trigger condition]\n\n**Steps**:\n1. [Step 1]\n\n**Verification**:\n```bash\n# Command to verify success\n```\n\n**Rollback**:\n```bash\n# Command to rollback if needed\n```\n\n");
    }

    // Troubleshooting
    md.push_str("## Troubleshooting\n\n");
    md.push_str("### Symptom: [Description]\n\n**Possible causes**:\n1. [Cause 1]\n\n**Resolution**:\n1. [Step 1]\n\n");

    // Escalation
    md.push_str("## Escalation\n\n");
    md.push_str("| Level | Contact | When |\n");
    md.push_str("|-------|---------|------|\n");
    md.push_str("| L1 | [Team] | [Condition] |\n");
    md.push_str("| L2 | [Team] | [Condition] |\n\n");

    // Related Documents
    md.push_str("## Related Documents\n\n");
    if source_rfc.is_some() {
        md.push_str("- Source RFC (linked above)\n");
    }
    md.push_str("- [Link to architecture]\n");
    md.push_str("- [Link to monitoring dashboard]\n");

    md
}

/// Convert a title to kebab-case for filenames
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

// ===== RFC 0002: Runbook Action Lookup =====

/// Handle blue_runbook_lookup
///
/// Find runbook by action query using word-based matching.
pub fn handle_lookup(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let action_query = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?
        .to_lowercase();

    // Get all runbooks with actions from metadata
    let runbooks = state
        .store
        .list_documents(DocType::Runbook)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    // Find best match
    let mut best_match: Option<(Document, Vec<String>, i32)> = None;

    for runbook in runbooks {
        if let Some(doc_id) = runbook.id {
            // Get actions for this runbook
            let actions = get_runbook_actions(&state.store, doc_id);

            if actions.is_empty() {
                continue;
            }

            // Calculate best match score for this runbook
            for action in &actions {
                let score = calculate_match_score(&action_query, action);
                if score > 0
                    && best_match.as_ref().is_none_or(|(_, _, s)| score > *s) {
                        best_match = Some((runbook.clone(), actions.clone(), score));
                        break; // This runbook matches, move to next
                    }
            }
        }
    }

    match best_match {
        Some((runbook, actions, _score)) => {
            // Parse operations from the runbook file
            let operations = if let Some(ref file_path) = runbook.file_path {
                let full_path = state.home.docs_path.join(file_path);
                if full_path.exists() {
                    if let Ok(content) = fs::read_to_string(&full_path) {
                        parse_operations(&content)
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            Ok(json!({
                "found": true,
                "runbook": {
                    "title": runbook.title,
                    "file": runbook.file_path,
                    "actions": actions,
                    "operations": operations
                },
                "hint": "Follow the steps above. Use verification to confirm success."
            }))
        }
        None => {
            Ok(json!({
                "found": false,
                "hint": "No runbook found. Proceed with caution."
            }))
        }
    }
}

/// Handle blue_runbook_actions
///
/// List all registered actions across runbooks.
pub fn handle_actions(state: &ProjectState) -> Result<Value, ServerError> {
    let runbooks = state
        .store
        .list_documents(DocType::Runbook)
        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

    let mut all_actions: Vec<Value> = Vec::new();

    for runbook in runbooks {
        if let Some(doc_id) = runbook.id {
            let actions = get_runbook_actions(&state.store, doc_id);
            for action in actions {
                all_actions.push(json!({
                    "action": action,
                    "runbook": runbook.title
                }));
            }
        }
    }

    Ok(json!({
        "actions": all_actions,
        "count": all_actions.len()
    }))
}

/// Get actions for a runbook from metadata table
fn get_runbook_actions(store: &blue_core::DocumentStore, doc_id: i64) -> Vec<String> {
    let mut actions = Vec::new();

    if let Ok(mut stmt) = store.conn().prepare(
        "SELECT value FROM metadata WHERE document_id = ?1 AND key = ?2"
    ) {
        if let Ok(rows) = stmt.query_map(rusqlite::params![doc_id, ACTION_KEY], |row| {
            row.get::<_, String>(0)
        }) {
            for action in rows.flatten() {
                actions.push(action);
            }
        }
    }

    actions
}

/// Calculate match score between query and action
///
/// Scoring:
/// - Exact match: 100
/// - All query words in action: 90
/// - Partial word match: 80 * (matched_words / query_words)
fn calculate_match_score(query: &str, action: &str) -> i32 {
    let query = query.trim().to_lowercase();
    let action = action.trim().to_lowercase();

    // Exact match
    if query == action {
        return 100;
    }

    let query_words: Vec<&str> = query.split_whitespace().collect();
    let action_words: Vec<&str> = action.split_whitespace().collect();

    if query_words.is_empty() {
        return 0;
    }

    // Count how many query words are in action
    let matched = query_words.iter().filter(|qw| action_words.contains(qw)).count();

    // All query words match (subset)
    if matched == query_words.len() {
        return 90;
    }

    // Partial match
    if matched > 0 {
        return (80 * matched as i32) / query_words.len() as i32;
    }

    // Check for substring match in any word
    let has_substring = query_words.iter().any(|qw| {
        action_words.iter().any(|aw| aw.contains(qw) || qw.contains(aw))
    });

    if has_substring {
        return 50;
    }

    0
}

/// Parse operations from runbook markdown content
fn parse_operations(content: &str) -> Vec<Value> {
    let mut operations = Vec::new();
    let mut current_op: Option<ParsedOperation> = None;
    let mut current_section = Section::None;

    for line in content.lines() {
        // Detect operation header
        if line.starts_with("### Operation:") {
            // Save previous operation
            if let Some(op) = current_op.take() {
                operations.push(op.to_json());
            }
            let name = line.trim_start_matches("### Operation:").trim().to_string();
            current_op = Some(ParsedOperation::new(name));
            current_section = Section::None;
            continue;
        }

        // Skip if we're not in an operation
        let Some(ref mut op) = current_op else {
            continue;
        };

        // Detect section headers within operation
        if line.starts_with("**When to use**:") {
            op.when_to_use = line.trim_start_matches("**When to use**:").trim().to_string();
            continue;
        }

        if line.starts_with("**Steps**:") {
            current_section = Section::Steps;
            continue;
        }

        if line.starts_with("**Verification**:") {
            current_section = Section::Verification;
            continue;
        }

        if line.starts_with("**Rollback**:") {
            current_section = Section::Rollback;
            continue;
        }

        // New top-level section ends operation parsing
        if line.starts_with("## ") {
            if let Some(op) = current_op.take() {
                operations.push(op.to_json());
            }
            break;
        }

        // Collect content based on current section
        match current_section {
            Section::Steps => {
                if line.starts_with("1.") || line.starts_with("2.") || line.starts_with("3.")
                   || line.starts_with("4.") || line.starts_with("5.") {
                    let step = line.trim_start_matches(|c: char| c.is_numeric() || c == '.').trim();
                    if !step.is_empty() {
                        op.steps.push(step.to_string());
                    }
                }
            }
            Section::Verification => {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("```") {
                    op.verification.push(trimmed.to_string());
                }
            }
            Section::Rollback => {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("```") {
                    op.rollback.push(trimmed.to_string());
                }
            }
            Section::None => {}
        }
    }

    // Don't forget the last operation
    if let Some(op) = current_op {
        operations.push(op.to_json());
    }

    operations
}

#[derive(Debug)]
enum Section {
    None,
    Steps,
    Verification,
    Rollback,
}

#[derive(Debug)]
struct ParsedOperation {
    name: String,
    when_to_use: String,
    steps: Vec<String>,
    verification: Vec<String>,
    rollback: Vec<String>,
}

impl ParsedOperation {
    fn new(name: String) -> Self {
        Self {
            name,
            when_to_use: String::new(),
            steps: Vec::new(),
            verification: Vec::new(),
            rollback: Vec::new(),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "when_to_use": self.when_to_use,
            "steps": self.steps,
            "verification": self.verification.join("\n"),
            "rollback": self.rollback.join("\n")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("Deploy Service"), "deploy-service");
        assert_eq!(to_kebab_case("API Gateway Runbook"), "api-gateway-runbook");
    }

    #[test]
    fn test_match_score_exact() {
        assert_eq!(calculate_match_score("docker build", "docker build"), 100);
        assert_eq!(calculate_match_score("DOCKER BUILD", "docker build"), 100);
    }

    #[test]
    fn test_match_score_all_words() {
        assert_eq!(calculate_match_score("docker", "docker build"), 90);
        assert_eq!(calculate_match_score("build", "docker build"), 90);
    }

    #[test]
    fn test_match_score_partial() {
        // "docker" matches one of two words in "build image" = 0
        // But "build" matches "build image" = 90
        assert_eq!(calculate_match_score("build", "build image"), 90);
        // Neither "test" nor "suite" is in "docker build"
        assert_eq!(calculate_match_score("test suite", "docker build"), 0);
    }

    #[test]
    fn test_match_score_no_match() {
        assert_eq!(calculate_match_score("deploy", "docker build"), 0);
        assert_eq!(calculate_match_score("", "docker build"), 0);
    }

    #[test]
    fn test_parse_operations() {
        let content = r#"# Runbook: Docker Build

## Common Operations

### Operation: Build Production Image

**When to use**: Preparing for deployment

**Steps**:
1. Ensure on correct branch
2. Pull latest
3. Build image

**Verification**:
```bash
docker images | grep myapp
```

**Rollback**:
```bash
docker rmi myapp:latest
```

## Troubleshooting
"#;

        let ops = parse_operations(content);
        assert_eq!(ops.len(), 1);

        let op = &ops[0];
        assert_eq!(op["name"], "Build Production Image");
        assert_eq!(op["when_to_use"], "Preparing for deployment");

        let steps = op["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "Ensure on correct branch");
    }

    #[test]
    fn test_parse_operations_multiple() {
        let content = r#"## Common Operations

### Operation: Start Service

**When to use**: After deployment

**Steps**:
1. Run start command

**Verification**:
```bash
curl localhost:8080/health
```

**Rollback**:
```bash
./stop.sh
```

### Operation: Stop Service

**When to use**: Before maintenance

**Steps**:
1. Run stop command

**Verification**:
```bash
pgrep myapp || echo "Stopped"
```

**Rollback**:
```bash
./start.sh
```

## Troubleshooting
"#;

        let ops = parse_operations(content);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0]["name"], "Start Service");
        assert_eq!(ops[1]["name"], "Stop Service");
    }
}
