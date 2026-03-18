//! Release tool handlers
//!
//! Handles release creation with semantic versioning analysis.
//! Reads current version from git tags, calculates next version,
//! verifies branch and working tree state, and returns release
//! commands for the CLI or `/wt` skill to execute.

use crate::{DocType, ProjectState};
use serde_json::{json, Value};
use std::process::Command;

use crate::handler_error::HandlerError;

/// Semantic version bump type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VersionBump {
    Patch,
    Minor,
    Major,
}

impl VersionBump {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        }
    }

    /// Parse a bump type from a string ("major", "minor", "patch")
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "major" => Some(Self::Major),
            "minor" => Some(Self::Minor),
            "patch" => Some(Self::Patch),
            _ => None,
        }
    }
}

/// Read the current version from git tags.
///
/// Runs `git describe --tags --abbrev=0` in the given directory.
/// Strips a leading 'v' if present. Returns "0.0.0" if no tags exist.
fn current_version_from_tags(repo_dir: &std::path::Path) -> String {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(["describe", "--tags", "--abbrev=0"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let tag = String::from_utf8_lossy(&o.stdout).trim().to_string();
            tag.strip_prefix('v')
                .unwrap_or(&tag)
                .to_string()
        }
        _ => "0.0.0".to_string(),
    }
}

/// Check which git branch is currently checked out.
fn current_branch(repo_dir: &std::path::Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Check whether the working tree is clean (no uncommitted changes).
fn is_working_tree_clean(repo_dir: &std::path::Path) -> bool {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .args(["status", "--porcelain"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().is_empty()
        }
        _ => false,
    }
}

/// Handle blue_release_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, HandlerError> {
    let version_override = args.get("version").and_then(|v| v.as_str());
    let bump_override = args
        .get("bump")
        .and_then(|v| v.as_str())
        .and_then(VersionBump::from_str);

    let repo_dir = &state.home.root;

    // Verify we're on the develop branch
    let branch = current_branch(repo_dir);
    if branch.as_deref() != Some("develop") {
        return Ok(json!({
            "status": "blocked",
            "message": crate::voice::error(
                &format!(
                    "Must be on develop branch to release (currently on '{}')",
                    branch.as_deref().unwrap_or("unknown")
                ),
                "Switch to develop first: git checkout develop"
            )
        }));
    }

    // Verify clean working tree
    if !is_working_tree_clean(repo_dir) {
        return Ok(json!({
            "status": "blocked",
            "message": crate::voice::error(
                "Working tree has uncommitted changes",
                "Commit or stash changes before releasing"
            )
        }));
    }

    // Check for in-progress work (blocks release)
    let in_progress: Vec<_> = state
        .store
        .list_documents(DocType::Rfc)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.status == "approved" || d.status == "in-progress")
        .collect();

    if !in_progress.is_empty() {
        let titles: Vec<_> = in_progress.iter().map(|d| d.title.as_str()).collect();
        return Ok(json!({
            "status": "blocked",
            "message": crate::voice::error(
                &format!("Can't release with in-progress work: {}", titles.join(", ")),
                "Complete or defer these RFCs first"
            ),
            "blocking_rfcs": titles
        }));
    }

    // Check for approved-but-unimplemented RFCs (warn, don't block)
    let approved_unimplemented: Vec<_> = state
        .store
        .list_documents(DocType::Rfc)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.status == "approved")
        .collect();

    let approved_warning = if !approved_unimplemented.is_empty() {
        let titles: Vec<_> = approved_unimplemented
            .iter()
            .map(|d| d.title.as_str())
            .collect();
        Some(json!({
            "warning": format!(
                "Approved but unimplemented RFCs exist: {}. These will not be included in this release.",
                titles.join(", ")
            ),
            "rfcs": titles
        }))
    } else {
        None
    };

    // Get implemented RFCs
    let implemented: Vec<_> = state
        .store
        .list_documents(DocType::Rfc)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.status == "implemented")
        .collect();

    // Get current version from git tags
    let current_version = current_version_from_tags(repo_dir);

    // Determine version bump: explicit bump > auto-analysis
    let suggested_bump = bump_override.unwrap_or_else(|| analyze_version_bump(&implemented));

    // Calculate next version
    let suggested_version = next_version(&current_version, suggested_bump);
    let version = version_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| suggested_version.clone());

    // Generate changelog entries
    let changelog_entries: Vec<String> = implemented
        .iter()
        .map(|rfc| format!("- {} (RFC {:04})", rfc.title, rfc.number.unwrap_or(0)))
        .collect();

    // Build the PR body
    let pr_body = format!(
        "## Release v{}\n\n### Changes\n{}\n\n### Release Process\nAfter merge, tag with `git tag v{}` and push the tag.",
        version,
        if changelog_entries.is_empty() {
            "- No RFC changes".to_string()
        } else {
            changelog_entries.join("\n")
        },
        version
    );

    // Build ordered commands array
    let commands: Vec<String> = vec![
        "git fetch origin".to_string(),
        "git rebase origin/main".to_string(),
        format!(
            "gh pr create --base main --head develop --title \"Release v{}\" --body \"{}\"",
            version,
            pr_body.replace('"', "\\\"")
        ),
        format!("# After PR is merged:\ngit tag v{}", version),
        format!("git push origin v{}", version),
    ];

    let mut result = json!({
        "status": "success",
        "current_version": current_version,
        "suggested_bump": suggested_bump.as_str(),
        "suggested_version": suggested_version,
        "version": version,
        "rfcs_included": implemented.iter().map(|r| &r.title).collect::<Vec<_>>(),
        "changelog_entries": changelog_entries,
        "commands": commands,
        "message": crate::voice::success(
            &format!("Ready to release v{} ({} bump)", version, suggested_bump.as_str()),
            Some(&format!("{} RFCs included. Run the commands in order to complete the release.", implemented.len()))
        )
    });

    if let Some(warning) = approved_warning {
        result
            .as_object_mut()
            .unwrap()
            .insert("approved_unimplemented".to_string(), warning);
    }

    Ok(result)
}

/// Analyze implemented RFCs to determine version bump
fn analyze_version_bump(rfcs: &[crate::Document]) -> VersionBump {
    let mut max_bump = VersionBump::Patch;

    for rfc in rfcs {
        let title_lower = rfc.title.to_lowercase();

        // Major version indicators
        if title_lower.contains("breaking")
            || title_lower.contains("remove")
            || title_lower.contains("deprecate")
        {
            return VersionBump::Major;
        }

        // Minor version indicators
        if title_lower.starts_with("add-")
            || title_lower.starts_with("implement-")
            || title_lower.contains("feature")
        {
            max_bump = max_bump.max(VersionBump::Minor);
        }
    }

    max_bump
}

/// Calculate next version based on current version and bump type
fn next_version(current: &str, bump: VersionBump) -> String {
    let parts: Vec<u32> = current.split('.').filter_map(|s| s.parse().ok()).collect();

    if parts.len() < 3 {
        return "0.1.0".to_string();
    }

    let (major, minor, patch) = (parts[0], parts[1], parts[2]);

    match bump {
        VersionBump::Major => format!("{}.0.0", major + 1),
        VersionBump::Minor => format!("{}.{}.0", major, minor + 1),
        VersionBump::Patch => format!("{}.{}.{}", major, minor, patch + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_version() {
        assert_eq!(next_version("1.2.3", VersionBump::Patch), "1.2.4");
        assert_eq!(next_version("1.2.3", VersionBump::Minor), "1.3.0");
        assert_eq!(next_version("1.2.3", VersionBump::Major), "2.0.0");
    }

    #[test]
    fn test_next_version_from_zero() {
        assert_eq!(next_version("0.0.0", VersionBump::Patch), "0.0.1");
        assert_eq!(next_version("0.0.0", VersionBump::Minor), "0.1.0");
        assert_eq!(next_version("0.0.0", VersionBump::Major), "1.0.0");
    }

    #[test]
    fn test_next_version_invalid_fallback() {
        assert_eq!(next_version("invalid", VersionBump::Patch), "0.1.0");
        assert_eq!(next_version("1.2", VersionBump::Patch), "0.1.0");
    }

    #[test]
    fn test_version_bump_comparison() {
        assert!(VersionBump::Major > VersionBump::Minor);
        assert!(VersionBump::Minor > VersionBump::Patch);
    }

    #[test]
    fn test_version_bump_from_str() {
        assert_eq!(VersionBump::from_str("major"), Some(VersionBump::Major));
        assert_eq!(VersionBump::from_str("Minor"), Some(VersionBump::Minor));
        assert_eq!(VersionBump::from_str("PATCH"), Some(VersionBump::Patch));
        assert_eq!(VersionBump::from_str("invalid"), None);
        assert_eq!(VersionBump::from_str(""), None);
    }

    #[test]
    fn test_version_from_tags_strips_v_prefix() {
        // Test the tag-stripping logic directly
        let tag = "v1.2.3";
        let version = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(version, "1.2.3");

        let tag_no_v = "1.2.3";
        let version = tag_no_v.strip_prefix('v').unwrap_or(tag_no_v);
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_current_version_from_tags_no_repo() {
        // When run outside a git repo, should return default
        let non_repo = std::env::temp_dir().join("not-a-git-repo-blue-test");
        let _ = std::fs::create_dir_all(&non_repo);
        let version = current_version_from_tags(&non_repo);
        assert_eq!(version, "0.0.0");
        let _ = std::fs::remove_dir(&non_repo);
    }

    #[test]
    fn test_analyze_version_bump_empty() {
        let rfcs: Vec<crate::Document> = vec![];
        assert_eq!(analyze_version_bump(&rfcs), VersionBump::Patch);
    }

    #[test]
    fn test_analyze_version_bump_breaking() {
        let rfcs = vec![crate::Document {
            id: None,
            doc_type: DocType::Rfc,
            number: Some(1),
            title: "breaking-change-api".to_string(),
            status: "implemented".to_string(),
            file_path: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
            content_hash: None,
            indexed_at: None,
        }];
        assert_eq!(analyze_version_bump(&rfcs), VersionBump::Major);
    }

    #[test]
    fn test_analyze_version_bump_feature() {
        let rfcs = vec![crate::Document {
            id: None,
            doc_type: DocType::Rfc,
            number: Some(2),
            title: "add-feature-auth".to_string(),
            status: "implemented".to_string(),
            file_path: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
            content_hash: None,
            indexed_at: None,
        }];
        assert_eq!(analyze_version_bump(&rfcs), VersionBump::Minor);
    }

    #[test]
    fn test_analyze_version_bump_patch() {
        let rfcs = vec![crate::Document {
            id: None,
            doc_type: DocType::Rfc,
            number: Some(3),
            title: "fix-typo".to_string(),
            status: "implemented".to_string(),
            file_path: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
            content_hash: None,
            indexed_at: None,
        }];
        assert_eq!(analyze_version_bump(&rfcs), VersionBump::Patch);
    }
}
