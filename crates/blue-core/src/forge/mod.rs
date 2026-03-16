//! Git Forge Integration (RFC 0013)
//!
//! Provides a unified interface for interacting with different git forges
//! (GitHub, Forgejo/Gitea) for PR operations.

mod forgejo;
mod git_url;
mod github;

pub use forgejo::ForgejoForge;
pub use git_url::{parse_git_url, GitUrl};
pub use github::GitHubForge;

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
    NotFound {
        owner: String,
        repo: String,
        number: u64,
    },
}

/// The Forge trait - unified interface for git forges
pub trait Forge: Send + Sync {
    /// Create a pull request
    fn create_pr(&self, opts: CreatePrOpts) -> Result<PullRequest, ForgeError>;

    /// Get a pull request by number
    fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest, ForgeError>;

    /// Merge a pull request
    fn merge_pr(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        strategy: MergeStrategy,
    ) -> Result<(), ForgeError>;

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
        ForgeType::GitHub => env::var("GITHUB_TOKEN")
            .or_else(|_| env::var("GH_TOKEN"))
            .map_err(|_| ForgeError::MissingToken {
                var: "GITHUB_TOKEN",
            }),
        ForgeType::Forgejo => env::var("FORGEJO_TOKEN")
            .or_else(|_| env::var("GITEA_TOKEN"))
            .map_err(|_| ForgeError::MissingToken {
                var: "FORGEJO_TOKEN",
            }),
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

/// AWS configuration (RFC 0034)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    /// AWS profile name from ~/.aws/config
    pub profile: String,
}

/// Release branch configuration (RFC 0034)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseConfig {
    /// Branch where active development happens
    #[serde(default = "default_develop")]
    pub develop_branch: String,
    /// Protected release branch
    #[serde(default = "default_main")]
    pub main_branch: String,
}

fn default_develop() -> String {
    "develop".to_string()
}

fn default_main() -> String {
    "main".to_string()
}

impl Default for ReleaseConfig {
    fn default() -> Self {
        Self {
            develop_branch: default_develop(),
            main_branch: default_main(),
        }
    }
}

/// Worktree initialization configuration (RFC 0034)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorktreeConfig {
    /// Additional environment variables for .env.isolated
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Unsupported config version {found}. Supported versions: {supported:?}")]
    UnsupportedVersion { found: u32, supported: Vec<u32> },

    #[error("Invalid forge type: {0}. Must be one of: github, forgejo, gitlab, bitbucket")]
    InvalidForgeType(String),

    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    #[error("Failed to read config: {0}")]
    ReadError(String),

    #[error("Failed to parse config: {0}")]
    ParseError(String),
}

/// Blue config file structure (RFC 0034)
///
/// Schema version 1 - single source of truth for repo-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueConfig {
    /// Schema version for migration support (required)
    #[serde(default = "default_version")]
    pub version: u32,

    /// Forge connection details (required)
    pub forge: ForgeConfig,

    /// AWS profile configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aws: Option<AwsConfig>,

    /// Release branch configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseConfig>,

    /// Worktree initialization configuration (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeConfig>,
}

fn default_version() -> u32 {
    1
}

impl BlueConfig {
    /// Validate the configuration schema
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Version check
        if self.version != 1 {
            return Err(ConfigError::UnsupportedVersion {
                found: self.version,
                supported: vec![1],
            });
        }

        // Forge type validation
        let valid_types = ["github", "forgejo", "gitlab", "bitbucket"];
        let forge_type_str = match self.forge.forge_type {
            ForgeType::GitHub => "github",
            ForgeType::Forgejo => "forgejo",
        };
        if !valid_types.contains(&forge_type_str) {
            return Err(ConfigError::InvalidForgeType(forge_type_str.to_string()));
        }

        Ok(())
    }

    /// Load config from .blue/config.yaml
    pub fn load(blue_dir: &std::path::Path) -> Result<Self, ConfigError> {
        let config_path = blue_dir.join("config.yaml");
        if !config_path.exists() {
            return Err(ConfigError::ReadError(format!(
                "Config file not found: {}",
                config_path.display()
            )));
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        let config: Self =
            serde_yaml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// Load config, returning None if not found (backward compatible)
    pub fn load_optional(blue_dir: &std::path::Path) -> Option<Self> {
        Self::load(blue_dir).ok()
    }

    /// Save config to .blue/config.yaml
    pub fn save(&self, blue_dir: &std::path::Path) -> Result<(), std::io::Error> {
        let config_path = blue_dir.join("config.yaml");
        let content = serde_yaml::to_string(self).map_err(std::io::Error::other)?;
        std::fs::write(&config_path, content)
    }

    /// Get the AWS profile if configured
    pub fn aws_profile(&self) -> Option<&str> {
        self.aws.as_ref().map(|a| a.profile.as_str())
    }

    /// Get the develop branch name (with default)
    pub fn develop_branch(&self) -> &str {
        self.release
            .as_ref()
            .map(|r| r.develop_branch.as_str())
            .unwrap_or("develop")
    }

    /// Get the main branch name (with default)
    pub fn main_branch(&self) -> &str {
        self.release
            .as_ref()
            .map(|r| r.main_branch.as_str())
            .unwrap_or("main")
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
        if let Ok(config) = BlueConfig::load(dir) {
            // Validate cached config matches current remote
            if config.forge.host == url.host
                && config.forge.owner == url.owner
                && config.forge.repo == url.repo
            {
                return config.forge.forge_type;
            }
        }
    }

    // Detect and cache
    let forge_type = detect_forge_type(remote_url);

    // Save to cache if blue_dir provided
    if let Some(dir) = blue_dir {
        let forge_config = ForgeConfig {
            forge_type,
            host: url.host.clone(),
            owner: url.owner.clone(),
            repo: url.repo.clone(),
        };

        let config = BlueConfig {
            version: 1,
            forge: forge_config,
            aws: None,
            release: None,
            worktree: None,
        };
        let _ = config.save(dir); // Ignore errors - caching is best-effort
    }

    forge_type
}

/// Create a forge instance with caching support
pub fn create_forge_cached(
    remote_url: &str,
    blue_dir: Option<&std::path::Path>,
) -> Result<Box<dyn Forge>, ForgeError> {
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
