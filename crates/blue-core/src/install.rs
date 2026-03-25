//! Blue install — set up hooks and skills in ~/.claude/ (RFC 0074)

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Home directory not found")]
    NoHome,
}

/// Skills embedded in the binary at compile time.
/// Each entry is (installed-name, SKILL.md content).
const SKILLS: &[(&str, &str)] = &[
    (
        "blue-alignment-expert",
        include_str!("../../../skills/alignment-expert/SKILL.md"),
    ),
    (
        "blue-alignment-play",
        include_str!("../../../skills/alignment-play/SKILL.md"),
    ),
    (
        "blue-domain-setup",
        include_str!("../../../skills/domain-setup/SKILL.md"),
    ),
    (
        "blue-wt",
        include_str!("../../../skills/wt/SKILL.md"),
    ),
    (
        "blue-org-context",
        include_str!("../../../skills/blue-org-context/SKILL.md"),
    ),
];

/// Result of an install or uninstall operation
pub struct InstallResult {
    pub skills_installed: Vec<String>,
    pub hooks_configured: Vec<String>,
    pub skills_removed: Vec<String>,
    pub hooks_removed: Vec<String>,
}

/// Get the Claude home directory (~/.claude)
fn claude_home() -> Result<PathBuf, InstallError> {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .ok_or(InstallError::NoHome)
}

/// Install Blue skills and hooks into ~/.claude/
pub fn install() -> Result<InstallResult, InstallError> {
    let claude_dir = claude_home()?;
    let skills_dir = claude_dir.join("skills");
    let settings_path = claude_dir.join("settings.json");

    let mut result = InstallResult {
        skills_installed: Vec::new(),
        hooks_configured: Vec::new(),
        skills_removed: Vec::new(),
        hooks_removed: Vec::new(),
    };

    // Install embedded skills as real directories (not symlinks)
    std::fs::create_dir_all(&skills_dir)?;

    for (name, content) in SKILLS {
        let skill_dir = skills_dir.join(name);
        std::fs::create_dir_all(&skill_dir)?;
        std::fs::write(skill_dir.join("SKILL.md"), content)?;
        result.skills_installed.push(name.to_string());
    }

    // Configure hooks in ~/.claude/settings.json
    configure_hooks(&settings_path, false)?;
    result.hooks_configured.push("SessionStart".to_string());
    result.hooks_configured.push("PreCompact".to_string());
    result.hooks_configured.push("PreToolUse (guard)".to_string());

    Ok(result)
}

/// Uninstall Blue skills and hooks from ~/.claude/
pub fn uninstall() -> Result<InstallResult, InstallError> {
    let claude_dir = claude_home()?;
    let skills_dir = claude_dir.join("skills");
    let settings_path = claude_dir.join("settings.json");

    let mut result = InstallResult {
        skills_installed: Vec::new(),
        hooks_configured: Vec::new(),
        skills_removed: Vec::new(),
        hooks_removed: Vec::new(),
    };

    // Remove Blue-managed skills (directories or symlinks starting with "blue-")
    if skills_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy().to_string();
                if name_str.starts_with("blue-") {
                    let path = entry.path();
                    if path.is_symlink() {
                        let _ = std::fs::remove_file(&path);
                        result.skills_removed.push(name_str);
                    } else if path.is_dir() {
                        let _ = std::fs::remove_dir_all(&path);
                        result.skills_removed.push(name_str);
                    }
                }
            }
        }
    }

    // Also remove legacy non-prefixed skill symlinks that point into a blue repo
    if skills_dir.exists() {
        let legacy_names = [
            "alignment-expert",
            "alignment-play",
            "domain-setup",
            "wt",
        ];
        for name in &legacy_names {
            let path = skills_dir.join(name);
            if path.symlink_metadata().is_ok() {
                let _ = std::fs::remove_file(&path);
                result.skills_removed.push(format!("{} (legacy)", name));
            }
        }
    }

    // Remove Blue hooks from settings.json
    configure_hooks(&settings_path, true)?;
    result.hooks_removed.push("SessionStart (blue)".to_string());
    result.hooks_removed.push("PreCompact (blue)".to_string());
    result.hooks_removed.push("PreToolUse guard (blue)".to_string());

    Ok(result)
}

/// Add or remove Blue hook entries in ~/.claude/settings.json.
///
/// When `remove` is false, adds blue hook entries (idempotently).
/// When `remove` is true, removes only entries whose command contains "blue hook".
/// All other hooks in the file are preserved.
fn configure_hooks(settings_path: &Path, remove: bool) -> Result<(), InstallError> {
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks.as_object_mut().unwrap();

    if remove {
        // Remove blue hook entries from every event type
        for (_event, entries) in hooks_obj.iter_mut() {
            if let Some(arr) = entries.as_array_mut() {
                arr.retain(|entry| !entry_contains_blue_hook(entry));
            }
        }
    } else {
        // Build the blue hook entry for each event
        let blue_hook_entry = |cmd: &str| -> serde_json::Value {
            serde_json::json!({
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": format!("blue hook {}", cmd)
                }]
            })
        };

        // SessionStart
        let session_start = hooks_obj
            .entry("SessionStart")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = session_start.as_array_mut() {
            // Remove existing blue hooks first (idempotent)
            arr.retain(|entry| !entry_contains_blue_hook(entry));
            arr.push(blue_hook_entry("session-start"));
        }

        // PreCompact
        let pre_compact = hooks_obj
            .entry("PreCompact")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = pre_compact.as_array_mut() {
            arr.retain(|entry| !entry_contains_blue_hook(entry));
            arr.push(blue_hook_entry("post-compact"));
        }

        // RFC 0076: PreToolUse guard (global, replaces per-project guard-write.sh)
        let pre_tool_use = hooks_obj
            .entry("PreToolUse")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = pre_tool_use.as_array_mut() {
            arr.retain(|entry| !entry_contains_blue_hook(entry));
            arr.push(serde_json::json!({
                "matcher": "Write|Edit|MultiEdit",
                "hooks": [{
                    "type": "command",
                    "command": "blue guard --stdin"
                }]
            }));
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write back with pretty formatting
    let content = serde_json::to_string_pretty(&settings)?;
    std::fs::write(settings_path, content)?;

    Ok(())
}

/// Check if a hook entry contains a blue-managed command ("blue hook" or "blue guard")
fn entry_contains_blue_hook(entry: &serde_json::Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains("blue hook") || c.contains("blue guard"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_contains_blue_hook_positive() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "blue hook session-start"
            }]
        });
        assert!(entry_contains_blue_hook(&entry));
    }

    #[test]
    fn test_entry_contains_blue_hook_negative() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "some-other-tool start"
            }]
        });
        assert!(!entry_contains_blue_hook(&entry));
    }

    #[test]
    fn test_configure_hooks_adds_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        configure_hooks(&settings_path, false).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 1);
        assert!(entry_contains_blue_hook(&session_start[0]));

        let pre_compact = settings["hooks"]["PreCompact"].as_array().unwrap();
        assert_eq!(pre_compact.len(), 1);
        assert!(entry_contains_blue_hook(&pre_compact[0]));
    }

    #[test]
    fn test_configure_hooks_preserves_existing() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // Write initial settings with an existing hook
        let initial = serde_json::json!({
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "",
                        "hooks": [{
                            "type": "command",
                            "command": "other-tool session-start"
                        }]
                    }
                ],
                "PreToolUse": [
                    {
                        "matcher": "Write",
                        "hooks": [{
                            "type": "command",
                            "command": "my-guard --check"
                        }]
                    }
                ]
            }
        });
        std::fs::write(&settings_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // Install
        configure_hooks(&settings_path, false).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // SessionStart should have both the existing and new hook
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 2);

        // PreToolUse should have the existing hook plus the blue guard hook (RFC 0076)
        let pre_tool_use = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool_use.len(), 2);
    }

    #[test]
    fn test_configure_hooks_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // Install twice
        configure_hooks(&settings_path, false).unwrap();
        configure_hooks(&settings_path, false).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Should still have exactly one blue hook per event
        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 1);
    }

    #[test]
    fn test_configure_hooks_remove() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // Install
        configure_hooks(&settings_path, false).unwrap();
        // Remove
        configure_hooks(&settings_path, true).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert!(session_start.is_empty());
    }

    #[test]
    fn test_configure_hooks_remove_preserves_others() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.json");

        // Write settings with both blue and non-blue hooks
        let initial = serde_json::json!({
            "hooks": {
                "SessionStart": [
                    {
                        "matcher": "",
                        "hooks": [{
                            "type": "command",
                            "command": "other-tool session-start"
                        }]
                    },
                    {
                        "matcher": "",
                        "hooks": [{
                            "type": "command",
                            "command": "blue hook session-start"
                        }]
                    }
                ]
            }
        });
        std::fs::write(&settings_path, serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // Remove blue hooks only
        configure_hooks(&settings_path, true).unwrap();

        let content = std::fs::read_to_string(&settings_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let session_start = settings["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 1);
        assert!(!entry_contains_blue_hook(&session_start[0]));
    }
}
