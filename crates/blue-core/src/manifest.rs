//! Context manifest for Blue
//!
//! Defines the manifest schema for context injection configuration.
//! See RFC 0016 for the full specification.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::uri::{BlueUri, UriError};

/// Errors that can occur during manifest operations
#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("URI error: {0}")]
    Uri(#[from] UriError),

    #[error("Validation error: {0}")]
    Validation(String),
}

/// The main context manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManifest {
    /// Schema version
    #[serde(default = "default_version")]
    pub version: u32,

    /// When this manifest was generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<DateTime<Utc>>,

    /// Git commit hash when generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,

    /// Identity tier configuration (always injected)
    #[serde(default)]
    pub identity: IdentityConfig,

    /// Workflow tier configuration (activity-triggered)
    #[serde(default)]
    pub workflow: WorkflowConfig,

    /// Reference tier configuration (on-demand)
    #[serde(default)]
    pub reference: ReferenceConfig,

    /// Plugin configurations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<PluginConfig>,
}

fn default_version() -> u32 {
    1
}

/// Identity tier configuration (Tier 1)
///
/// "Who am I" - Always injected at session start.
/// Contains ADRs, voice patterns, core identity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityConfig {
    /// URIs to include in identity context
    #[serde(default)]
    pub sources: Vec<SourceConfig>,

    /// Maximum token budget for identity tier
    #[serde(default = "default_identity_tokens")]
    pub max_tokens: usize,
}

fn default_identity_tokens() -> usize {
    500
}

/// Workflow tier configuration (Tier 2)
///
/// "What should I do" - Triggered by activity.
/// Contains current RFC, active tasks, workflow state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// URIs to include in workflow context
    #[serde(default)]
    pub sources: Vec<SourceConfig>,

    /// Triggers that refresh workflow context
    #[serde(default)]
    pub refresh_triggers: Vec<RefreshTrigger>,

    /// Maximum token budget for workflow tier
    #[serde(default = "default_workflow_tokens")]
    pub max_tokens: usize,
}

fn default_workflow_tokens() -> usize {
    2000
}

/// Reference tier configuration (Tier 3)
///
/// "How does this work" - On-demand via MCP Resources.
/// Contains full documents, dialogues, historical context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceConfig {
    /// Relevance graph URI for computing context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<String>,

    /// Maximum token budget for reference tier
    #[serde(default = "default_reference_tokens")]
    pub max_tokens: usize,

    /// Days after which context is considered stale
    #[serde(default = "default_staleness_days")]
    pub staleness_days: u32,
}

fn default_reference_tokens() -> usize {
    4000
}

fn default_staleness_days() -> u32 {
    30
}

/// A source configuration within a tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// The URI to resolve
    pub uri: String,

    /// Optional label for this source
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Whether to allow external references
    #[serde(default)]
    pub allow_external: bool,
}

/// Refresh triggers for workflow context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RefreshTrigger {
    /// Refresh when the active RFC changes
    OnRfcChange,

    /// Refresh every N conversation turns
    #[serde(rename = "every_n_turns")]
    EveryNTurns(u32),

    /// Refresh on specific tool calls
    OnToolCall(String),
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin URI scheme
    pub uri: String,

    /// Context types this plugin provides
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provides: Vec<String>,

    /// Conditions that activate this plugin
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub salience_triggers: Vec<SalienceTrigger>,
}

/// Salience triggers for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalienceTrigger {
    /// Pattern to match in commit messages
    CommitMsgPattern(String),

    /// Annotation to look for in files
    FileAnnotation(String),

    /// Keyword to look for in conversation
    KeywordMatch(String),
}

impl ContextManifest {
    /// Load a manifest from a YAML file
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(path)?;
        let manifest: Self = serde_yaml::from_str(&content)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Load manifest from project root, using defaults if not present
    pub fn load_or_default(project_root: &Path) -> Result<Self, ManifestError> {
        let manifest_path = project_root.join(".blue").join("context.manifest.yaml");
        if manifest_path.exists() {
            Self::load(&manifest_path)
        } else {
            Ok(Self::default())
        }
    }

    /// Save the manifest to a YAML file
    pub fn save(&self, path: &Path) -> Result<(), ManifestError> {
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Validate the manifest
    pub fn validate(&self) -> Result<(), ManifestError> {
        // Validate version
        if self.version != 1 {
            return Err(ManifestError::Validation(format!(
                "Unsupported manifest version: {}",
                self.version
            )));
        }

        // Validate all URIs can be parsed
        for source in &self.identity.sources {
            BlueUri::parse(&source.uri)?;
        }
        for source in &self.workflow.sources {
            BlueUri::parse(&source.uri)?;
        }
        for plugin in &self.plugins {
            BlueUri::parse(&plugin.uri)?;
        }

        Ok(())
    }

    /// Get all source URIs from identity tier
    pub fn identity_uris(&self) -> Vec<&str> {
        self.identity
            .sources
            .iter()
            .map(|s| s.uri.as_str())
            .collect()
    }

    /// Get all source URIs from workflow tier
    pub fn workflow_uris(&self) -> Vec<&str> {
        self.workflow
            .sources
            .iter()
            .map(|s| s.uri.as_str())
            .collect()
    }

    /// Get total token budget
    pub fn total_budget(&self) -> usize {
        self.identity.max_tokens + self.workflow.max_tokens + self.reference.max_tokens
    }

    /// Create a summary string
    pub fn summary(&self) -> String {
        let identity_count = self.identity.sources.len();
        let workflow_count = self.workflow.sources.len();
        let plugin_count = self.plugins.len();

        format!(
            "Identity: {} sources ({} tokens) | Workflow: {} sources ({} tokens) | Plugins: {}",
            identity_count,
            self.identity.max_tokens,
            workflow_count,
            self.workflow.max_tokens,
            plugin_count
        )
    }
}

impl Default for ContextManifest {
    fn default() -> Self {
        Self {
            version: 1,
            generated_at: None,
            source_commit: None,
            identity: IdentityConfig {
                sources: vec![
                    SourceConfig {
                        uri: "blue://docs/adrs/".to_string(),
                        label: Some("Architecture Decision Records".to_string()),
                        allow_external: false,
                    },
                    SourceConfig {
                        uri: "blue://context/voice".to_string(),
                        label: Some("Voice patterns".to_string()),
                        allow_external: false,
                    },
                ],
                max_tokens: 500,
            },
            workflow: WorkflowConfig {
                sources: vec![SourceConfig {
                    uri: "blue://state/current-rfc".to_string(),
                    label: Some("Active RFC".to_string()),
                    allow_external: false,
                }],
                refresh_triggers: vec![RefreshTrigger::OnRfcChange],
                max_tokens: 2000,
            },
            reference: ReferenceConfig {
                graph: Some("blue://context/relevance".to_string()),
                max_tokens: 4000,
                staleness_days: 30,
            },
            plugins: Vec::new(),
        }
    }
}

/// Summary of resolved manifest content
#[derive(Debug, Clone, Default)]
pub struct ManifestResolution {
    /// Resolved identity tier
    pub identity: TierResolution,

    /// Resolved workflow tier
    pub workflow: TierResolution,

    /// Reference tier (not pre-resolved, on-demand)
    pub reference_budget: usize,
}

/// Resolution result for a single tier
#[derive(Debug, Clone, Default)]
pub struct TierResolution {
    /// Number of sources resolved
    pub source_count: usize,

    /// Estimated token count
    pub token_count: usize,

    /// List of resolved source details
    pub sources: Vec<ResolvedSource>,
}

/// A resolved source with metadata
#[derive(Debug, Clone)]
pub struct ResolvedSource {
    /// Original URI
    pub uri: String,

    /// Label if provided
    pub label: Option<String>,

    /// Number of files resolved
    pub file_count: usize,

    /// Estimated tokens
    pub tokens: usize,

    /// Content hash for change detection
    pub content_hash: String,
}

impl ContextManifest {
    /// Resolve the manifest against a project root
    pub fn resolve(&self, project_root: &Path) -> Result<ManifestResolution, ManifestError> {
        let identity = self.resolve_tier(&self.identity.sources, project_root)?;
        let workflow = self.resolve_tier(&self.workflow.sources, project_root)?;

        Ok(ManifestResolution {
            identity,
            workflow,
            reference_budget: self.reference.max_tokens,
        })
    }

    fn resolve_tier(
        &self,
        sources: &[SourceConfig],
        project_root: &Path,
    ) -> Result<TierResolution, ManifestError> {
        let mut resolution = TierResolution::default();

        for source in sources {
            let uri = BlueUri::parse(&source.uri)?;
            let paths = uri.resolve(project_root)?;

            let mut content = String::new();
            for path in &paths {
                if let Ok(text) = std::fs::read_to_string(path) {
                    content.push_str(&text);
                }
            }

            let tokens = crate::uri::estimate_tokens(&content);
            let hash = compute_content_hash(&content);

            resolution.sources.push(ResolvedSource {
                uri: source.uri.clone(),
                label: source.label.clone(),
                file_count: paths.len(),
                tokens,
                content_hash: hash,
            });

            resolution.source_count += 1;
            resolution.token_count += tokens;
        }

        Ok(resolution)
    }
}

/// Compute a simple hash of content for change detection
fn compute_content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_manifest() {
        let manifest = ContextManifest::default();
        assert_eq!(manifest.version, 1);
        assert!(!manifest.identity.sources.is_empty());
        assert_eq!(manifest.identity.max_tokens, 500);
    }

    #[test]
    fn test_manifest_summary() {
        let manifest = ContextManifest::default();
        let summary = manifest.summary();
        assert!(summary.contains("Identity:"));
        assert!(summary.contains("Workflow:"));
    }

    #[test]
    fn test_manifest_validation() {
        let manifest = ContextManifest::default();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_yaml_roundtrip() {
        let manifest = ContextManifest::default();
        let yaml = serde_yaml::to_string(&manifest).unwrap();
        let parsed: ContextManifest = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.version, manifest.version);
    }
}
