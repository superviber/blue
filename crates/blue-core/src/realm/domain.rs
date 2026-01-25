//! Domain definitions for cross-repo coordination
//!
//! A domain is the coordination context between repos - the "edge" connecting nodes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::RealmError;

/// A domain is a coordination context between repos
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    /// Domain name (unique within realm)
    pub name: String,

    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// When the domain was created
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    /// Member repos in this domain
    #[serde(default)]
    pub members: Vec<String>,
}

impl Domain {
    /// Create a new domain
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            created_at: Utc::now(),
            members: Vec::new(),
        }
    }

    /// Add a member repo
    pub fn add_member(&mut self, repo: impl Into<String>) {
        let repo = repo.into();
        if !self.members.contains(&repo) {
            self.members.push(repo);
        }
    }

    /// Check if a repo is a member
    pub fn has_member(&self, repo: &str) -> bool {
        self.members.iter().any(|m| m == repo)
    }

    /// Load from a YAML file
    pub fn load(path: &Path) -> Result<Self, RealmError> {
        let content = std::fs::read_to_string(path).map_err(|e| RealmError::ReadFile {
            path: path.display().to_string(),
            source: e,
        })?;
        let domain: Self = serde_yaml::from_str(&content)?;
        Ok(domain)
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

/// A binding declares what a repo exports or imports in a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    /// Which repo this binding is for
    pub repo: String,

    /// Role in the domain
    pub role: BindingRole,

    /// What this repo exports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<ExportBinding>,

    /// What this repo imports
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<ImportBinding>,
}

impl Binding {
    /// Create a new provider binding
    pub fn provider(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            role: BindingRole::Provider,
            exports: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Create a new consumer binding
    pub fn consumer(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            role: BindingRole::Consumer,
            exports: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// Add an export
    pub fn add_export(&mut self, export: ExportBinding) {
        self.exports.push(export);
    }

    /// Add an import
    pub fn add_import(&mut self, import: ImportBinding) {
        self.imports.push(import);
    }

    /// Load from a YAML file
    pub fn load(path: &Path) -> Result<Self, RealmError> {
        let content = std::fs::read_to_string(path).map_err(|e| RealmError::ReadFile {
            path: path.display().to_string(),
            source: e,
        })?;
        let binding: Self = serde_yaml::from_str(&content)?;
        Ok(binding)
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

/// Role of a repo in a domain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindingRole {
    /// Provides/exports data
    Provider,

    /// Consumes/imports data
    Consumer,

    /// Both provides and consumes
    Both,
}

/// An export declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBinding {
    /// Which contract this exports
    pub contract: String,

    /// Source files that define the exported values
    #[serde(default)]
    pub source_files: Vec<String>,
}

impl ExportBinding {
    /// Create a new export binding
    pub fn new(contract: impl Into<String>) -> Self {
        Self {
            contract: contract.into(),
            source_files: Vec::new(),
        }
    }

    /// Add a source file
    pub fn with_source(mut self, path: impl Into<String>) -> Self {
        self.source_files.push(path.into());
        self
    }
}

/// An import declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBinding {
    /// Which contract this imports
    pub contract: String,

    /// Semver version requirement
    #[serde(default = "default_version_req")]
    pub version: String,

    /// File that binds to this contract
    #[serde(default)]
    pub binding: String,

    /// Current status of this import
    #[serde(default)]
    pub status: ImportStatus,

    /// Actually resolved version
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,

    /// When the version was resolved
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<DateTime<Utc>>,
}

fn default_version_req() -> String {
    ">=1.0.0".to_string()
}

impl ImportBinding {
    /// Create a new import binding
    pub fn new(contract: impl Into<String>) -> Self {
        Self {
            contract: contract.into(),
            version: default_version_req(),
            binding: String::new(),
            status: ImportStatus::Pending,
            resolved_version: None,
            resolved_at: None,
        }
    }

    /// Set the version requirement
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set the binding file
    pub fn with_binding(mut self, binding: impl Into<String>) -> Self {
        self.binding = binding.into();
        self
    }

    /// Resolve to a specific version
    pub fn resolve(&mut self, version: impl Into<String>) {
        self.resolved_version = Some(version.into());
        self.resolved_at = Some(Utc::now());
        self.status = ImportStatus::Current;
    }

    /// Check if this import satisfies a given version
    pub fn satisfies(&self, version: &str) -> Result<bool, RealmError> {
        let req = semver::VersionReq::parse(&self.version)
            .map_err(RealmError::InvalidVersion)?;
        let ver = semver::Version::parse(version)?;
        Ok(req.matches(&ver))
    }
}

/// Status of an import binding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    /// Not yet resolved
    #[default]
    Pending,

    /// Resolved and up to date
    Current,

    /// A newer version is available
    Outdated,

    /// The imported contract was removed
    Broken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_new() {
        let mut domain = Domain::new("s3-access");
        domain.add_member("aperture");
        domain.add_member("fungal");

        assert_eq!(domain.name, "s3-access");
        assert!(domain.has_member("aperture"));
        assert!(domain.has_member("fungal"));
        assert!(!domain.has_member("ml-infra"));
    }

    #[test]
    fn test_binding_provider() {
        let mut binding = Binding::provider("aperture");
        binding.add_export(
            ExportBinding::new("s3-permissions")
                .with_source("models/training/s3_paths.py"),
        );

        assert_eq!(binding.role, BindingRole::Provider);
        assert_eq!(binding.exports.len(), 1);
        assert_eq!(binding.exports[0].contract, "s3-permissions");
    }

    #[test]
    fn test_binding_consumer() {
        let mut binding = Binding::consumer("fungal");
        binding.add_import(
            ImportBinding::new("s3-permissions")
                .with_version(">=1.0.0, <2.0.0")
                .with_binding("cdk/training_tools_access_stack.py"),
        );

        assert_eq!(binding.role, BindingRole::Consumer);
        assert_eq!(binding.imports.len(), 1);
        assert_eq!(binding.imports[0].version, ">=1.0.0, <2.0.0");
    }

    #[test]
    fn test_import_satisfies() {
        // semver uses comma to separate version requirements
        let import = ImportBinding::new("test")
            .with_version(">=1.0.0, <2.0.0");

        assert!(import.satisfies("1.0.0").unwrap());
        assert!(import.satisfies("1.5.0").unwrap());
        assert!(!import.satisfies("2.0.0").unwrap());
        assert!(!import.satisfies("0.9.0").unwrap());
    }

    #[test]
    fn test_binding_yaml_roundtrip() {
        let mut binding = Binding::provider("aperture");
        binding.add_export(ExportBinding::new("s3-permissions"));

        let yaml = serde_yaml::to_string(&binding).unwrap();
        let parsed: Binding = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.repo, binding.repo);
        assert_eq!(parsed.exports.len(), 1);
    }
}
