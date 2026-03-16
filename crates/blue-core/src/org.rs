//! Org-mapped directory layout (RFC 0067)
//!
//! Global configuration for organizing repos by GitHub/Forgejo org
//! under a blue home directory.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrgError {
    #[error("Failed to read config: {0}")]
    ReadConfig(String),

    #[error("Failed to write config: {0}")]
    WriteConfig(String),

    #[error("Org not found: {0}")]
    OrgNotFound(String),

    #[error("Repo not found: {org}/{repo}")]
    RepoNotFound { org: String, repo: String },

    #[error("Clone failed: {0}")]
    CloneFailed(String),

    #[error("Git error: {0}")]
    Git(String),
}

/// Git hosting provider
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Github,
    Forgejo,
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::Github => write!(f, "github"),
            Provider::Forgejo => write!(f, "forgejo"),
        }
    }
}

/// A registered org
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    pub name: String,
    pub provider: Provider,
    /// Host for non-GitHub providers (e.g., git.beyondtheuniverse.superviber.com)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Override the default clone URL pattern
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
}

impl Org {
    pub fn github(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            provider: Provider::Github,
            host: None,
            url_pattern: None,
        }
    }

    pub fn forgejo(name: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            provider: Provider::Forgejo,
            host: Some(host.into()),
            url_pattern: None,
        }
    }

    /// Build the SSH clone URL for a repo
    pub fn clone_url(&self, repo: &str) -> String {
        if let Some(pattern) = &self.url_pattern {
            return pattern
                .replace("{org}", &self.name)
                .replace("{repo}", repo);
        }
        match self.provider {
            Provider::Github => format!("git@github.com:{}/{}.git", self.name, repo),
            Provider::Forgejo => {
                let host = self.host.as_deref().unwrap_or("localhost");
                format!("git@{}:{}/{}.git", host, self.name, repo)
            }
        }
    }
}

/// Blue global config stored at ~/.config/blue/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlueGlobalConfig {
    #[serde(default)]
    pub home: HomeConfig,
    #[serde(default)]
    pub orgs: Vec<Org>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeConfig {
    /// Root directory for org-mapped repos (default: ~/code)
    #[serde(default = "default_home_path")]
    pub path: String,
}

impl Default for HomeConfig {
    fn default() -> Self {
        Self {
            path: default_home_path(),
        }
    }
}

fn default_home_path() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("code")
        .display()
        .to_string()
}

impl BlueGlobalConfig {
    /// Load from ~/.config/blue/config.toml (returns default if not found)
    pub fn load() -> Self {
        let path = config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save to ~/.config/blue/config.toml
    pub fn save(&self) -> Result<(), OrgError> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| OrgError::WriteConfig(format!("mkdir: {}", e)))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| OrgError::WriteConfig(format!("serialize: {}", e)))?;
        std::fs::write(&path, &content)
            .map_err(|e| OrgError::WriteConfig(format!("write: {}", e)))?;
        Ok(())
    }

    /// Get the blue home directory
    pub fn home_path(&self) -> PathBuf {
        PathBuf::from(&self.home.path)
    }

    /// Find an org by name
    pub fn find_org(&self, name: &str) -> Option<&Org> {
        self.orgs.iter().find(|o| o.name == name)
    }

    /// Add an org (replaces if exists)
    pub fn add_org(&mut self, org: Org) {
        self.orgs.retain(|o| o.name != org.name);
        self.orgs.push(org);
    }

    /// Remove an org by name
    pub fn remove_org(&mut self, name: &str) -> bool {
        let before = self.orgs.len();
        self.orgs.retain(|o| o.name != name);
        self.orgs.len() < before
    }

    /// Resolve a repo path using the fallback chain:
    /// 1. {home}/{org}/{repo}/ (org layout)
    /// 2. {home}/{repo}/ (flat fallback)
    /// 3. None
    pub fn resolve_repo_path(&self, org: &str, repo: &str) -> Option<PathBuf> {
        let home = self.home_path();

        // Tier 1: org layout
        let org_path = home.join(org).join(repo);
        if org_path.exists() {
            return Some(org_path);
        }

        // Tier 2: flat fallback
        let flat_path = home.join(repo);
        if flat_path.exists() {
            return Some(flat_path);
        }

        None
    }

    /// Expected path for a repo in the org layout
    pub fn repo_path(&self, org: &str, repo: &str) -> PathBuf {
        self.home_path().join(org).join(repo)
    }

    /// Scan an org directory and list discovered repos
    pub fn scan_org(&self, org_name: &str) -> Vec<String> {
        let org_dir = self.home_path().join(org_name);
        let mut repos = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&org_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(".git").exists() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        repos.push(name.to_string());
                    }
                }
            }
        }
        repos.sort();
        repos
    }

    /// Scan all org directories and return org → repos mapping
    pub fn scan_all_orgs(&self) -> Vec<(String, Vec<String>)> {
        self.orgs
            .iter()
            .map(|org| {
                let repos = self.scan_org(&org.name);
                (org.name.clone(), repos)
            })
            .collect()
    }
}

/// Parse a git remote URL into (org, repo) tuple
///
/// Handles:
/// - git@github.com:org/repo.git
/// - https://github.com/org/repo.git
/// - git@custom.host:org/repo.git
pub fn parse_remote_url(url: &str) -> Option<(String, String)> {
    // SSH format: git@host:org/repo.git
    if let Some(path) = url.strip_prefix("git@").and_then(|s| s.split(':').nth(1)) {
        let path = path.trim_end_matches(".git");
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }

    // HTTPS format: https://host/org/repo.git
    if url.starts_with("https://") || url.starts_with("http://") {
        let path = url
            .split("://")
            .nth(1)
            .and_then(|s| s.splitn(2, '/').nth(1))?;
        let path = path.trim_end_matches(".git").trim_end_matches('/');
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }

    None
}

/// Detect org from a git repo's remote URL
pub fn detect_org_from_repo(repo_path: &Path) -> Option<(String, String, Provider)> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url = remote.url()?;

    let (org, repo_name) = parse_remote_url(url)?;

    let provider = if url.contains("github.com") {
        Provider::Github
    } else {
        Provider::Forgejo
    };

    Some((org, repo_name, provider))
}

/// Clone a repo into the org-mapped directory
pub fn clone_repo(config: &BlueGlobalConfig, url: &str) -> Result<PathBuf, OrgError> {
    let (org, repo_name) = parse_remote_url(url)
        .ok_or_else(|| OrgError::CloneFailed(format!("Could not parse org/repo from: {}", url)))?;

    let target = config.repo_path(&org, &repo_name);

    if target.exists() {
        return Err(OrgError::CloneFailed(format!(
            "Already exists: {}",
            target.display()
        )));
    }

    // Ensure org dir exists
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| OrgError::CloneFailed(format!("mkdir: {}", e)))?;
    }

    // Clone
    git2::Repository::clone(url, &target)
        .map_err(|e| OrgError::CloneFailed(format!("git clone: {}", e)))?;

    Ok(target)
}

/// Clone a repo by org + name (uses org registry for URL)
pub fn clone_repo_by_name(
    config: &BlueGlobalConfig,
    org_name: &str,
    repo_name: &str,
) -> Result<PathBuf, OrgError> {
    let org = config
        .find_org(org_name)
        .ok_or_else(|| OrgError::OrgNotFound(org_name.to_string()))?;
    let url = org.clone_url(repo_name);
    clone_repo(config, &url)
}

/// A proposed migration move
#[derive(Debug, Clone)]
pub struct MigrationMove {
    pub repo_dir_name: String,
    pub org: String,
    pub repo_name: String,
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Scan a directory for git repos and propose org-mapped moves
pub fn scan_for_migration(scan_dir: &Path, home: &Path) -> Vec<MigrationMove> {
    let mut moves = Vec::new();

    let entries = match std::fs::read_dir(scan_dir) {
        Ok(e) => e,
        Err(_) => return moves,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join(".git").exists() {
            continue;
        }

        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Try to detect org from git remote
        if let Some((org, repo_name, _provider)) = detect_org_from_repo(&path) {
            let target = home.join(&org).join(&repo_name);

            // Only propose if source != target
            if path != target {
                moves.push(MigrationMove {
                    repo_dir_name: dir_name,
                    org,
                    repo_name,
                    from: path,
                    to: target,
                });
            }
        }
    }

    moves.sort_by(|a, b| a.org.cmp(&b.org).then(a.repo_name.cmp(&b.repo_name)));
    moves
}

/// Execute a migration move (rename directory)
pub fn execute_move(mv: &MigrationMove) -> Result<(), OrgError> {
    // Ensure org directory exists
    if let Some(parent) = mv.to.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| OrgError::WriteConfig(format!("mkdir {}: {}", parent.display(), e)))?;
    }

    // Check target doesn't already exist
    if mv.to.exists() {
        return Err(OrgError::CloneFailed(format!(
            "Target already exists: {}",
            mv.to.display()
        )));
    }

    std::fs::rename(&mv.from, &mv.to).map_err(|e| {
        OrgError::WriteConfig(format!(
            "Move {} → {}: {}",
            mv.from.display(),
            mv.to.display(),
            e
        ))
    })?;

    Ok(())
}

/// Path to the global config file
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("blue")
        .join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_remote_url_ssh_github() {
        let (org, repo) = parse_remote_url("git@github.com:superviber/blue.git").unwrap();
        assert_eq!(org, "superviber");
        assert_eq!(repo, "blue");
    }

    #[test]
    fn test_parse_remote_url_https_github() {
        let (org, repo) =
            parse_remote_url("https://github.com/cultivarium/fungal-image-analysis.git").unwrap();
        assert_eq!(org, "cultivarium");
        assert_eq!(repo, "fungal-image-analysis");
    }

    #[test]
    fn test_parse_remote_url_ssh_forgejo() {
        let (org, repo) = parse_remote_url(
            "git@git.beyondtheuniverse.superviber.com:letemcook/aperture.git",
        )
        .unwrap();
        assert_eq!(org, "letemcook");
        assert_eq!(repo, "aperture");
    }

    #[test]
    fn test_org_clone_url_github() {
        let org = Org::github("superviber");
        assert_eq!(
            org.clone_url("blue"),
            "git@github.com:superviber/blue.git"
        );
    }

    #[test]
    fn test_org_clone_url_forgejo() {
        let org = Org::forgejo("letemcook", "git.beyondtheuniverse.superviber.com");
        assert_eq!(
            org.clone_url("aperture"),
            "git@git.beyondtheuniverse.superviber.com:letemcook/aperture.git"
        );
    }

    #[test]
    fn test_org_clone_url_custom_pattern() {
        let org = Org {
            name: "myorg".to_string(),
            provider: Provider::Github,
            host: None,
            url_pattern: Some("https://github.com/{org}/{repo}.git".to_string()),
        };
        assert_eq!(
            org.clone_url("myrepo"),
            "https://github.com/myorg/myrepo.git"
        );
    }

    #[test]
    fn test_config_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = BlueGlobalConfig {
            home: HomeConfig {
                path: "/tmp/test-code".to_string(),
            },
            orgs: vec![
                Org::github("superviber"),
                Org::forgejo("letemcook", "git.example.com"),
            ],
        };

        let content = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &content).unwrap();

        let loaded: BlueGlobalConfig =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.home.path, "/tmp/test-code");
        assert_eq!(loaded.orgs.len(), 2);
        assert_eq!(loaded.orgs[0].name, "superviber");
        assert_eq!(loaded.orgs[1].provider, Provider::Forgejo);
    }

    #[test]
    fn test_resolve_repo_path() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        // Create org/repo directory
        std::fs::create_dir_all(home.join("superviber").join("blue")).unwrap();

        let config = BlueGlobalConfig {
            home: HomeConfig {
                path: home.display().to_string(),
            },
            orgs: vec![Org::github("superviber")],
        };

        // Should find via org path
        let resolved = config.resolve_repo_path("superviber", "blue");
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("superviber/blue"));

        // Should not find missing repo
        assert!(config.resolve_repo_path("superviber", "nope").is_none());
    }

    #[test]
    fn test_resolve_repo_path_flat_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        // Create flat repo directory (no org subfolder)
        std::fs::create_dir_all(home.join("blue")).unwrap();

        let config = BlueGlobalConfig {
            home: HomeConfig {
                path: home.display().to_string(),
            },
            orgs: vec![Org::github("superviber")],
        };

        // Should fall back to flat layout
        let resolved = config.resolve_repo_path("superviber", "blue");
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("blue"));
    }

    #[test]
    fn test_scan_org() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        // Create org with two git repos and one non-git dir
        let org_dir = home.join("superviber");
        std::fs::create_dir_all(org_dir.join("blue").join(".git")).unwrap();
        std::fs::create_dir_all(org_dir.join("coherence").join(".git")).unwrap();
        std::fs::create_dir_all(org_dir.join("not-a-repo")).unwrap();

        let config = BlueGlobalConfig {
            home: HomeConfig {
                path: home.display().to_string(),
            },
            orgs: vec![Org::github("superviber")],
        };

        let repos = config.scan_org("superviber");
        assert_eq!(repos, vec!["blue", "coherence"]);
    }
}
