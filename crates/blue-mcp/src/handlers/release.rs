//! Release tool handlers
//!
//! Handles release creation with semantic versioning analysis.

use blue_core::{DocType, ProjectState};
use serde_json::{json, Value};

use crate::error::ServerError;

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
}

/// Handle blue_release_create
pub fn handle_create(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let version_override = args.get("version").and_then(|v| v.as_str());

    // Check for in-progress work
    let in_progress: Vec<_> = state
        .store
        .list_documents(DocType::Rfc)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.status == "in-progress")
        .collect();

    if !in_progress.is_empty() {
        let titles: Vec<_> = in_progress.iter().map(|d| d.title.as_str()).collect();
        return Ok(json!({
            "status": "blocked",
            "message": blue_core::voice::error(
                &format!("Can't release with in-progress work: {}", titles.join(", ")),
                "Complete or defer these RFCs first"
            ),
            "blocking_rfcs": titles
        }));
    }

    // Get implemented RFCs
    let implemented: Vec<_> = state
        .store
        .list_documents(DocType::Rfc)
        .unwrap_or_default()
        .into_iter()
        .filter(|d| d.status == "implemented")
        .collect();

    // Analyze version bump
    let suggested_bump = analyze_version_bump(&implemented);

    // Get current version (would read from Cargo.toml in real impl)
    let current_version = "0.1.0";

    // Calculate next version
    let suggested_version = next_version(current_version, suggested_bump);
    let version = version_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| suggested_version.clone());

    // Generate changelog entries
    let changelog_entries: Vec<String> = implemented
        .iter()
        .map(|rfc| format!("- {} (RFC {:04})", rfc.title, rfc.number.unwrap_or(0)))
        .collect();

    Ok(json!({
        "status": "success",
        "current_version": current_version,
        "suggested_bump": suggested_bump.as_str(),
        "suggested_version": suggested_version,
        "version": version,
        "rfcs_included": implemented.iter().map(|r| &r.title).collect::<Vec<_>>(),
        "changelog_entries": changelog_entries,
        "commands": {
            "create_pr": format!("gh pr create --base main --head develop --title 'Release {}'", version),
            "tag": format!("git tag v{}", version),
            "push_tag": format!("git push origin v{}", version)
        },
        "message": blue_core::voice::success(
            &format!("Ready to release {} ({} bump)", version, suggested_bump.as_str()),
            Some(&format!("{} RFCs included. Follow the commands to complete.", implemented.len()))
        )
    }))
}

/// Analyze implemented RFCs to determine version bump
fn analyze_version_bump(rfcs: &[blue_core::Document]) -> VersionBump {
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
    fn test_version_bump_comparison() {
        assert!(VersionBump::Major > VersionBump::Minor);
        assert!(VersionBump::Minor > VersionBump::Patch);
    }
}
