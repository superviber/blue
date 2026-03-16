//! ID auto-increment for epics and stories with Jira collision check
//!
//! Epics: org key namespace (TMS-01, TMS-02, ...)
//! Stories: area key namespace (CON-001, MER-002, ...)
//!
//! Before assigning an ID, queries both the local PM repo and Jira
//! to ensure no collisions with issues created outside Blue.

use std::path::Path;
use regex::Regex;

use crate::tracker::{IssueTracker, TrackerError};

use super::domain::PmDomain;

#[derive(Debug, thiserror::Error)]
pub enum IdError {
    #[error("No Jira project_key configured in domain.yaml")]
    NoProjectKey,

    #[error("Tracker error: {0}")]
    Tracker(#[from] TrackerError),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Invalid ID format: {0}")]
    InvalidFormat(String),
}

/// Parse a prefixed ID like "TMS-01" or "BKD-003" into (prefix, number)
pub fn parse_id(id: &str) -> Option<(&str, u32)> {
    let parts: Vec<&str> = id.rsplitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }
    let num: u32 = parts[0].parse().ok()?;
    Some((parts[1], num))
}

/// Format an epic ID: TMS-01, TMS-02, etc.
pub fn format_epic_id(org_key: &str, num: u32) -> String {
    format!("{}-{:02}", org_key, num)
}

/// Format a story ID: CON-001, MER-002, etc.
pub fn format_story_id(key: &str, num: u32) -> String {
    format!("{}-{:03}", key, num)
}

/// Scan the PM repo's epics/ directory for existing epic IDs with the given prefix.
/// Returns the highest number found.
fn scan_local_epic_ids(pm_repo_root: &Path, org_key: &str) -> Result<u32, IdError> {
    let epics_dir = pm_repo_root.join("epics");
    if !epics_dir.exists() {
        return Ok(0);
    }

    let pattern = format!(r"^{}-(\d+)", regex::escape(org_key));
    let re = Regex::new(&pattern).map_err(|e| IdError::InvalidFormat(e.to_string()))?;
    let mut max_num = 0u32;

    let entries = std::fs::read_dir(&epics_dir)
        .map_err(|e| IdError::Io(format!("read epics/: {}", e)))?;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(caps) = re.captures(&name_str) {
            if let Some(m) = caps.get(1) {
                if let Ok(n) = m.as_str().parse::<u32>() {
                    max_num = max_num.max(n);
                }
            }
        }
    }

    Ok(max_num)
}

/// Scan the PM repo's epics/ subdirectories for existing story IDs with the given area key prefix.
/// Returns the highest number found.
fn scan_local_story_ids(pm_repo_root: &Path, area_key: &str) -> Result<u32, IdError> {
    let epics_dir = pm_repo_root.join("epics");
    if !epics_dir.exists() {
        return Ok(0);
    }

    let pattern = format!(r"^{}-(\d+)", regex::escape(area_key));
    let re = Regex::new(&pattern).map_err(|e| IdError::InvalidFormat(e.to_string()))?;
    let mut max_num = 0u32;

    // Scan all epic subdirectories for story files
    let epic_dirs = std::fs::read_dir(&epics_dir)
        .map_err(|e| IdError::Io(format!("read epics/: {}", e)))?;

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
            let name = file_entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(caps) = re.captures(&name_str) {
                if let Some(m) = caps.get(1) {
                    if let Ok(n) = m.as_str().parse::<u32>() {
                        max_num = max_num.max(n);
                    }
                }
            }
        }
    }

    Ok(max_num)
}

/// Query Jira for the highest issue number matching a summary prefix pattern.
/// Searches issue summaries that start with the key prefix to detect
/// issues created directly in Jira.
fn scan_jira_ids(
    tracker: &dyn IssueTracker,
    project_key: &str,
    id_prefix: &str,
) -> Result<u32, IdError> {
    // List all issues in the project and check summaries for our prefix pattern
    let issues = tracker.list_issues(project_key, None)?;

    let pattern = format!(r"(?:^|\[){}-(\d+)", regex::escape(id_prefix));
    let re = Regex::new(&pattern).map_err(|e| IdError::InvalidFormat(e.to_string()))?;
    let mut max_num = 0u32;

    for issue in &issues {
        // Check summary for ID references
        if let Some(caps) = re.captures(&issue.summary) {
            if let Some(m) = caps.get(1) {
                if let Ok(n) = m.as_str().parse::<u32>() {
                    max_num = max_num.max(n);
                }
            }
        }
        // Check labels for ID references
        for label in &issue.labels {
            if let Some(caps) = re.captures(label) {
                if let Some(m) = caps.get(1) {
                    if let Ok(n) = m.as_str().parse::<u32>() {
                        max_num = max_num.max(n);
                    }
                }
            }
        }
    }

    Ok(max_num)
}

/// Allocate the next epic ID, checking both local files and Jira for collisions.
///
/// Returns the next available ID like "TMS-03".
pub fn next_epic_id(
    pm_repo_root: &Path,
    domain: &PmDomain,
    tracker: Option<&dyn IssueTracker>,
) -> Result<String, IdError> {
    let local_max = scan_local_epic_ids(pm_repo_root, &domain.key)?;

    let jira_max = match (tracker, domain.jira_project_key()) {
        (Some(t), Some(pk)) => scan_jira_ids(t, pk, &domain.key).unwrap_or(0),
        _ => 0,
    };

    let next = local_max.max(jira_max) + 1;
    Ok(format_epic_id(&domain.key, next))
}

/// Allocate the next story ID for an area key, checking both local files and Jira.
///
/// Returns the next available ID like "CON-003".
pub fn next_story_id(
    pm_repo_root: &Path,
    domain: &PmDomain,
    area_key: &str,
    tracker: Option<&dyn IssueTracker>,
) -> Result<String, IdError> {
    let local_max = scan_local_story_ids(pm_repo_root, area_key)?;

    let jira_max = match (tracker, domain.jira_project_key()) {
        (Some(t), Some(pk)) => scan_jira_ids(t, pk, area_key).unwrap_or(0),
        _ => 0,
    };

    let next = local_max.max(jira_max) + 1;
    Ok(format_story_id(area_key, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_id() {
        assert_eq!(parse_id("TMS-01"), Some(("TMS", 1)));
        assert_eq!(parse_id("BKD-003"), Some(("BKD", 3)));
        assert_eq!(parse_id("FRD-100"), Some(("FRD", 100)));
        assert!(parse_id("invalid").is_none());
        assert!(parse_id("").is_none());
    }

    #[test]
    fn test_format_epic_id() {
        assert_eq!(format_epic_id("TMS", 1), "TMS-01");
        assert_eq!(format_epic_id("TMS", 12), "TMS-12");
    }

    #[test]
    fn test_format_story_id() {
        assert_eq!(format_story_id("BKD", 1), "BKD-001");
        assert_eq!(format_story_id("BKD", 42), "BKD-042");
        assert_eq!(format_story_id("FRD", 100), "FRD-100");
    }

    #[test]
    fn test_scan_local_epic_ids_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(scan_local_epic_ids(dir.path(), "TMS").unwrap(), 0);
    }

    #[test]
    fn test_scan_local_epic_ids() {
        let dir = tempfile::tempdir().unwrap();
        let epics = dir.path().join("epics");
        std::fs::create_dir_all(epics.join("TMS-01-party-system")).unwrap();
        std::fs::create_dir_all(epics.join("TMS-02-discovery")).unwrap();
        std::fs::create_dir_all(epics.join("OTHER-01-unrelated")).unwrap();

        assert_eq!(scan_local_epic_ids(dir.path(), "TMS").unwrap(), 2);
        assert_eq!(scan_local_epic_ids(dir.path(), "OTHER").unwrap(), 1);
    }

    #[test]
    fn test_scan_local_story_ids() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party-system");
        std::fs::create_dir_all(&epic_dir).unwrap();

        std::fs::write(epic_dir.join("BKD-001-create-party-api.md"), "").unwrap();
        std::fs::write(epic_dir.join("BKD-002-party-invites.md"), "").unwrap();
        std::fs::write(epic_dir.join("FRD-001-party-ui.md"), "").unwrap();
        std::fs::write(epic_dir.join("_epic.md"), "").unwrap();

        assert_eq!(scan_local_story_ids(dir.path(), "BKD").unwrap(), 2);
        assert_eq!(scan_local_story_ids(dir.path(), "FRD").unwrap(), 1);
        assert_eq!(scan_local_story_ids(dir.path(), "PRD").unwrap(), 0);
    }

    #[test]
    fn test_scan_local_story_ids_across_epics() {
        let dir = tempfile::tempdir().unwrap();
        let epic1 = dir.path().join("epics").join("TMS-01-party");
        let epic2 = dir.path().join("epics").join("TMS-02-discovery");
        std::fs::create_dir_all(&epic1).unwrap();
        std::fs::create_dir_all(&epic2).unwrap();

        std::fs::write(epic1.join("BKD-001-api.md"), "").unwrap();
        std::fs::write(epic2.join("BKD-003-ai.md"), "").unwrap(); // gap: no BKD-002

        // Should find max = 3 even though 002 is missing
        assert_eq!(scan_local_story_ids(dir.path(), "BKD").unwrap(), 3);
    }

    #[test]
    fn test_next_epic_id_no_tracker() {
        let dir = tempfile::tempdir().unwrap();
        let epics = dir.path().join("epics");
        std::fs::create_dir_all(epics.join("TMS-01-party")).unwrap();
        std::fs::create_dir_all(epics.join("TMS-02-discovery")).unwrap();

        let domain = PmDomain {
            org: "test".to_string(),
            key: "TMS".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };

        let id = next_epic_id(dir.path(), &domain, None).unwrap();
        assert_eq!(id, "TMS-03");
    }

    #[test]
    fn test_next_story_id_no_tracker() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();
        std::fs::write(epic_dir.join("BKD-001-api.md"), "").unwrap();
        std::fs::write(epic_dir.join("BKD-002-invites.md"), "").unwrap();

        let domain = PmDomain {
            org: "test".to_string(),
            key: "TMS".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };

        let id = next_story_id(dir.path(), &domain, "BKD", None).unwrap();
        assert_eq!(id, "BKD-003");
    }

    #[test]
    fn test_next_epic_id_empty() {
        let dir = tempfile::tempdir().unwrap();
        let domain = PmDomain {
            org: "test".to_string(),
            key: "TMS".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };

        let id = next_epic_id(dir.path(), &domain, None).unwrap();
        assert_eq!(id, "TMS-01");
    }

    #[test]
    fn test_next_story_id_area_keys() {
        let dir = tempfile::tempdir().unwrap();
        let epic_dir = dir.path().join("epics").join("TMS-01-party");
        std::fs::create_dir_all(&epic_dir).unwrap();
        std::fs::write(epic_dir.join("CON-001-create-party.md"), "").unwrap();
        std::fs::write(epic_dir.join("CON-002-invites.md"), "").unwrap();
        std::fs::write(epic_dir.join("MER-001-merchant.md"), "").unwrap();

        let domain = PmDomain {
            org: "test".to_string(),
            key: "TMS".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };

        let con_id = next_story_id(dir.path(), &domain, "CON", None).unwrap();
        assert_eq!(con_id, "CON-003");

        let mer_id = next_story_id(dir.path(), &domain, "MER", None).unwrap();
        assert_eq!(mer_id, "MER-002");
    }
}
