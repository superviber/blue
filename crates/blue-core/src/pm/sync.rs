//! PM repo sync — write-through projection from epics/ to Jira (RFC 0068)
//!
//! Scans a PM repo's `epics/` directory for YAML front matter files (epics and stories),
//! creates/transitions Jira issues, and writes back `jira_url` to the files.
//! Two-pass: create all issues first, then resolve `depends_on` links.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::tracker::{
    CreateIssueOpts, IssueTracker, IssueType, SyncAction, SyncConfig, SyncReport, SyncResult,
    TrackerError, TransitionOpts,
};

// ---------------------------------------------------------------------------
// YAML front matter types
// ---------------------------------------------------------------------------

/// Parsed YAML front matter from an epic or story file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmFrontMatter {
    /// "epic" or "story"
    #[serde(rename = "type")]
    pub item_type: String,

    /// Local ID (e.g., "TMS-01" or "CON-001")
    pub id: String,

    /// Area key (e.g., "CON" for Consumer App) — determines story ID prefix
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,

    /// Jira components (e.g., ["Engineering", "Security"]) — multi-select
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,

    /// Title
    pub title: String,

    /// Status (backlog, ready, in-progress, in-review, done, blocked)
    #[serde(default = "default_status")]
    pub status: String,

    /// Parent epic ID (stories only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epic: Option<String>,

    /// Target repo name (stories only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,

    /// Story points (stories only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points: Option<u32>,

    /// Sprint (stories only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprint: Option<String>,

    /// Priority (epics only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,

    /// Release (epics only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,

    /// Assignee
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    /// Labels
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,

    /// Dependencies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<serde_yaml::Value>,

    /// Jira URL (written back by sync)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jira_url: Option<String>,
}

fn default_status() -> String {
    "backlog".to_string()
}

/// A discovered PM item (epic or story) with its file path
#[derive(Debug, Clone)]
pub struct PmItem {
    pub front_matter: PmFrontMatter,
    pub file_path: PathBuf,
    pub raw_content: String,
}

// ---------------------------------------------------------------------------
// Status mapping
// ---------------------------------------------------------------------------

/// Map PM status to Jira target status name using the status_map from jira.toml.
/// Falls back to sensible defaults.
pub fn pm_status_to_jira(status: &str) -> Option<&'static str> {
    match status {
        "backlog" => Some("To Do"),
        "ready" => Some("To Do"),
        "in-progress" => Some("In Progress"),
        "in-review" => Some("In Progress"),
        "done" => Some("Done"),
        "blocked" => Some("In Progress"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// YAML front matter parsing
// ---------------------------------------------------------------------------

/// Parse YAML front matter from a markdown file.
///
/// Expects `---` delimited YAML block at the start of the file.
pub fn parse_pm_front_matter(content: &str) -> Option<PmFrontMatter> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }

    // Find closing ---
    let rest = &content[3..];
    let end = rest.find("\n---")?;
    let yaml_block = &rest[..end];

    serde_yaml::from_str(yaml_block).ok()
}

/// Write updated front matter back to a file, preserving content after the YAML block.
pub fn update_pm_front_matter(content: &str, fm: &PmFrontMatter) -> Option<String> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let rest = &trimmed[3..];
    let end = rest.find("\n---")?;
    let after_yaml = &rest[end + 4..]; // skip \n---

    let new_yaml = serde_yaml::to_string(fm).ok()?;
    Some(format!("---\n{}---{}", new_yaml, after_yaml))
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover all epic and story files in a PM repo's epics/ directory.
pub fn discover_pm_items(pm_repo_root: &Path) -> Result<Vec<PmItem>, TrackerError> {
    let epics_dir = pm_repo_root.join("epics");
    if !epics_dir.exists() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();

    let epic_dirs = std::fs::read_dir(&epics_dir)
        .map_err(|e| TrackerError::Http(format!("Failed to read epics/: {}", e)))?;

    for epic_entry in epic_dirs.flatten() {
        let epic_path = epic_entry.path();
        if !epic_path.is_dir() {
            continue;
        }

        let files = match std::fs::read_dir(&epic_path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file_entry in files.flatten() {
            let path = file_entry.path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(fm) = parse_pm_front_matter(&content) {
                items.push(PmItem {
                    front_matter: fm,
                    file_path: path,
                    raw_content: content,
                });
            }
        }
    }

    // Sort: epics first, then stories (alphabetically by ID)
    items.sort_by(|a, b| {
        let a_is_epic = a.front_matter.item_type == "epic";
        let b_is_epic = b.front_matter.item_type == "epic";
        b_is_epic
            .cmp(&a_is_epic)
            .then(a.front_matter.id.cmp(&b.front_matter.id))
    });

    Ok(items)
}

// ---------------------------------------------------------------------------
// Sync engine
// ---------------------------------------------------------------------------

/// Run PM sync for all epics and stories in the PM repo.
///
/// Two-pass:
/// 1. Create all issues (epics first, then stories)
/// 2. Resolve depends_on links (future: Phase 4)
///
/// Writes back `jira_url` to YAML front matter.
pub fn run_pm_sync(
    tracker: &dyn IssueTracker,
    config: &SyncConfig,
    pm_repo_root: &Path,
    dry_run: bool,
) -> Result<SyncReport, TrackerError> {
    let mut report = SyncReport::default();
    let items = discover_pm_items(pm_repo_root)?;

    if items.is_empty() {
        return Ok(report);
    }

    // Build epic ID → Jira key mapping for linking stories to epics
    let mut epic_jira_keys: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Pass 1: Create/sync all items
    for item in &items {
        let result = sync_pm_item(tracker, config, item, &epic_jira_keys, dry_run);

        // Track epic Jira keys for story linking
        if item.front_matter.item_type == "epic" {
            if let Some(ref key) = result.jira_key {
                epic_jira_keys.insert(item.front_matter.id.clone(), key.clone());
            }
        }

        match &result.action {
            SyncAction::Created => report.created += 1,
            SyncAction::Transitioned => report.transitioned += 1,
            SyncAction::UpToDate => report.up_to_date += 1,
            SyncAction::Error => report.errors += 1,
            SyncAction::Skipped => {}
        }
        report.results.push(result);
    }

    // Pass 2: depends_on link resolution (Phase 4 — future)
    // For now we just create the issues and link stories to epics.

    Ok(report)
}

/// Sync a single PM item (epic or story) against Jira.
fn sync_pm_item(
    tracker: &dyn IssueTracker,
    config: &SyncConfig,
    item: &PmItem,
    epic_jira_keys: &std::collections::HashMap<String, String>,
    dry_run: bool,
) -> SyncResult {
    let fm = &item.front_matter;
    let file_name = item
        .file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let display_title = format!("[{}] {}", fm.id, fm.title);

    // Already synced? Check jira_url
    if let Some(ref jira_url) = fm.jira_url {
        // Extract Jira key from URL
        let jira_key = jira_url
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_string();

        if jira_key.is_empty() {
            return SyncResult {
                rfc_title: display_title,
                rfc_file: file_name,
                action: SyncAction::Error,
                jira_key: None,
                error: Some(format!("Invalid jira_url: {}", jira_url)),
            };
        }

        // Check if status transition needed
        if let Some(target_status) = pm_status_to_jira(&fm.status) {
            if !dry_run {
                match tracker.get_issue(&jira_key) {
                    Ok(issue) => {
                        if issue.status.name != target_status {
                            match tracker.transition_issue(TransitionOpts {
                                key: jira_key.clone(),
                                target_status: target_status.to_string(),
                            }) {
                                Ok(()) => {
                                    return SyncResult {
                                        rfc_title: display_title,
                                        rfc_file: file_name,
                                        action: SyncAction::Transitioned,
                                        jira_key: Some(jira_key),
                                        error: None,
                                    };
                                }
                                Err(e) => {
                                    return SyncResult {
                                        rfc_title: display_title,
                                        rfc_file: file_name,
                                        action: SyncAction::Error,
                                        jira_key: Some(jira_key),
                                        error: Some(format!("Transition failed: {}", e)),
                                    };
                                }
                            }
                        }
                    }
                    Err(e) => {
                        return SyncResult {
                            rfc_title: display_title,
                            rfc_file: file_name,
                            action: SyncAction::Error,
                            jira_key: Some(jira_key),
                            error: Some(format!("Failed to get issue: {}", e)),
                        };
                    }
                }
            }

            return SyncResult {
                rfc_title: display_title,
                rfc_file: file_name,
                action: SyncAction::UpToDate,
                jira_key: Some(jira_key),
                error: None,
            };
        }

        return SyncResult {
            rfc_title: display_title,
            rfc_file: file_name,
            action: SyncAction::UpToDate,
            jira_key: Some(jira_key),
            error: None,
        };
    }

    // No jira_url — need to create
    let issue_type = if fm.item_type == "epic" {
        IssueType::Epic
    } else {
        IssueType::Story
    };

    // Resolve epic key for stories
    let epic_key = if fm.item_type == "story" {
        fm.epic
            .as_ref()
            .and_then(|epic_id| epic_jira_keys.get(epic_id))
            .cloned()
    } else {
        None
    };

    if dry_run {
        return SyncResult {
            rfc_title: display_title,
            rfc_file: file_name,
            action: SyncAction::Created,
            jira_key: None,
            error: None,
        };
    }

    let summary = format!("[{}] {}", fm.id, fm.title);
    let description = if fm.item_type == "story" {
        let mut desc = format!("Story: {}", fm.id);
        if let Some(ref area) = fm.area {
            desc.push_str(&format!("\nArea: {}", area));
        }
        if !fm.components.is_empty() {
            desc.push_str(&format!("\nComponents: {}", fm.components.join(", ")));
        }
        if let Some(ref repo) = fm.repo {
            desc.push_str(&format!("\nRepo: {}", repo));
        }
        if let Some(ref epic) = fm.epic {
            desc.push_str(&format!("\nEpic: {}", epic));
        }
        if let Some(points) = fm.points {
            desc.push_str(&format!("\nPoints: {}", points));
        }
        if let Some(ref sprint) = fm.sprint {
            desc.push_str(&format!("\nSprint: {}", sprint));
        }
        Some(desc)
    } else {
        let mut desc = format!("Epic: {}", fm.id);
        if let Some(ref release) = fm.release {
            desc.push_str(&format!("\nRelease: {}", release));
        }
        Some(desc)
    };

    let opts = CreateIssueOpts {
        project: config.project_key.clone(),
        issue_type,
        summary,
        description,
        epic_key,
        labels: {
            let mut labels = fm.labels.clone();
            labels.push("blue-sync".to_string());
            // Add area label for filtering
            if let Some(ref area) = fm.area {
                labels.push(format!("area:{}", area));
            }
            labels
        },
        components: fm.components.clone(),
    };

    match tracker.create_issue(opts) {
        Ok(issue) => {
            // Write back jira_url to file
            let jira_url = format!("https://{}/browse/{}", config.domain, issue.key);
            let mut updated_fm = fm.clone();
            updated_fm.jira_url = Some(jira_url);

            if let Some(new_content) = update_pm_front_matter(&item.raw_content, &updated_fm) {
                if let Err(e) = std::fs::write(&item.file_path, &new_content) {
                    return SyncResult {
                        rfc_title: display_title,
                        rfc_file: file_name,
                        action: SyncAction::Error,
                        jira_key: Some(issue.key),
                        error: Some(format!("Created issue but failed to write back: {}", e)),
                    };
                }
            }

            SyncResult {
                rfc_title: display_title,
                rfc_file: file_name,
                action: SyncAction::Created,
                jira_key: Some(issue.key),
                error: None,
            }
        }
        Err(e) => SyncResult {
            rfc_title: display_title,
            rfc_file: file_name,
            action: SyncAction::Error,
            jira_key: None,
            error: Some(format!("Failed to create issue: {}", e)),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPIC_YAML: &str = r#"---
type: epic
id: TMS-01
title: "Party System"
status: backlog
priority: 1
labels: [phase-0, core-social]
release: phase-0-mvp
---

## Description

The party system enables users to create and join social events.
"#;

    const STORY_YAML: &str = r#"---
type: story
id: CON-001
title: "Create Party API endpoint"
area: CON
components:
  - Engineering
epic: TMS-01
repo: themove-backend
status: backlog
points: 3
sprint: s01
labels: [api, auth]
---

## Description

REST endpoint for party creation.
"#;

    const STORY_WITH_JIRA: &str = r#"---
type: story
id: CON-002
title: "Party Invites API"
area: CON
components:
  - Engineering
epic: TMS-01
repo: themove-backend
status: in-progress
points: 2
sprint: s01
jira_url: https://themovesocial.atlassian.net/browse/SCRUM-42
---

## Description

Invite system.
"#;

    #[test]
    fn test_parse_pm_front_matter_epic() {
        let fm = parse_pm_front_matter(EPIC_YAML).unwrap();
        assert_eq!(fm.item_type, "epic");
        assert_eq!(fm.id, "TMS-01");
        assert_eq!(fm.title, "Party System");
        assert_eq!(fm.status, "backlog");
        assert_eq!(fm.priority, Some(1));
        assert_eq!(fm.labels, vec!["phase-0", "core-social"]);
        assert_eq!(fm.release.as_deref(), Some("phase-0-mvp"));
        assert!(fm.jira_url.is_none());
    }

    #[test]
    fn test_parse_pm_front_matter_story() {
        let fm = parse_pm_front_matter(STORY_YAML).unwrap();
        assert_eq!(fm.item_type, "story");
        assert_eq!(fm.id, "CON-001");
        assert_eq!(fm.title, "Create Party API endpoint");
        assert_eq!(fm.area.as_deref(), Some("CON"));
        assert_eq!(fm.components, vec!["Engineering"]);
        assert_eq!(fm.epic.as_deref(), Some("TMS-01"));
        assert_eq!(fm.repo.as_deref(), Some("themove-backend"));
        assert_eq!(fm.points, Some(3));
        assert_eq!(fm.sprint.as_deref(), Some("s01"));
    }

    #[test]
    fn test_parse_pm_front_matter_with_jira() {
        let fm = parse_pm_front_matter(STORY_WITH_JIRA).unwrap();
        assert_eq!(fm.id, "CON-002");
        assert_eq!(
            fm.jira_url.as_deref(),
            Some("https://themovesocial.atlassian.net/browse/SCRUM-42")
        );
    }

    #[test]
    fn test_parse_pm_front_matter_no_yaml() {
        assert!(parse_pm_front_matter("# Just a heading\n\nNo YAML here.").is_none());
        assert!(parse_pm_front_matter("").is_none());
    }

    #[test]
    fn test_update_pm_front_matter() {
        let fm = parse_pm_front_matter(STORY_YAML).unwrap();
        let mut updated = fm.clone();
        updated.jira_url = Some("https://example.atlassian.net/browse/PROJ-99".to_string());

        let result = update_pm_front_matter(STORY_YAML, &updated).unwrap();
        assert!(result.contains("jira_url: https://example.atlassian.net/browse/PROJ-99"));
        assert!(result.contains("## Description"));
    }

    #[test]
    fn test_pm_status_to_jira() {
        assert_eq!(pm_status_to_jira("backlog"), Some("To Do"));
        assert_eq!(pm_status_to_jira("ready"), Some("To Do"));
        assert_eq!(pm_status_to_jira("in-progress"), Some("In Progress"));
        assert_eq!(pm_status_to_jira("in-review"), Some("In Progress"));
        assert_eq!(pm_status_to_jira("done"), Some("Done"));
        assert_eq!(pm_status_to_jira("blocked"), Some("In Progress"));
        assert_eq!(pm_status_to_jira("unknown"), None);
    }

    #[test]
    fn test_discover_pm_items() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();

        std::fs::write(epic_dir.join("_epic.md"), EPIC_YAML).unwrap();
        std::fs::write(epic_dir.join("CON-001-api.md"), STORY_YAML).unwrap();
        std::fs::write(epic_dir.join("CON-002-invites.md"), STORY_WITH_JIRA).unwrap();

        let items = discover_pm_items(dir.path()).unwrap();
        assert_eq!(items.len(), 3);

        // Epics should come first
        assert_eq!(items[0].front_matter.item_type, "epic");
        assert_eq!(items[0].front_matter.id, "TMS-01");

        // Then stories sorted by ID
        assert_eq!(items[1].front_matter.id, "CON-001");
        assert_eq!(items[2].front_matter.id, "CON-002");
    }

    #[test]
    fn test_discover_pm_items_empty() {
        let dir = tempfile::tempdir().unwrap();
        let items = discover_pm_items(dir.path()).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_run_pm_sync_dry_run() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();

        std::fs::write(epic_dir.join("_epic.md"), EPIC_YAML).unwrap();
        std::fs::write(epic_dir.join("CON-001-api.md"), STORY_YAML).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: crate::tracker::sync::DriftPolicy::Warn,
        };

        let tracker = MockPmTracker::new();
        let report = run_pm_sync(&tracker, &config, dir.path(), true).unwrap();

        assert_eq!(report.created, 2); // epic + story
        assert_eq!(report.results.len(), 2);

        // Files should NOT be modified in dry-run
        let epic_content = std::fs::read_to_string(epic_dir.join("_epic.md")).unwrap();
        assert!(!epic_content.contains("jira_url"));
    }

    #[test]
    fn test_run_pm_sync_creates_and_writes_back() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();

        std::fs::write(epic_dir.join("_epic.md"), EPIC_YAML).unwrap();
        std::fs::write(epic_dir.join("CON-001-api.md"), STORY_YAML).unwrap();

        let config = SyncConfig {
            domain: "test.atlassian.net".to_string(),
            project_key: "TEST".to_string(),
            drift_policy: crate::tracker::sync::DriftPolicy::Warn,
        };

        let tracker = MockPmTracker::new();
        let report = run_pm_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.created, 2);

        // Verify jira_url written back
        let epic_content = std::fs::read_to_string(epic_dir.join("_epic.md")).unwrap();
        assert!(epic_content.contains("jira_url:"));
        assert!(epic_content.contains("test.atlassian.net"));

        let story_content = std::fs::read_to_string(epic_dir.join("CON-001-api.md")).unwrap();
        assert!(story_content.contains("jira_url:"));
    }

    #[test]
    fn test_run_pm_sync_skips_already_synced() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();

        std::fs::write(epic_dir.join("CON-002-invites.md"), STORY_WITH_JIRA).unwrap();

        let config = SyncConfig {
            domain: "themovesocial.atlassian.net".to_string(),
            project_key: "SCRUM".to_string(),
            drift_policy: crate::tracker::sync::DriftPolicy::Warn,
        };

        let tracker = MockPmTracker::new();
        let report = run_pm_sync(&tracker, &config, dir.path(), false).unwrap();

        assert_eq!(report.up_to_date, 1);
        assert_eq!(report.created, 0);
    }

    const STORY_MULTI_COMPONENT: &str = r#"---
type: story
id: CON-019
title: "Party auth token hardening"
area: CON
components:
  - Engineering
  - Security
epic: TMS-03
status: backlog
points: 5
labels: [security, auth]
---

## Description

Harden party auth tokens.
"#;

    #[test]
    fn test_parse_pm_front_matter_multi_component() {
        let fm = parse_pm_front_matter(STORY_MULTI_COMPONENT).unwrap();
        assert_eq!(fm.area.as_deref(), Some("CON"));
        assert_eq!(fm.components, vec!["Engineering", "Security"]);
    }

    // --- Mock tracker for PM sync tests ---

    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockPmTracker {
        create_counter: AtomicUsize,
    }

    impl MockPmTracker {
        fn new() -> Self {
            Self {
                create_counter: AtomicUsize::new(0),
            }
        }
    }

    impl IssueTracker for MockPmTracker {
        fn auth_status(&self) -> Result<crate::tracker::AuthStatus, TrackerError> {
            Ok(crate::tracker::AuthStatus {
                authenticated: true,
                user: Some("test".to_string()),
                email: Some("test@test.com".to_string()),
                domain: "test.atlassian.net".to_string(),
            })
        }

        fn list_projects(&self) -> Result<Vec<crate::tracker::TrackerProject>, TrackerError> {
            Ok(vec![])
        }

        fn create_issue(
            &self,
            opts: CreateIssueOpts,
        ) -> Result<crate::tracker::Issue, TrackerError> {
            let n = self.create_counter.fetch_add(1, Ordering::SeqCst);
            Ok(crate::tracker::Issue {
                key: format!("{}-{}", opts.project, n + 10),
                summary: opts.summary,
                issue_type: opts.issue_type,
                status: crate::tracker::IssueStatus {
                    name: "To Do".to_string(),
                    category: crate::tracker::StatusCategory::ToDo,
                },
                assignee: None,
                epic_key: opts.epic_key,
                labels: opts.labels,
                description: opts.description,
            })
        }

        fn get_issue(&self, key: &str) -> Result<crate::tracker::Issue, TrackerError> {
            // Return "In Progress" for SCRUM-42 (the story in STORY_WITH_JIRA)
            let status_name = if key == "SCRUM-42" {
                "In Progress"
            } else {
                "To Do"
            };
            Ok(crate::tracker::Issue {
                key: key.to_string(),
                summary: "Mock".to_string(),
                issue_type: IssueType::Task,
                status: crate::tracker::IssueStatus {
                    name: status_name.to_string(),
                    category: crate::tracker::StatusCategory::InProgress,
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
        ) -> Result<Vec<crate::tracker::Issue>, TrackerError> {
            Ok(vec![])
        }

        fn transition_issue(&self, _opts: TransitionOpts) -> Result<(), TrackerError> {
            Ok(())
        }

        fn delete_issue(&self, _key: &str) -> Result<(), TrackerError> {
            Ok(())
        }

        fn create_project(&self, _opts: crate::tracker::CreateProjectOpts) -> Result<crate::tracker::TrackerProject, TrackerError> {
            Ok(crate::tracker::TrackerProject {
                key: "TEST".to_string(),
                name: "Test".to_string(),
                project_type: "software".to_string(),
            })
        }

        fn tracker_type(&self) -> crate::tracker::TrackerType {
            crate::tracker::TrackerType::Jira
        }
    }
}
