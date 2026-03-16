//! domain.yaml schema — org key, repo registry, components, and areas
//!
//! Lives at the PM repo root. Single source for org identity,
//! Jira project mapping, repo-to-key assignments, components, and areas.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PmDomainError {
    #[error("domain.yaml not found at {0}")]
    NotFound(String),

    #[error("Failed to read domain.yaml: {0}")]
    Read(String),

    #[error("Failed to parse domain.yaml: {0}")]
    Parse(String),

    #[error("Failed to write domain.yaml: {0}")]
    Write(String),

    #[error("Repo not found in domain.yaml: {0}")]
    RepoNotFound(String),

    #[error("Duplicate repo key: {0}")]
    DuplicateKey(String),

    #[error("Duplicate repo name: {0}")]
    DuplicateName(String),

    #[error("Duplicate area key: {0}")]
    DuplicateAreaKey(String),

    #[error("Duplicate component name: {0}")]
    DuplicateComponentName(String),

    #[error("Area not found: {0}")]
    AreaNotFound(String),

    #[error("Invalid area key: {0} (must be 2-4 uppercase letters)")]
    InvalidAreaKey(String),

    #[error("Component reference not found: area {area} references unknown component {component}")]
    InvalidComponentRef { area: String, component: String },
}

/// Nested Jira configuration (preferred over flat fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraConfig {
    pub domain: String,
    pub project_key: String,
    #[serde(default = "default_drift_policy")]
    pub drift_policy: String,
}

/// A component in the domain (e.g., "auth", "payments")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
}

/// An area groups components and repos under a short key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Area {
    pub key: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<String>,
}

/// The PM repo's domain.yaml — org identity, repo registry, components, and areas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmDomain {
    /// Org name (e.g., "the-move-social")
    pub org: String,

    /// Org-wide key prefix for epics (e.g., "TMS")
    pub key: String,

    /// Jira Cloud domain (e.g., "themovesocial.atlassian.net") — flat field for backward compat
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// Jira project key (e.g., "SCRUM") — flat field for backward compat
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_key: Option<String>,

    /// Drift policy for sync (warn, block, overwrite)
    #[serde(default = "default_drift_policy")]
    pub drift_policy: String,

    /// Nested Jira configuration (preferred over flat fields)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jira: Option<JiraConfig>,

    /// Components defined in the domain
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<Component>,

    /// Areas that group components and repos
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub areas: Vec<Area>,

    /// Registered repos with their keys
    #[serde(default)]
    pub repos: Vec<RepoEntry>,
}

fn default_drift_policy() -> String {
    "warn".to_string()
}

/// A repo registered in the org's domain.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    /// Repo name (e.g., "themove-backend")
    pub name: String,

    /// Short key prefix for story IDs (e.g., "BKD") — now optional
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Git clone URL
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// What type of work belongs in this repo
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl PmDomain {
    /// Load from a domain.yaml file
    pub fn load(path: &Path) -> Result<Self, PmDomainError> {
        if !path.exists() {
            return Err(PmDomainError::NotFound(path.display().to_string()));
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| PmDomainError::Read(format!("{}: {}", path.display(), e)))?;
        let domain: Self =
            serde_yaml::from_str(&content).map_err(|e| PmDomainError::Parse(e.to_string()))?;
        domain.validate()?;
        Ok(domain)
    }

    /// Save to a domain.yaml file
    pub fn save(&self, path: &Path) -> Result<(), PmDomainError> {
        self.validate()?;
        let content =
            serde_yaml::to_string(self).map_err(|e| PmDomainError::Write(e.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PmDomainError::Write(format!("mkdir: {}", e)))?;
        }
        std::fs::write(path, content)
            .map_err(|e| PmDomainError::Write(format!("{}: {}", path.display(), e)))?;
        Ok(())
    }

    /// Validate: no duplicate keys/names, valid areas and components
    fn validate(&self) -> Result<(), PmDomainError> {
        // Validate repo keys and names
        let mut keys = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for repo in &self.repos {
            if let Some(ref k) = repo.key {
                if !keys.insert(k) {
                    return Err(PmDomainError::DuplicateKey(k.clone()));
                }
            }
            if !names.insert(&repo.name) {
                return Err(PmDomainError::DuplicateName(repo.name.clone()));
            }
        }

        // Validate component names unique
        let mut comp_names = std::collections::HashSet::new();
        for comp in &self.components {
            if !comp_names.insert(&comp.name) {
                return Err(PmDomainError::DuplicateComponentName(comp.name.clone()));
            }
        }

        // Validate area keys unique and well-formed (2-4 uppercase letters)
        let mut area_keys = std::collections::HashSet::new();
        let area_key_re = regex::Regex::new(r"^[A-Z]{2,4}$").unwrap();
        for area in &self.areas {
            if !area_key_re.is_match(&area.key) {
                return Err(PmDomainError::InvalidAreaKey(area.key.clone()));
            }
            if !area_keys.insert(&area.key) {
                return Err(PmDomainError::DuplicateAreaKey(area.key.clone()));
            }
            // Validate component references
            for comp_ref in &area.components {
                if !comp_names.contains(comp_ref) {
                    return Err(PmDomainError::InvalidComponentRef {
                        area: area.key.clone(),
                        component: comp_ref.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    /// Find repo entry by name
    pub fn find_repo(&self, name: &str) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.name == name)
    }

    /// Find repo entry by key
    pub fn find_repo_by_key(&self, key: &str) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.key.as_deref() == Some(key))
    }

    /// Add or update a repo entry. Returns true if added (vs updated).
    pub fn upsert_repo(&mut self, entry: RepoEntry) -> Result<bool, PmDomainError> {
        // Check key uniqueness against other repos
        if let Some(ref entry_key) = entry.key {
            if let Some(existing) = self
                .repos
                .iter()
                .find(|r| r.key.as_deref() == Some(entry_key.as_str()) && r.name != entry.name)
            {
                return Err(PmDomainError::DuplicateKey(format!(
                    "{} (already used by {})",
                    entry_key, existing.name
                )));
            }
        }

        if let Some(pos) = self.repos.iter().position(|r| r.name == entry.name) {
            self.repos[pos] = entry;
            Ok(false)
        } else {
            self.repos.push(entry);
            Ok(true)
        }
    }

    /// Find the domain.yaml file starting from a PM repo root
    pub fn find_in_repo(pm_repo_root: &Path) -> Option<PathBuf> {
        let path = pm_repo_root.join("domain.yaml");
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Get effective Jira domain (prefers jira.domain, falls back to self.domain)
    pub fn jira_domain(&self) -> Option<&str> {
        self.jira
            .as_ref()
            .map(|j| j.domain.as_str())
            .or(self.domain.as_deref())
    }

    /// Get effective Jira project key (prefers jira.project_key, falls back to self.project_key)
    pub fn jira_project_key(&self) -> Option<&str> {
        self.jira
            .as_ref()
            .map(|j| j.project_key.as_str())
            .or(self.project_key.as_deref())
    }

    /// Get effective drift policy (prefers jira.drift_policy, falls back to self.drift_policy)
    pub fn effective_drift_policy(&self) -> &str {
        self.jira
            .as_ref()
            .map(|j| j.drift_policy.as_str())
            .unwrap_or(&self.drift_policy)
    }

    /// Find area by key
    pub fn find_area(&self, key: &str) -> Option<&Area> {
        self.areas.iter().find(|a| a.key == key)
    }

    /// Find area by name
    pub fn find_area_by_name(&self, name: &str) -> Option<&Area> {
        self.areas.iter().find(|a| a.name == name)
    }

    /// Find areas that touch a given repo
    pub fn areas_for_repo(&self, repo_name: &str) -> Vec<&Area> {
        self.areas
            .iter()
            .filter(|a| a.repos.iter().any(|r| r == repo_name))
            .collect()
    }

    /// Find component by name
    pub fn find_component(&self, name: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_domain() -> PmDomain {
        PmDomain {
            org: "the-move-social".to_string(),
            key: "TMS".to_string(),
            domain: Some("themovesocial.atlassian.net".to_string()),
            project_key: Some("SCRUM".to_string()),
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![
                RepoEntry {
                    name: "themove-backend".to_string(),
                    key: Some("BKD".to_string()),
                    url: Some("git@github.com:the-move-social/themove-backend.git".to_string()),
                    description: Some("Backend API services".to_string()),
                },
                RepoEntry {
                    name: "themove-frontend".to_string(),
                    key: Some("FRD".to_string()),
                    url: Some("git@github.com:the-move-social/themove-frontend.git".to_string()),
                    description: Some("React web application".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_find_repo_by_name() {
        let domain = sample_domain();
        let repo = domain.find_repo("themove-backend").unwrap();
        assert_eq!(repo.key.as_deref(), Some("BKD"));
    }

    #[test]
    fn test_find_repo_by_key() {
        let domain = sample_domain();
        let repo = domain.find_repo_by_key("FRD").unwrap();
        assert_eq!(repo.name, "themove-frontend");
    }

    #[test]
    fn test_find_repo_missing() {
        let domain = sample_domain();
        assert!(domain.find_repo("nope").is_none());
        assert!(domain.find_repo_by_key("XXX").is_none());
    }

    #[test]
    fn test_upsert_repo_add() {
        let mut domain = sample_domain();
        let added = domain
            .upsert_repo(RepoEntry {
                name: "themove-product".to_string(),
                key: Some("PRD".to_string()),
                url: None,
                description: Some("Product specs".to_string()),
            })
            .unwrap();
        assert!(added);
        assert_eq!(domain.repos.len(), 3);
    }

    #[test]
    fn test_upsert_repo_update() {
        let mut domain = sample_domain();
        let added = domain
            .upsert_repo(RepoEntry {
                name: "themove-backend".to_string(),
                key: Some("BKD".to_string()),
                url: None,
                description: Some("Updated description".to_string()),
            })
            .unwrap();
        assert!(!added);
        assert_eq!(domain.repos.len(), 2);
        assert_eq!(
            domain
                .find_repo("themove-backend")
                .unwrap()
                .description
                .as_deref(),
            Some("Updated description")
        );
    }

    #[test]
    fn test_upsert_repo_duplicate_key() {
        let mut domain = sample_domain();
        let result = domain.upsert_repo(RepoEntry {
            name: "other-repo".to_string(),
            key: Some("BKD".to_string()), // already used by themove-backend
            url: None,
            description: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_duplicate_key() {
        let domain = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![
                RepoEntry {
                    name: "a".to_string(),
                    key: Some("DUP".to_string()),
                    url: None,
                    description: None,
                },
                RepoEntry {
                    name: "b".to_string(),
                    key: Some("DUP".to_string()),
                    url: None,
                    description: None,
                },
            ],
        };
        assert!(domain.validate().is_err());
    }

    #[test]
    fn test_yaml_roundtrip() {
        let domain = sample_domain();
        let yaml = serde_yaml::to_string(&domain).unwrap();
        let parsed: PmDomain = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.org, "the-move-social");
        assert_eq!(parsed.key, "TMS");
        assert_eq!(parsed.repos.len(), 2);
        assert_eq!(parsed.repos[0].key.as_deref(), Some("BKD"));
    }

    #[test]
    fn test_load_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("domain.yaml");

        let domain = sample_domain();
        domain.save(&path).unwrap();

        let loaded = PmDomain::load(&path).unwrap();
        assert_eq!(loaded.org, domain.org);
        assert_eq!(loaded.key, domain.key);
        assert_eq!(loaded.repos.len(), domain.repos.len());
    }

    #[test]
    fn test_find_in_repo() {
        let dir = tempfile::tempdir().unwrap();

        // No domain.yaml -> None
        assert!(PmDomain::find_in_repo(dir.path()).is_none());

        // Create domain.yaml -> Some
        let domain = sample_domain();
        domain.save(&dir.path().join("domain.yaml")).unwrap();
        assert!(PmDomain::find_in_repo(dir.path()).is_some());
    }

    // -----------------------------------------------------------------------
    // New tests for RFC 0070 Phase 1
    // -----------------------------------------------------------------------

    fn sample_domain_with_areas() -> PmDomain {
        PmDomain {
            org: "the-move-social".to_string(),
            key: "TMS".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: Some(JiraConfig {
                domain: "themovesocial.atlassian.net".to_string(),
                project_key: "SCRUM".to_string(),
                drift_policy: "block".to_string(),
            }),
            components: vec![
                Component {
                    name: "auth".to_string(),
                    description: Some("Authentication and authorization".to_string()),
                    lead: Some("alice".to_string()),
                },
                Component {
                    name: "payments".to_string(),
                    description: Some("Payment processing".to_string()),
                    lead: None,
                },
                Component {
                    name: "ui-kit".to_string(),
                    description: None,
                    lead: None,
                },
            ],
            areas: vec![
                Area {
                    key: "BE".to_string(),
                    name: "Backend".to_string(),
                    description: Some("Backend services".to_string()),
                    components: vec!["auth".to_string(), "payments".to_string()],
                    repos: vec!["themove-backend".to_string()],
                },
                Area {
                    key: "FE".to_string(),
                    name: "Frontend".to_string(),
                    description: None,
                    components: vec!["ui-kit".to_string()],
                    repos: vec![
                        "themove-frontend".to_string(),
                        "themove-backend".to_string(),
                    ],
                },
            ],
            repos: vec![
                RepoEntry {
                    name: "themove-backend".to_string(),
                    key: Some("BKD".to_string()),
                    url: None,
                    description: None,
                },
                RepoEntry {
                    name: "themove-frontend".to_string(),
                    key: Some("FRD".to_string()),
                    url: None,
                    description: None,
                },
            ],
        }
    }

    #[test]
    fn test_find_area_by_key() {
        let domain = sample_domain_with_areas();
        let area = domain.find_area("BE").unwrap();
        assert_eq!(area.name, "Backend");
        assert_eq!(area.components.len(), 2);
        assert!(domain.find_area("XX").is_none());
    }

    #[test]
    fn test_find_area_by_name() {
        let domain = sample_domain_with_areas();
        let area = domain.find_area_by_name("Frontend").unwrap();
        assert_eq!(area.key, "FE");
        assert!(domain.find_area_by_name("Missing").is_none());
    }

    #[test]
    fn test_areas_for_repo() {
        let domain = sample_domain_with_areas();
        // themove-backend is in both BE and FE areas
        let areas = domain.areas_for_repo("themove-backend");
        assert_eq!(areas.len(), 2);
        let keys: Vec<&str> = areas.iter().map(|a| a.key.as_str()).collect();
        assert!(keys.contains(&"BE"));
        assert!(keys.contains(&"FE"));

        // themove-frontend is only in FE
        let areas = domain.areas_for_repo("themove-frontend");
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].key, "FE");

        // unknown repo
        assert!(domain.areas_for_repo("nope").is_empty());
    }

    #[test]
    fn test_find_component() {
        let domain = sample_domain_with_areas();
        let comp = domain.find_component("auth").unwrap();
        assert_eq!(
            comp.description.as_deref(),
            Some("Authentication and authorization")
        );
        assert_eq!(comp.lead.as_deref(), Some("alice"));

        let comp2 = domain.find_component("ui-kit").unwrap();
        assert!(comp2.description.is_none());
        assert!(comp2.lead.is_none());

        assert!(domain.find_component("missing").is_none());
    }

    #[test]
    fn test_validate_duplicate_area_key() {
        let domain = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![
                Area {
                    key: "BE".to_string(),
                    name: "Backend".to_string(),
                    description: None,
                    components: vec![],
                    repos: vec![],
                },
                Area {
                    key: "BE".to_string(),
                    name: "Backend Dup".to_string(),
                    description: None,
                    components: vec![],
                    repos: vec![],
                },
            ],
            repos: vec![],
        };
        let err = domain.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate area key"));
    }

    #[test]
    fn test_validate_invalid_area_key() {
        // lowercase
        let mut domain = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![Area {
                key: "be".to_string(),
                name: "Backend".to_string(),
                description: None,
                components: vec![],
                repos: vec![],
            }],
            repos: vec![],
        };
        let err = domain.validate().unwrap_err();
        assert!(err.to_string().contains("Invalid area key"));

        // too short (1 char)
        domain.areas[0].key = "B".to_string();
        let err = domain.validate().unwrap_err();
        assert!(err.to_string().contains("Invalid area key"));

        // too long (5 chars)
        domain.areas[0].key = "BACKE".to_string();
        let err = domain.validate().unwrap_err();
        assert!(err.to_string().contains("Invalid area key"));

        // valid 2 chars
        domain.areas[0].key = "BE".to_string();
        assert!(domain.validate().is_ok());

        // valid 4 chars
        domain.areas[0].key = "BACK".to_string();
        assert!(domain.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_component_ref() {
        let domain = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![Component {
                name: "auth".to_string(),
                description: None,
                lead: None,
            }],
            areas: vec![Area {
                key: "BE".to_string(),
                name: "Backend".to_string(),
                description: None,
                components: vec!["auth".to_string(), "nonexistent".to_string()],
                repos: vec![],
            }],
            repos: vec![],
        };
        let err = domain.validate().unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Component reference not found"));
        assert!(msg.contains("nonexistent"));
        assert!(msg.contains("BE"));
    }

    #[test]
    fn test_validate_duplicate_component_name() {
        let domain = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![
                Component {
                    name: "auth".to_string(),
                    description: None,
                    lead: None,
                },
                Component {
                    name: "auth".to_string(),
                    description: Some("dup".to_string()),
                    lead: None,
                },
            ],
            areas: vec![],
            repos: vec![],
        };
        let err = domain.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate component name"));
    }

    #[test]
    fn test_jira_config_helpers() {
        // Old format: flat fields only
        let old_format = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: Some("old.atlassian.net".to_string()),
            project_key: Some("OLD".to_string()),
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };
        assert_eq!(old_format.jira_domain(), Some("old.atlassian.net"));
        assert_eq!(old_format.jira_project_key(), Some("OLD"));
        assert_eq!(old_format.effective_drift_policy(), "warn");

        // New format: jira sub-struct (should take precedence)
        let new_format = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: Some("old.atlassian.net".to_string()),
            project_key: Some("OLD".to_string()),
            drift_policy: "warn".to_string(),
            jira: Some(JiraConfig {
                domain: "new.atlassian.net".to_string(),
                project_key: "NEW".to_string(),
                drift_policy: "block".to_string(),
            }),
            components: vec![],
            areas: vec![],
            repos: vec![],
        };
        assert_eq!(new_format.jira_domain(), Some("new.atlassian.net"));
        assert_eq!(new_format.jira_project_key(), Some("NEW"));
        assert_eq!(new_format.effective_drift_policy(), "block");

        // No jira at all
        let no_jira = PmDomain {
            org: "test".to_string(),
            key: "TST".to_string(),
            domain: None,
            project_key: None,
            drift_policy: "warn".to_string(),
            jira: None,
            components: vec![],
            areas: vec![],
            repos: vec![],
        };
        assert_eq!(no_jira.jira_domain(), None);
        assert_eq!(no_jira.jira_project_key(), None);
        assert_eq!(no_jira.effective_drift_policy(), "warn");
    }

    #[test]
    fn test_yaml_roundtrip_new_schema() {
        let domain = sample_domain_with_areas();
        let yaml = serde_yaml::to_string(&domain).unwrap();
        let parsed: PmDomain = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.org, "the-move-social");
        assert_eq!(parsed.components.len(), 3);
        assert_eq!(parsed.areas.len(), 2);
        assert_eq!(parsed.repos.len(), 2);

        // Verify jira config roundtrips
        let jira = parsed.jira.as_ref().unwrap();
        assert_eq!(jira.domain, "themovesocial.atlassian.net");
        assert_eq!(jira.project_key, "SCRUM");
        assert_eq!(jira.drift_policy, "block");

        // Verify areas roundtrip
        let be = parsed.find_area("BE").unwrap();
        assert_eq!(be.components, vec!["auth", "payments"]);
        assert_eq!(be.repos, vec!["themove-backend"]);

        // Verify components roundtrip
        let auth = parsed.find_component("auth").unwrap();
        assert_eq!(auth.lead.as_deref(), Some("alice"));
    }

    #[test]
    fn test_backward_compat_old_schema() {
        // Old format YAML without components, areas, or jira sub-struct
        let yaml = r#"
org: the-move-social
key: TMS
domain: themovesocial.atlassian.net
project_key: SCRUM
drift_policy: warn
repos:
  - name: themove-backend
    key: BKD
    url: "git@github.com:the-move-social/themove-backend.git"
    description: Backend API services
  - name: themove-frontend
    key: FRD
"#;
        let parsed: PmDomain = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.org, "the-move-social");
        assert_eq!(parsed.key, "TMS");
        assert_eq!(parsed.domain.as_deref(), Some("themovesocial.atlassian.net"));
        assert_eq!(parsed.project_key.as_deref(), Some("SCRUM"));
        assert!(parsed.jira.is_none());
        assert!(parsed.components.is_empty());
        assert!(parsed.areas.is_empty());
        assert_eq!(parsed.repos.len(), 2);
        assert_eq!(parsed.repos[0].key.as_deref(), Some("BKD"));
        assert_eq!(parsed.repos[1].key.as_deref(), Some("FRD"));

        // Helpers fall back to flat fields
        assert_eq!(parsed.jira_domain(), Some("themovesocial.atlassian.net"));
        assert_eq!(parsed.jira_project_key(), Some("SCRUM"));
        assert_eq!(parsed.effective_drift_policy(), "warn");
    }
}
