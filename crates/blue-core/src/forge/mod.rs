//! Git Forge Integration (RFC 0013)
//!
//! Provides a unified interface for interacting with different git forges
//! (GitHub, Forgejo/Gitea) for PR operations.

mod git_url;
mod github;
mod forgejo;

pub use git_url::{GitUrl, parse_git_url};
pub use github::GitHubForge;
pub use forgejo::ForgejoForge;

use serde::{Deserialize, Serialize};
use std::env;

/// Supported forge types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForgeType {
    GitHub,
    Forgejo,
}

impl std::fmt::Display for ForgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForgeType::GitHub => write!(f, "github"),
            ForgeType::Forgejo => write!(f, "forgejo"),
        }
    }
}

/// Options for creating a pull request
#[derive(Debug, Clone)]
pub struct CreatePrOpts {
    pub owner: String,
    pub repo: String,
    pub head: String,
    pub base: String,
    pub title: String,
    pub body: Option<String>,
    pub draft: bool,
}

/// Merge strategy for PRs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Merge,
    Squash,
    Rebase,
}

/// Pull request info returned from forge operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub state: PrState,
    pub merged: bool,
    pub head: String,
    pub base: String,
}

/// Pull request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Open,
    Closed,
}

/// Forge errors
#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Missing token: set {var} environment variable")]
    MissingToken { var: &'static str },

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("PR not found: {owner}/{repo}#{number}")]
    NotFound { owner: String, repo: String, number: u64 },
}

/// The Forge trait - unified interface for git forges
pub trait Forge: Send + Sync {
    /// Create a pull request
    fn create_pr(&self, opts: CreatePrOpts) -> Result<PullRequest, ForgeError>;

    /// Get a pull request by number
    fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest, ForgeError>;

    /// Merge a pull request
    fn merge_pr(&self, owner: &str, repo: &str, number: u64, strategy: MergeStrategy) -> Result<(), ForgeError>;

    /// Check if a PR is merged
    fn pr_is_merged(&self, owner: &str, repo: &str, number: u64) -> Result<bool, ForgeError>;

    /// Get the forge type
    fn forge_type(&self) -> ForgeType;
}

/// Detect forge type from a git remote URL
pub fn detect_forge_type(remote_url: &str) -> ForgeType {
    let url = parse_git_url(remote_url);

    match url.host.as_str() {
        "github.com" => ForgeType::GitHub,
        "codeberg.org" => ForgeType::Forgejo,
        host if host.contains("gitea") => ForgeType::Forgejo,
        host if host.contains("forgejo") => ForgeType::Forgejo,
        _ => {
            // For unknown hosts, try probing the Forgejo/Gitea API
            // If that fails, default to GitHub API format
            if probe_forgejo_api(&url.host) {
                ForgeType::Forgejo
            } else {
                ForgeType::GitHub
            }
        }
    }
}

/// Probe a host to see if it's running Forgejo/Gitea
fn probe_forgejo_api(host: &str) -> bool {
    // Try to hit the version endpoint - Forgejo/Gitea respond, GitHub doesn't
    let url = format!("https://{}/api/v1/version", host);

    // Use a blocking client with short timeout for probing
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client.get(&url).send() {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Get the appropriate token for a forge type
pub fn get_token(forge_type: ForgeType) -> Result<String, ForgeError> {
    match forge_type {
        ForgeType::GitHub => {
            env::var("GITHUB_TOKEN")
                .or_else(|_| env::var("GH_TOKEN"))
                .map_err(|_| ForgeError::MissingToken { var: "GITHUB_TOKEN" })
        }
        ForgeType::Forgejo => {
            env::var("FORGEJO_TOKEN")
                .or_else(|_| env::var("GITEA_TOKEN"))
                .map_err(|_| ForgeError::MissingToken { var: "FORGEJO_TOKEN" })
        }
    }
}

/// Create a forge instance for a given remote URL
pub fn create_forge(remote_url: &str) -> Result<Box<dyn Forge>, ForgeError> {
    let url = parse_git_url(remote_url);
    let forge_type = detect_forge_type(remote_url);
    let token = get_token(forge_type)?;

    match forge_type {
        ForgeType::GitHub => Ok(Box::new(GitHubForge::new(token))),
        ForgeType::Forgejo => Ok(Box::new(ForgejoForge::new(&url.host, token))),
    }
}

/// Forge configuration for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeConfig {
    #[serde(rename = "type")]
    pub forge_type: ForgeType,
    pub host: String,
    pub owner: String,
    pub repo: String,
}

/// Blue config file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlueConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge: Option<ForgeConfig>,
}

impl BlueConfig {
    /// Load config from .blue/config.yaml
    pub fn load(blue_dir: &std::path::Path) -> Option<Self> {
        let config_path = blue_dir.join("config.yaml");
        if !config_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&config_path).ok()?;
        serde_yaml::from_str(&content).ok()
    }

    /// Save config to .blue/config.yaml
    pub fn save(&self, blue_dir: &std::path::Path) -> Result<(), std::io::Error> {
        let config_path = blue_dir.join("config.yaml");
        let content = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&config_path, content)
    }
}

/// Detect forge type with caching support
///
/// If blue_dir is provided, will check for cached config first and save
/// detected type for future use.
pub fn detect_forge_type_cached(remote_url: &str, blue_dir: Option<&std::path::Path>) -> ForgeType {
    let url = parse_git_url(remote_url);

    // Check cache first
    if let Some(dir) = blue_dir {
        if let Some(config) = BlueConfig::load(dir) {
            if let Some(forge) = config.forge {
                // Validate cached config matches current remote
                if forge.host == url.host && forge.owner == url.owner && forge.repo == url.repo {
                    return forge.forge_type;
                }
            }
        }
    }

    // Detect and cache
    let forge_type = detect_forge_type(remote_url);

    // Save to cache if blue_dir provided
    if let Some(dir) = blue_dir {
        let forge_config = ForgeConfig {
            forge_type,
            host: url.host,
            owner: url.owner,
            repo: url.repo,
        };

        let mut config = BlueConfig::load(dir).unwrap_or_default();
        config.forge = Some(forge_config);
        let _ = config.save(dir); // Ignore errors - caching is best-effort
    }

    forge_type
}

/// Create a forge instance with caching support
pub fn create_forge_cached(remote_url: &str, blue_dir: Option<&std::path::Path>) -> Result<Box<dyn Forge>, ForgeError> {
    let url = parse_git_url(remote_url);
    let forge_type = detect_forge_type_cached(remote_url, blue_dir);
    let token = get_token(forge_type)?;

    match forge_type {
        ForgeType::GitHub => Ok(Box::new(GitHubForge::new(token))),
        ForgeType::Forgejo => Ok(Box::new(ForgejoForge::new(&url.host, token))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_github() {
        assert_eq!(
            detect_forge_type("git@github.com:owner/repo.git"),
            ForgeType::GitHub
        );
        assert_eq!(
            detect_forge_type("https://github.com/owner/repo.git"),
            ForgeType::GitHub
        );
    }

    #[test]
    fn test_detect_codeberg() {
        assert_eq!(
            detect_forge_type("git@codeberg.org:owner/repo.git"),
            ForgeType::Forgejo
        );
    }

    #[test]
    fn test_detect_gitea_in_host() {
        assert_eq!(
            detect_forge_type("git@gitea.example.com:owner/repo.git"),
            ForgeType::Forgejo
        );
    }
}
