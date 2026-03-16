//! Jira sync engine — write-through projection from RFC docs to Jira (RFC 0063, Phase 2)
//!
//! Scans local `.blue/docs/rfcs/` for RFC markdown files, parses Jira binding
//! fields from the front-matter table, and creates/transitions Jira issues
//! to keep the tracker in sync with git as the sole authority.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::{CreateIssueOpts, IssueTracker, IssueType, TrackerError, TransitionOpts};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Jira binding stored in RFC front-matter table rows
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JiraBinding {
    pub blue_uuid: Option<String>,
    pub task_key: Option<String>,
    pub epic_id: Option<String>,
}

/// Result of syncing a single RFC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub rfc_title: String,
    pub rfc_file: String,
    pub action: SyncAction,
    pub jira_key: Option<String>,
    pub error: Option<String>,
}

/// What happened during sync for one RFC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncAction {
    /// New Jira issue created
    Created,
    /// Status transitioned in Jira
    Transitioned,
    /// Already in sync
    UpToDate,
    /// Skipped (e.g. no tracker configured)
    Skipped,
    /// Failed
    Error,
}

/// Drift between local RFC and remote Jira state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    pub rfc_title: String,
    pub jira_key: String,
    pub field: String,
    pub local_value: String,
    pub jira_value: String,
}

/// Configuration for sync behaviour
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub domain: String,
    pub project_key: String,
    pub drift_policy: DriftPolicy,
}

/// What to do when Jira state drifts from local
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriftPolicy {
    /// Overwrite Jira with local state
    Overwrite,
    /// Warn but take no action
    Warn,
    /// Block sync entirely when drift detected
    Block,
}

impl Default for DriftPolicy {
    fn default() -> Self {
        DriftPolicy::Warn
    }
}

/// Full sync report covering all RFCs
#[derive(Debug, Default)]
pub struct SyncReport {
    pub results: Vec<SyncResult>,
    pub drift: Vec<DriftReport>,
    pub created: usize,
    pub transitioned: usize,
    pub up_to_date: usize,
    pub errors: usize,
}

// ---------------------------------------------------------------------------
// Front-matter parsing
// ---------------------------------------------------------------------------

/// Parse a Jira binding from RFC markdown front-matter table.
///
/// Looks for rows like:
/// ```text
/// | **Jira** | PROJ-123 |
/// | **Blue UUID** | a1b2c3d4-... |
/// | **Epic** | user-auth-overhaul |
/// ```
pub fn parse_jira_binding(content: &str) -> JiraBinding {
    let mut binding = JiraBinding::default();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Some(value) = extract_table_value(trimmed, "Jira") {
            if !value.is_empty() {
                binding.task_key = Some(value);
            }
        } else if let Some(value) = extract_table_value(trimmed, "Blue UUID") {
            if !value.is_empty() {
                binding.blue_uuid = Some(value);
            }
        } else if let Some(value) = extract_table_value(trimmed, "Epic") {
            if !value.is_empty() {
                binding.epic_id = Some(value);
            }
        }
    }

    binding
}

/// Extract a value from a markdown table row like `| **Key** | value |`.
///
/// Returns `None` if the line does not match the given key.
fn extract_table_value(line: &str, key: &str) -> Option<String> {
    // Match: | **Key** | value |
    // Also handle optional trailing pipe and whitespace.
    let bold_key = format!("**{}**", key);

    if !line.starts_with('|') {
        return None;
    }

    let parts: Vec<&str> = line.split('|').collect();
    // parts[0] is empty (before first |), parts[1] is key cell, parts[2] is value cell
    if parts.len() < 3 {
        return None;
    }

    let key_cell = parts[1].trim();
    if key_cell != bold_key {
        return None;
    }

    let value_cell = parts[2].trim();
    Some(value_cell.to_string())
}

/// Write or update Jira binding fields in RFC markdown front-matter.
///
/// If a row for a field already exists, its value is replaced.
/// If it does not exist, a new row is inserted after the last metadata row
/// in the table (before the `---` separator or end of table).
pub fn update_jira_binding(content: &str, binding: &JiraBinding) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Track which fields we still need to insert
    let mut needs_uuid = binding.blue_uuid.is_some();
    let mut needs_task = binding.task_key.is_some();
    let mut needs_epic = binding.epic_id.is_some();

    // First pass: update existing rows in-place
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if extract_table_value(trimmed, "Blue UUID").is_some() {
            if let Some(ref uuid) = binding.blue_uuid {
                *line = format!("| **Blue UUID** | {} |", uuid);
            }
            needs_uuid = false;
        } else if extract_table_value(trimmed, "Jira").is_some() {
            if let Some(ref key) = binding.task_key {
                *line = format!("| **Jira** | {} |", key);
            }
            needs_task = false;
        } else if extract_table_value(trimmed, "Epic").is_some() {
            if let Some(ref epic) = binding.epic_id {
                *line = format!("| **Epic** | {} |", epic);
            }
            needs_epic = false;
        }
    }

    // Second pass: insert missing fields after the last table-row before `---`
    if needs_uuid || needs_task || needs_epic {
        let mut insert_rows: Vec<String> = Vec::new();
        if needs_uuid {
            if let Some(ref uuid) = binding.blue_uuid {
                insert_rows.push(format!("| **Blue UUID** | {} |", uuid));
            }
        }
        if needs_task {
            if let Some(ref key) = binding.task_key {
                insert_rows.push(format!("| **Jira** | {} |", key));
            }
        }
        if needs_epic {
            if let Some(ref epic) = binding.epic_id {
                insert_rows.push(format!("| **Epic** | {} |", epic));
            }
        }

        if !insert_rows.is_empty() {
            // Find the insertion point: last metadata table row (starts with `|` and
            // contains `**`) before a `---` line or a non-table line.
            let mut insert_idx = None;
            let mut in_table = false;
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("| |") || trimmed.starts_with("|---|") {
                    in_table = true;
                    continue;
                }
                if in_table && trimmed.starts_with('|') && trimmed.contains("**") {
                    insert_idx = Some(i + 1);
                }
                if in_table && !trimmed.starts_with('|') {
                    // We've left the table
                    break;
                }
            }

            if let Some(idx) = insert_idx {
                for (offset, row) in insert_rows.into_iter().enumerate() {
                    lines.insert(idx + offset, row);
                }
            }
        }
    }

    // Preserve trailing newline if original had one
    let mut result = lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Parse RFC status from front-matter table.
///
/// Returns the lowercased, kebab-case status (e.g. "draft", "in-progress").
pub fn parse_rfc_status(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(value) = extract_table_value(line.trim(), "Status") {
            // Normalise: "In Progress" -> "in-progress", "Draft" -> "draft"
            let normalised = value
                .trim()
                .to_lowercase()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join("-");
            return Some(normalised);
        }
    }
    None
}

/// Parse RFC title from first heading.
pub fn parse_rfc_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed.trim_start_matches("# ").to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Status mapping
// ---------------------------------------------------------------------------

/// Map RFC status to Jira target status name.
pub fn rfc_status_to_jira(status: &str) -> Option<&'static str> {
    match status {
        "draft" => Some("To Do"),
        "accepted" => Some("To Do"),
        "in-progress" => Some("In Progress"),
        "implemented" => Some("Done"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Sync engine
// ---------------------------------------------------------------------------

/// Run sync for all RFCs in `docs_path` against a Jira tracker.
///
/// `docs_path` should point to `.blue/docs/rfcs/` (or similar).
/// When `dry_run` is true, no Jira mutations or file writes occur.
pub fn run_sync(
    tracker: &dyn IssueTracker,
    config: &SyncConfig,
    docs_path: &Path,
    dry_run: bool,
) -> Result<SyncReport, TrackerError> {
    let mut report = SyncReport::default();

    let rfc_files = discover_rfc_files(docs_path)?;

    for rfc_path in rfc_files {
        let result = sync_single_rfc(tracker, config, &rfc_path, dry_run);
        match &result.action {
            SyncAction::Created => report.created += 1,
            SyncAction::Transitioned => report.transitioned += 1,
            SyncAction::UpToDate => report.up_to_date += 1,
            SyncAction::Error => report.errors += 1,
            SyncAction::Skipped => {}
        }
        report.results.push(result);
    }

    Ok(report)
}

/// Discover RFC markdown files in a directory.
fn discover_rfc_files(docs_path: &Path) -> Result<Vec<PathBuf>, TrackerError> {
    if !docs_path.exists() {
        return Ok(Vec::new());
    }

    let mut files: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(docs_path).map_err(|e| {
        TrackerError::Http(format!("Failed to read docs directory: {}", e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            TrackerError::Http(format!("Failed to read directory entry: {}", e))
        })?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "md") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Sync a single RFC file against Jira.
fn sync_single_rfc(
    tracker: &dyn IssueTracker,
    config: &SyncConfig,
    rfc_path: &Path,
    dry_run: bool,
) -> SyncResult {
    let file_name = rfc_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let content = match std::fs::read_to_string(rfc_path) {
        Ok(c) => c,
        Err(e) => {
            return SyncResult {
                rfc_title: file_name.clone(),
                rfc_file: file_name,
                action: SyncAction::Error,
                jira_key: None,
                error: Some(format!("Failed to read file: {}", e)),
            };
        }
    };

    let title = parse_rfc_title(&content).unwrap_or_else(|| file_name.clone());
    let status = parse_rfc_status(&content);
    let mut binding = parse_jira_binding(&content);
    let mut action = SyncAction::UpToDate;

    // Step 1: Ensure blue_uuid
    if binding.blue_uuid.is_none() {
        let uuid = Uuid::new_v4().to_string();
        binding.blue_uuid = Some(uuid);
        action = SyncAction::Created; // will be overridden if we also create issue
    }

    // Step 2: Create Jira issue if no task_key
    if binding.task_key.is_none() {
        if dry_run {
            action = SyncAction::Created;
        } else {
            let opts = CreateIssueOpts {
                project: config.project_key.clone(),
                issue_type: IssueType::Task,
                summary: title.clone(),
                description: Some(format!("Synced from RFC: {}", file_name)),
                epic_key: binding.epic_id.clone(),
                labels: vec!["blue-sync".to_string()],
                components: vec![],
            };

            match tracker.create_issue(opts) {
                Ok(issue) => {
                    binding.task_key = Some(issue.key.clone());
                    action = SyncAction::Created;
                }
                Err(e) => {
                    return SyncResult {
                        rfc_title: title,
                        rfc_file: file_name,
                        action: SyncAction::Error,
                        jira_key: None,
                        error: Some(format!("Failed to create Jira issue: {}", e)),
                    };
                }
            }
        }
    } else if let (Some(ref task_key), Some(ref rfc_status)) = (&binding.task_key, &status) {
        // Step 3: Transition if status changed
        if let Some(target_jira_status) = rfc_status_to_jira(rfc_status) {
            if !dry_run {
                // Check current Jira status
                match tracker.get_issue(task_key) {
                    Ok(issue) => {
                        if issue.status.name != target_jira_status {
                            match tracker.transition_issue(TransitionOpts {
                                key: task_key.clone(),
                                target_status: target_jira_status.to_string(),
                            }) {
                                Ok(()) => {
                                    action = SyncAction::Transitioned;
                                }
                                Err(e) => {
                                    return SyncResult {
                                        rfc_title: title,
                                        rfc_file: file_name,
                                        action: SyncAction::Error,
                                        jira_key: Some(task_key.clone()),
                                        error: Some(format!("Transition failed: {}", e)),
                                    };
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return SyncResult {
                            rfc_title: title,
                            rfc_file: file_name,
                            action: SyncAction::Error,
                            jira_key: Some(task_key.clone()),
                            error: Some(format!("Failed to get Jira issue: {}", e)),
                        };
                    }
                }
            } else {
                // In dry-run mode, report as potentially needing transition
                action = SyncAction::Skipped;
            }
        }
    }

    // Write binding back to file if changed
    let original_binding = parse_jira_binding(&content);
    if binding != original_binding {
        let content_modified = update_jira_binding(&content, &binding);
        if !dry_run {
            if let Err(e) = std::fs::write(rfc_path, &content_modified) {
                return SyncResult {
                    rfc_title: title,
                    rfc_file: file_name,
                    action: SyncAction::Error,
                    jira_key: binding.task_key,
                    error: Some(format!("Failed to write binding to file: {}", e)),
                };
            }
        }
    }

    SyncResult {
        rfc_title: title,
        rfc_file: file_name,
        action,
        jira_key: binding.task_key,
        error: None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RFC: &str = r#"# RFC 0063: Jira Cloud Integration

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |
| **Jira** | PROJ-142 |
| **Blue UUID** | a1b2c3d4-e5f6-7890-abcd-ef1234567890 |
| **Epic** | user-auth-overhaul |

---

## Summary

Integrate with Jira Cloud for project management.
"#;

    const SAMPLE_RFC_NO_BINDING: &str = r#"# RFC 0099: New Feature

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-03-12 |

---

## Summary

A new feature.
"#;

    const SAMPLE_RFC_PARTIAL: &str = r#"# RFC 0070: Partial Binding

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-03-10 |
| **Jira** | BLUE-42 |

---

## Summary

Has Jira key but no UUID.
"#;

    #[test]
    fn test_parse_jira_binding_full() {
        let binding = parse_jira_binding(SAMPLE_RFC);
        assert_eq!(binding.task_key.as_deref(), Some("PROJ-142"));
        assert_eq!(
            binding.blue_uuid.as_deref(),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
        );
        assert_eq!(binding.epic_id.as_deref(), Some("user-auth-overhaul"));
    }

    #[test]
    fn test_parse_jira_binding_none() {
        let binding = parse_jira_binding(SAMPLE_RFC_NO_BINDING);
        assert!(binding.task_key.is_none());
        assert!(binding.blue_uuid.is_none());
        assert!(binding.epic_id.is_none());
    }

    #[test]
    fn test_parse_jira_binding_partial() {
        let binding = parse_jira_binding(SAMPLE_RFC_PARTIAL);
        assert_eq!(binding.task_key.as_deref(), Some("BLUE-42"));
        assert!(binding.blue_uuid.is_none());
        assert!(binding.epic_id.is_none());
    }

    #[test]
    fn test_parse_jira_binding_empty_content() {
        let binding = parse_jira_binding("");
        assert_eq!(binding, JiraBinding::default());
    }

    #[test]
    fn test_parse_rfc_status() {
        assert_eq!(parse_rfc_status(SAMPLE_RFC).as_deref(), Some("draft"));
        assert_eq!(
            parse_rfc_status(SAMPLE_RFC_NO_BINDING).as_deref(),
            Some("in-progress")
        );
        assert_eq!(
            parse_rfc_status(SAMPLE_RFC_PARTIAL).as_deref(),
            Some("accepted")
        );
    }

    #[test]
    fn test_parse_rfc_status_missing() {
        assert_eq!(parse_rfc_status("No table here"), None);
    }

    #[test]
    fn test_parse_rfc_title() {
        assert_eq!(
            parse_rfc_title(SAMPLE_RFC).as_deref(),
            Some("RFC 0063: Jira Cloud Integration")
        );
        assert_eq!(
            parse_rfc_title(SAMPLE_RFC_NO_BINDING).as_deref(),
            Some("RFC 0099: New Feature")
        );
    }

    #[test]
    fn test_parse_rfc_title_missing() {
        assert_eq!(parse_rfc_title("No heading here"), None);
    }

    #[test]
    fn test_update_jira_binding_replace_existing() {
        let binding = JiraBinding {
            blue_uuid: Some("new-uuid-1234".to_string()),
            task_key: Some("PROJ-999".to_string()),
            epic_id: Some("new-epic".to_string()),
        };

        let result = update_jira_binding(SAMPLE_RFC, &binding);

        assert!(result.contains("| **Jira** | PROJ-999 |"));
        assert!(result.contains("| **Blue UUID** | new-uuid-1234 |"));
        assert!(result.contains("| **Epic** | new-epic |"));
        // Original fields should still be present
        assert!(result.contains("| **Status** | Draft |"));
        assert!(result.contains("| **Date** | 2026-03-11 |"));
    }

    #[test]
    fn test_update_jira_binding_insert_new_fields() {
        let binding = JiraBinding {
            blue_uuid: Some("inserted-uuid".to_string()),
            task_key: Some("BLUE-77".to_string()),
            epic_id: None,
        };

        let result = update_jira_binding(SAMPLE_RFC_NO_BINDING, &binding);

        assert!(result.contains("| **Blue UUID** | inserted-uuid |"));
        assert!(result.contains("| **Jira** | BLUE-77 |"));
        // Original content preserved
        assert!(result.contains("| **Status** | In Progress |"));
        assert!(result.contains("## Summary"));
    }

    #[test]
    fn test_update_jira_binding_partial_update() {
        // SAMPLE_RFC_PARTIAL has Jira key but no UUID/Epic
        let binding = JiraBinding {
            blue_uuid: Some("added-uuid".to_string()),
            task_key: Some("BLUE-42".to_string()), // same as existing
            epic_id: Some("my-epic".to_string()),
        };

        let result = update_jira_binding(SAMPLE_RFC_PARTIAL, &binding);

        assert!(result.contains("| **Jira** | BLUE-42 |"));
        assert!(result.contains("| **Blue UUID** | added-uuid |"));
        assert!(result.contains("| **Epic** | my-epic |"));
    }

    #[test]
    fn test_update_jira_binding_no_changes() {
        let binding = JiraBinding::default();
        let result = update_jira_binding(SAMPLE_RFC, &binding);
        // With default binding (all None), existing values should remain untouched
        assert!(result.contains("| **Jira** | PROJ-142 |"));
        assert!(result.contains("| **Blue UUID** | a1b2c3d4-e5f6-7890-abcd-ef1234567890 |"));
    }

    #[test]
    fn test_rfc_status_to_jira_mapping() {
        assert_eq!(rfc_status_to_jira("draft"), Some("To Do"));
        assert_eq!(rfc_status_to_jira("accepted"), Some("To Do"));
        assert_eq!(rfc_status_to_jira("in-progress"), Some("In Progress"));
        assert_eq!(rfc_status_to_jira("implemented"), Some("Done"));
        assert_eq!(rfc_status_to_jira("superseded"), None);
        assert_eq!(rfc_status_to_jira("unknown"), None);
    }

    #[test]
    fn test_extract_table_value() {
        assert_eq!(
            extract_table_value("| **Jira** | PROJ-123 |", "Jira"),
            Some("PROJ-123".to_string())
        );
        assert_eq!(
            extract_table_value("| **Blue UUID** | abc-123 |", "Blue UUID"),
            Some("abc-123".to_string())
        );
        assert_eq!(
            extract_table_value("| **Status** | Draft |", "Jira"),
            None
        );
        assert_eq!(extract_table_value("not a table row", "Jira"), None);
        assert_eq!(extract_table_value("| only one cell", "Jira"), None);
    }

    #[test]
    fn test_extract_table_value_empty() {
        assert_eq!(
            extract_table_value("| **Jira** |  |", "Jira"),
            Some(String::new())
        );
    }

    #[test]
    fn test_discover_rfc_files_nonexistent_dir() {
        let files = discover_rfc_files(Path::new("/nonexistent/path")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_rfc_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0001-test.md"), "# RFC").unwrap();
        std::fs::write(dir.path().join("0002-other.md"), "# RFC 2").unwrap();
        std::fs::write(dir.path().join("not-md.txt"), "skip").unwrap();

        let files = discover_rfc_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].file_name().unwrap().to_string_lossy().contains("0001"));
        assert!(files[1].file_name().unwrap().to_string_lossy().contains("0002"));
    }

    #[test]
    fn test_sync_report_default() {
        let report = SyncReport::default();
        assert_eq!(report.created, 0);
        assert_eq!(report.transitioned, 0);
        assert_eq!(report.up_to_date, 0);
        assert_eq!(report.errors, 0);
        assert!(report.results.is_empty());
        assert!(report.drift.is_empty());
    }

    #[test]
    fn test_drift_policy_default() {
        assert_eq!(DriftPolicy::default(), DriftPolicy::Warn);
    }

    #[test]
    fn test_sync_config_serde() {
        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "BLUE".to_string(),
            drift_policy: DriftPolicy::Overwrite,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SyncConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.domain, "test.atlassian.net");
        assert_eq!(parsed.drift_policy, DriftPolicy::Overwrite);
    }

    #[test]
    fn test_sync_result_serde() {
        let result = SyncResult {
            rfc_title: "Test".to_string(),
            rfc_file: "test.md".to_string(),
            action: SyncAction::Created,
            jira_key: Some("PROJ-1".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: SyncResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.action, SyncAction::Created);
        assert_eq!(parsed.jira_key.as_deref(), Some("PROJ-1"));
    }

    #[test]
    fn test_run_sync_empty_dir() {
        // Mock tracker not needed since no files to sync
        let dir = tempfile::tempdir().unwrap();
        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        // We need a tracker but since there are no files, it won't be called.
        // Use a minimal mock.
        let tracker = MockTracker;
        let report = run_sync(&tracker, &config, dir.path(), true).unwrap();
        assert!(report.results.is_empty());
        assert_eq!(report.created, 0);
    }

    #[test]
    fn test_run_sync_dry_run_creates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("0001-test.md"),
            SAMPLE_RFC_NO_BINDING,
        )
        .unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = MockTracker;
        let report = run_sync(&tracker, &config, dir.path(), true).unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].action, SyncAction::Created);
        assert_eq!(report.created, 1);

        // In dry-run mode, the file should NOT be modified
        let content = std::fs::read_to_string(dir.path().join("0001-test.md")).unwrap();
        assert!(!content.contains("Blue UUID"));
    }

    #[test]
    fn test_run_sync_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0063-jira.md"), SAMPLE_RFC).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "PROJ".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = MockTracker;
        // Already has task_key and uuid, status=draft maps to "To Do", mock returns "To Do"
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].action, SyncAction::UpToDate);
    }

    #[test]
    fn test_update_preserves_trailing_newline() {
        let content = "# RFC\n\n| | |\n|---|---|\n| **Status** | Draft |\n";
        let binding = JiraBinding {
            blue_uuid: Some("test-uuid".to_string()),
            ..Default::default()
        };
        let result = update_jira_binding(content, &binding);
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_parse_status_title_case() {
        let content = "| **Status** | In Progress |";
        assert_eq!(parse_rfc_status(content).as_deref(), Some("in-progress"));
    }

    #[test]
    fn test_parse_status_single_word() {
        let content = "| **Status** | Implemented |";
        assert_eq!(parse_rfc_status(content).as_deref(), Some("implemented"));
    }

    // --- Mock trackers for unit tests ---

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Simple mock: always returns "To Do" status, creates succeed
    struct MockTracker;

    impl IssueTracker for MockTracker {
        fn auth_status(&self) -> Result<super::super::AuthStatus, TrackerError> {
            Ok(super::super::AuthStatus {
                authenticated: true,
                user: Some("test".to_string()),
                email: Some("test@test.com".to_string()),
                domain: "test.atlassian.net".to_string(),
            })
        }

        fn list_projects(&self) -> Result<Vec<super::super::TrackerProject>, TrackerError> {
            Ok(vec![])
        }

        fn create_issue(
            &self,
            opts: CreateIssueOpts,
        ) -> Result<super::super::Issue, TrackerError> {
            Ok(super::super::Issue {
                key: format!("{}-1", opts.project),
                summary: opts.summary,
                issue_type: opts.issue_type,
                status: super::super::IssueStatus {
                    name: "To Do".to_string(),
                    category: super::super::StatusCategory::ToDo,
                },
                assignee: None,
                epic_key: opts.epic_key,
                labels: opts.labels,
                description: opts.description,
            })
        }

        fn get_issue(&self, key: &str) -> Result<super::super::Issue, TrackerError> {
            Ok(super::super::Issue {
                key: key.to_string(),
                summary: "Mock issue".to_string(),
                issue_type: IssueType::Task,
                status: super::super::IssueStatus {
                    name: "To Do".to_string(),
                    category: super::super::StatusCategory::ToDo,
                },
                assignee: None,
                epic_key: None,
                labels: vec![],
                description: None,
            })
        }

        fn list_issues(
            &self,
            _project: &str,
            _epic_key: Option<&str>,
        ) -> Result<Vec<super::super::Issue>, TrackerError> {
            Ok(vec![])
        }

        fn transition_issue(&self, _opts: TransitionOpts) -> Result<(), TrackerError> {
            Ok(())
        }

        fn delete_issue(&self, _key: &str) -> Result<(), TrackerError> {
            Ok(())
        }

        fn create_project(&self, _opts: super::super::CreateProjectOpts) -> Result<super::super::TrackerProject, TrackerError> {
            Ok(super::super::TrackerProject {
                key: "TEST".to_string(),
                name: "Test".to_string(),
                project_type: "software".to_string(),
            })
        }

        fn tracker_type(&self) -> super::super::TrackerType {
            super::super::TrackerType::Jira
        }
    }

    /// Mock that tracks create/transition calls and can return different statuses
    struct CountingTracker {
        creates: AtomicUsize,
        transitions: AtomicUsize,
        /// Status returned by get_issue (simulates current Jira state)
        jira_status: String,
    }

    impl CountingTracker {
        fn new(jira_status: &str) -> Self {
            Self {
                creates: AtomicUsize::new(0),
                transitions: AtomicUsize::new(0),
                jira_status: jira_status.to_string(),
            }
        }
    }

    impl IssueTracker for CountingTracker {
        fn auth_status(&self) -> Result<super::super::AuthStatus, TrackerError> {
            Ok(super::super::AuthStatus {
                authenticated: true,
                user: Some("test".to_string()),
                email: Some("test@test.com".to_string()),
                domain: "test.atlassian.net".to_string(),
            })
        }
        fn list_projects(&self) -> Result<Vec<super::super::TrackerProject>, TrackerError> {
            Ok(vec![])
        }
        fn create_issue(
            &self,
            opts: CreateIssueOpts,
        ) -> Result<super::super::Issue, TrackerError> {
            let n = self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(super::super::Issue {
                key: format!("{}-{}", opts.project, n + 100),
                summary: opts.summary,
                issue_type: opts.issue_type,
                status: super::super::IssueStatus {
                    name: "To Do".to_string(),
                    category: super::super::StatusCategory::ToDo,
                },
                assignee: None,
                epic_key: opts.epic_key,
                labels: opts.labels,
                description: opts.description,
            })
        }
        fn get_issue(&self, key: &str) -> Result<super::super::Issue, TrackerError> {
            Ok(super::super::Issue {
                key: key.to_string(),
                summary: "Mock".to_string(),
                issue_type: IssueType::Task,
                status: super::super::IssueStatus {
                    name: self.jira_status.clone(),
                    category: super::super::StatusCategory::ToDo,
                },
                assignee: None,
                epic_key: None,
                labels: vec![],
                description: None,
            })
        }
        fn list_issues(
            &self,
            _p: &str,
            _e: Option<&str>,
        ) -> Result<Vec<super::super::Issue>, TrackerError> {
            Ok(vec![])
        }
        fn transition_issue(&self, _opts: TransitionOpts) -> Result<(), TrackerError> {
            self.transitions.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn delete_issue(&self, _key: &str) -> Result<(), TrackerError> {
            Ok(())
        }
        fn create_project(&self, _opts: super::super::CreateProjectOpts) -> Result<super::super::TrackerProject, TrackerError> {
            Ok(super::super::TrackerProject {
                key: "TEST".to_string(),
                name: "Test".to_string(),
                project_type: "software".to_string(),
            })
        }
        fn tracker_type(&self) -> super::super::TrackerType {
            super::super::TrackerType::Jira
        }
    }

    /// Mock that fails on create_issue
    struct FailingCreateTracker;

    impl IssueTracker for FailingCreateTracker {
        fn auth_status(&self) -> Result<super::super::AuthStatus, TrackerError> {
            Ok(super::super::AuthStatus {
                authenticated: true,
                user: None,
                email: None,
                domain: "test".to_string(),
            })
        }
        fn list_projects(&self) -> Result<Vec<super::super::TrackerProject>, TrackerError> {
            Ok(vec![])
        }
        fn create_issue(
            &self,
            _opts: CreateIssueOpts,
        ) -> Result<super::super::Issue, TrackerError> {
            Err(TrackerError::Api {
                status: 403,
                message: "Permission denied".to_string(),
            })
        }
        fn get_issue(&self, _key: &str) -> Result<super::super::Issue, TrackerError> {
            Err(TrackerError::NotFound {
                key: "N/A".to_string(),
            })
        }
        fn list_issues(
            &self,
            _p: &str,
            _e: Option<&str>,
        ) -> Result<Vec<super::super::Issue>, TrackerError> {
            Ok(vec![])
        }
        fn transition_issue(&self, _opts: TransitionOpts) -> Result<(), TrackerError> {
            Err(TrackerError::Api {
                status: 400,
                message: "Bad request".to_string(),
            })
        }
        fn delete_issue(&self, _key: &str) -> Result<(), TrackerError> {
            Ok(())
        }
        fn create_project(&self, _opts: super::super::CreateProjectOpts) -> Result<super::super::TrackerProject, TrackerError> {
            Ok(super::super::TrackerProject {
                key: "TEST".to_string(),
                name: "Test".to_string(),
                project_type: "software".to_string(),
            })
        }
        fn tracker_type(&self) -> super::super::TrackerType {
            super::super::TrackerType::Jira
        }
    }

    // ---- Additional sync engine tests ----

    #[test]
    fn test_run_sync_creates_and_writes_binding() {
        // Non-dry-run: should create issue AND write uuid+task_key back to file
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0001-new.draft.md"), SAMPLE_RFC_NO_BINDING).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.created, 1);
        assert_eq!(tracker.creates.load(Ordering::SeqCst), 1);

        // Verify the file was updated with binding
        let content = std::fs::read_to_string(dir.path().join("0001-new.draft.md")).unwrap();
        let binding = parse_jira_binding(&content);
        assert!(
            binding.blue_uuid.is_some(),
            "UUID should have been minted and written"
        );
        assert_eq!(
            binding.task_key.as_deref(),
            Some("TEST-100"),
            "Task key should have been written"
        );
    }

    #[test]
    fn test_run_sync_transitions_status() {
        // RFC has task_key, Jira status differs from expected → transition
        let dir = tempfile::tempdir().unwrap();
        // This RFC is "in-progress" which maps to Jira "In Progress"
        // But the mock returns "To Do" → should trigger transition
        let rfc = r#"# RFC 0050: Active Work

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-03-12 |
| **Jira** | TEST-42 |
| **Blue UUID** | existing-uuid-1234 |

---

## Summary

Active work.
"#;
        std::fs::write(dir.path().join("0050-active.wip.md"), rfc).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do"); // Jira says "To Do", RFC says "In Progress"
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.transitioned, 1);
        assert_eq!(tracker.transitions.load(Ordering::SeqCst), 1);
        assert_eq!(tracker.creates.load(Ordering::SeqCst), 0); // no creates
    }

    #[test]
    fn test_run_sync_no_transition_when_status_matches() {
        // RFC "draft" maps to "To Do", mock returns "To Do" → up-to-date
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0063-jira.md"), SAMPLE_RFC).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "PROJ".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.up_to_date, 1);
        assert_eq!(report.transitioned, 0);
        assert_eq!(tracker.transitions.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_run_sync_create_error_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0001-fail.draft.md"), SAMPLE_RFC_NO_BINDING).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = FailingCreateTracker;
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.errors, 1);
        assert_eq!(report.results[0].action, SyncAction::Error);
        assert!(report.results[0].error.is_some());
        assert!(report.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Permission denied"));
    }

    #[test]
    fn test_run_sync_multiple_rfcs_mixed() {
        let dir = tempfile::tempdir().unwrap();

        // RFC 1: no binding → create
        std::fs::write(dir.path().join("0001-new.draft.md"), SAMPLE_RFC_NO_BINDING).unwrap();
        // RFC 2: has binding, up-to-date
        std::fs::write(dir.path().join("0063-jira.draft.md"), SAMPLE_RFC).unwrap();
        // RFC 3: has task_key, needs transition (in-progress but Jira says "To Do")
        let in_progress_rfc = r#"# RFC 0080: Transitioning

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-03-12 |
| **Jira** | TEST-55 |
| **Blue UUID** | uuid-for-80 |

---

## Summary

Needs transition.
"#;
        std::fs::write(dir.path().join("0080-transitioning.wip.md"), in_progress_rfc).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.results.len(), 3);
        assert_eq!(report.created, 1);
        assert_eq!(report.transitioned, 1);
        assert_eq!(report.up_to_date, 1);
    }

    #[test]
    fn test_run_sync_uuid_minted_even_with_existing_task_key() {
        // RFC has task_key but no UUID → should mint UUID
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("0070-partial.draft.md"), SAMPLE_RFC_PARTIAL).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "BLUE".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let _report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        // Should not create a new issue (task_key exists), but should mint UUID
        assert_eq!(tracker.creates.load(Ordering::SeqCst), 0);

        let content = std::fs::read_to_string(dir.path().join("0070-partial.draft.md")).unwrap();
        let binding = parse_jira_binding(&content);
        assert!(binding.blue_uuid.is_some(), "UUID should have been minted");
        assert_eq!(binding.task_key.as_deref(), Some("BLUE-42")); // preserved
    }

    #[test]
    fn test_run_sync_dry_run_no_file_writes() {
        let dir = tempfile::tempdir().unwrap();
        let original = SAMPLE_RFC_NO_BINDING;
        std::fs::write(dir.path().join("0001-test.md"), original).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let report = run_sync(&tracker, &config, dir.path(), true).unwrap();

        assert_eq!(report.created, 1); // reports as would-create
        assert_eq!(tracker.creates.load(Ordering::SeqCst), 0); // but didn't actually call API

        // File should be untouched
        let after = std::fs::read_to_string(dir.path().join("0001-test.md")).unwrap();
        assert_eq!(after, original);
    }

    #[test]
    fn test_run_sync_superseded_rfc_skipped() {
        // "superseded" has no Jira mapping → should be up-to-date (no transition)
        let dir = tempfile::tempdir().unwrap();
        let rfc = r#"# RFC 0030: Old

| | |
|---|---|
| **Status** | Superseded |
| **Date** | 2026-01-01 |
| **Jira** | TEST-10 |
| **Blue UUID** | old-uuid |

---

## Summary

Replaced.
"#;
        std::fs::write(dir.path().join("0030-old.superseded.md"), rfc).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: DriftPolicy::Warn,
        };

        let tracker = CountingTracker::new("To Do");
        let report = run_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.up_to_date, 1); // no Jira mapping for "superseded"
        assert_eq!(tracker.transitions.load(Ordering::SeqCst), 0);
    }
}
