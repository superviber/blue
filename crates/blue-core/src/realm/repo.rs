//! Repo registration for realms
//!
//! Defines how repos are registered in a realm.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::RealmError;

/// Configuration for a repo registered in a realm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Repo name (unique within realm)
    pub name: String,

    /// Optional organization prefix
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,

    /// Local filesystem path (for development)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Remote URL (for cloning)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Maintainers (email addresses)
    #[serde(default)]
    pub maintainers: Vec<String>,

    /// When the repo joined the realm
    #[serde(default = "Utc::now")]
    pub joined_at: DateTime<Utc>,
}

impl RepoConfig {
    /// Create a new repo config with a local path
    pub fn local(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            org: None,
            path: Some(path.into()),
            url: None,
            maintainers: Vec::new(),
            joined_at: Utc::now(),
        }
    }

    /// Create a new repo config with a remote URL
    pub fn remote(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            org: None,
            path: None,
            url: Some(url.into()),
            maintainers: Vec::new(),
            joined_at: Utc::now(),
        }
    }

    /// Set the organization
    pub fn with_org(mut self, org: impl Into<String>) -> Self {
        self.org = Some(org.into());
        self
    }

    /// Add a maintainer
    pub fn with_maintainer(mut self, email: impl Into<String>) -> Self {
        self.maintainers.push(email.into());
        self
    }

    /// Get the fully qualified name (org/name or just name)
    pub fn qualified_name(&self) -> String {
        match &self.org {
            Some(org) => format!("{}/{}", org, self.name),
            None => self.name.clone(),
        }
    }

    /// Check if a given email is a maintainer
    pub fn is_maintainer(&self, email: &str) -> bool {
        self.maintainers.iter().any(|m| m == email)
    }

    /// Load from a YAML file
    pub fn load(path: &Path) -> Result<Self, RealmError> {
        let content = std::fs::read_to_string(path).map_err(|e| RealmError::ReadFile {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: Self = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save to a YAML file
    pub fn save(&self, path: &Path) -> Result<(), RealmError> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content).map_err(|e| RealmError::WriteFile {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }
}

/// Local repo configuration stored in {repo}/.blue/config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRepoConfig {
    /// Realm membership
    pub realm: RealmRef,

    /// This repo's name in the realm
    pub repo: String,
}

impl LocalRepoConfig {
    /// Create a new local config
    pub fn new(
        realm_name: impl Into<String>,
        realm_url: impl Into<String>,
        repo: impl Into<String>,
    ) -> Self {
        Self {
            realm: RealmRef {
                name: realm_name.into(),
                url: realm_url.into(),
            },
            repo: repo.into(),
        }
    }

    /// Load from a YAML file
    pub fn load(path: &Path) -> Result<Self, RealmError> {
        let content = std::fs::read_to_string(path).map_err(|e| RealmError::ReadFile {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: Self = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Save to a YAML file
    pub fn save(&self, path: &Path) -> Result<(), RealmError> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content).map_err(|e| RealmError::WriteFile {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }
}

/// Reference to a realm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealmRef {
    /// Realm name
    pub name: String,

    /// Forgejo URL for the realm repo
    pub url: String,
}

/// RFC 0038: Local realm configuration stored in {repo}/.blue/realm.toml
///
/// This file defines cross-repo RFC dependencies and realm-specific settings.
/// Example:
/// ```toml
/// [realm]
/// name = "blue-ecosystem"
///
/// [rfc.0038]
/// depends_on = ["blue-web:0015", "blue-cli:0008"]
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalRealmDependencies {
    /// Realm membership (optional, for validation)
    #[serde(default)]
    pub realm: Option<LocalRealmMembership>,

    /// RFC dependencies by RFC number/slug
    /// Key: RFC identifier (e.g., "0038" or "sdlc-workflow-discipline")
    /// Value: Dependency configuration
    #[serde(default)]
    pub rfc: std::collections::HashMap<String, RfcDependencies>,
}

/// Realm membership in realm.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRealmMembership {
    /// Realm name
    pub name: String,
}

/// Dependencies for a single RFC
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RfcDependencies {
    /// Cross-repo dependencies
    /// Format: ["repo:rfc-id", "another-repo:rfc-id"]
    /// Example: ["blue-web:0015", "blue-cli:0008"]
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl LocalRealmDependencies {
    /// Create a new empty dependencies config
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with realm name
    pub fn with_realm(realm_name: impl Into<String>) -> Self {
        Self {
            realm: Some(LocalRealmMembership {
                name: realm_name.into(),
            }),
            rfc: std::collections::HashMap::new(),
        }
    }

    /// Load from a TOML file
    pub fn load(path: &Path) -> Result<Self, RealmError> {
        let content = std::fs::read_to_string(path).map_err(|e| RealmError::ReadFile {
            path: path.display().to_string(),
            source: e,
        })?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| RealmError::ValidationFailed(format!("Invalid TOML: {}", e)))?;
        Ok(config)
    }

    /// Save to a TOML file
    pub fn save(&self, path: &Path) -> Result<(), RealmError> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to serialize TOML: {}", e))
        })?;
        std::fs::write(path, content).map_err(|e| RealmError::WriteFile {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    /// Get dependencies for a specific RFC
    pub fn get_rfc_deps(&self, rfc_id: &str) -> Vec<String> {
        self.rfc
            .get(rfc_id)
            .map(|d| d.depends_on.clone())
            .unwrap_or_default()
    }

    /// Add dependencies for an RFC
    pub fn add_rfc_deps(&mut self, rfc_id: impl Into<String>, deps: Vec<String>) {
        self.rfc
            .insert(rfc_id.into(), RfcDependencies { depends_on: deps });
    }

    /// Check if the realm.toml exists at the given path
    pub fn exists(base_path: &Path) -> bool {
        base_path.join(".blue").join("realm.toml").exists()
    }

    /// Load from the standard location (.blue/realm.toml)
    pub fn load_from_blue(base_path: &Path) -> Result<Self, RealmError> {
        let path = base_path.join(".blue").join("realm.toml");
        Self::load(&path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_config_local() {
        let config = RepoConfig::local("aperture", "/Users/ericg/letemcook/aperture")
            .with_org("cultivarium")
            .with_maintainer("eric@example.com");

        assert_eq!(config.name, "aperture");
        assert_eq!(config.qualified_name(), "cultivarium/aperture");
        assert!(config.is_maintainer("eric@example.com"));
        assert!(!config.is_maintainer("other@example.com"));
    }

    #[test]
    fn test_repo_config_remote() {
        let config = RepoConfig::remote("aperture", "git@github.com:cultivarium/aperture.git");

        assert!(config.path.is_none());
        assert!(config.url.is_some());
    }

    #[test]
    fn test_repo_config_yaml_roundtrip() {
        let config = RepoConfig::local("aperture", "/path/to/aperture").with_org("cultivarium");

        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: RepoConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.name, config.name);
        assert_eq!(parsed.org, config.org);
    }

    #[test]
    fn test_local_repo_config() {
        let config = LocalRepoConfig::new(
            "letemcook",
            "https://git.example.com/realms/letemcook.git",
            "aperture",
        );

        assert_eq!(config.realm.name, "letemcook");
        assert_eq!(config.repo, "aperture");
    }
}
