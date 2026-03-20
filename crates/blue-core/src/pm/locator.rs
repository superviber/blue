//! PM repo locator — find the PM repo from any code repo via RFC 0067 org layout
//!
//! Resolution strategy:
//! 1. Detect current repo's org from git remote
//! 2. Scan sibling repos in the org directory for domain.yaml at root
//! 3. Return the PM repo path, available areas for current repo, and domain config

use std::path::{Path, PathBuf};

use crate::org::{detect_org_from_repo, BlueGlobalConfig};

use super::domain::{PmDomain, PmDomainError};

/// Result of locating the PM repo from a code repo
#[derive(Debug, Clone)]
pub struct PmRepoLocation {
    /// Path to the PM repo root
    pub pm_repo_path: PathBuf,
    /// Path to domain.yaml
    pub domain_yaml_path: PathBuf,
    /// Parsed domain config
    pub domain: PmDomain,
    /// Current repo's name (as detected from git remote)
    pub current_repo_name: String,
    /// Current repo's key (from domain.yaml repos), if registered (legacy/backward compat)
    pub current_repo_key: Option<String>,
    /// Areas that touch this repo (from domain.yaml areas[].repos)
    pub available_areas: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LocatorError {
    #[error("Could not detect org from git remote in {0}")]
    NoOrg(String),

    #[error("No PM repo found in org {org} (scanned {scanned} sibling repos)")]
    NoPmRepo { org: String, scanned: usize },

    #[error("Domain config error: {0}")]
    Domain(#[from] PmDomainError),

    #[error("Org config error: {0}")]
    Org(String),
}

/// Locate PM repo using org.yaml (RFC 0074).
/// Falls back to the existing sibling-scanning approach if no org.yaml found.
pub fn locate_pm_repo_from_org(code_repo_path: &Path) -> Result<PmRepoLocation, LocatorError> {
    // Try org.yaml first
    if let Some((org_root, manifest)) = crate::org::OrgManifest::find_in_ancestors(code_repo_path) {
        let pm_path = manifest.pm_repo_path(&org_root);
        let domain_yaml = pm_path.join("domain.yaml");
        if domain_yaml.exists() {
            let domain = PmDomain::load(&domain_yaml)?;

            // Detect current repo name
            let current_repo_name = crate::org::detect_org_from_repo(code_repo_path)
                .map(|(_, name, _)| name)
                .unwrap_or_else(|| {
                    code_repo_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string()
                });

            let current_repo_key = domain
                .find_repo(&current_repo_name)
                .and_then(|r| r.key.clone());

            let available_areas: Vec<String> = domain
                .areas_for_repo(&current_repo_name)
                .iter()
                .map(|a| a.key.clone())
                .collect();

            return Ok(PmRepoLocation {
                pm_repo_path: pm_path,
                domain_yaml_path: domain_yaml,
                domain,
                current_repo_name,
                current_repo_key,
                available_areas,
            });
        }
    }

    // Fall back to existing approach
    locate_pm_repo(code_repo_path)
}

/// Locate the PM repo from a code repo.
///
/// Scans sibling repos in the same org directory for a `domain.yaml` at root.
/// Uses the RFC 0067 org layout (`{home}/{org}/{repo}/`).
pub fn locate_pm_repo(code_repo_path: &Path) -> Result<PmRepoLocation, LocatorError> {
    let config = BlueGlobalConfig::load();
    locate_pm_repo_with_config(code_repo_path, &config)
}

/// Locate PM repo with an explicit config (testable).
pub fn locate_pm_repo_with_config(
    code_repo_path: &Path,
    config: &BlueGlobalConfig,
) -> Result<PmRepoLocation, LocatorError> {
    // Step 1: Detect org and repo name from git remote
    let (org_name, repo_name, _provider) =
        detect_org_from_repo(code_repo_path).ok_or_else(|| {
            LocatorError::NoOrg(code_repo_path.display().to_string())
        })?;

    // Step 2: Resolve org directory
    let home = config.home_path();
    let org_dir = home.join(&org_name);

    if !org_dir.exists() {
        return Err(LocatorError::NoPmRepo {
            org: org_name,
            scanned: 0,
        });
    }

    // Step 3: Scan sibling repos for domain.yaml
    let mut scanned = 0;
    let entries = std::fs::read_dir(&org_dir)
        .map_err(|e| LocatorError::Org(format!("read {}: {}", org_dir.display(), e)))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        scanned += 1;

        let domain_yaml = path.join("domain.yaml");
        if domain_yaml.exists() {
            let domain = PmDomain::load(&domain_yaml)?;

            // Verify this domain belongs to our org
            if domain.org == org_name || domain.repos.iter().any(|r| r.name == repo_name) {
                let current_repo_key = domain
                    .find_repo(&repo_name)
                    .and_then(|r| r.key.clone());

                // Populate available areas from domain.yaml areas[]
                let available_areas: Vec<String> = domain
                    .areas_for_repo(&repo_name)
                    .iter()
                    .map(|a| a.key.clone())
                    .collect();

                return Ok(PmRepoLocation {
                    pm_repo_path: path,
                    domain_yaml_path: domain_yaml,
                    domain,
                    current_repo_name: repo_name,
                    current_repo_key,
                    available_areas,
                });
            }
        }
    }

    // Step 4: Check if current repo IS the PM repo
    let self_domain_yaml = code_repo_path.join("domain.yaml");
    if self_domain_yaml.exists() {
        let domain = PmDomain::load(&self_domain_yaml)?;
        let current_repo_key = domain
            .find_repo(&repo_name)
            .and_then(|r| r.key.clone());

        let available_areas: Vec<String> = domain
            .areas_for_repo(&repo_name)
            .iter()
            .map(|a| a.key.clone())
            .collect();

        return Ok(PmRepoLocation {
            pm_repo_path: code_repo_path.to_path_buf(),
            domain_yaml_path: self_domain_yaml,
            domain,
            current_repo_name: repo_name,
            current_repo_key,
            available_areas,
        });
    }

    Err(LocatorError::NoPmRepo {
        org: org_name,
        scanned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests require git repos with remotes — tested via CLI e2e tests.
    // Unit tests cover the domain loading and scanning logic in domain.rs.

    #[test]
    fn test_locator_error_display() {
        let err = LocatorError::NoPmRepo {
            org: "myorg".to_string(),
            scanned: 5,
        };
        assert!(err.to_string().contains("myorg"));
        assert!(err.to_string().contains("5"));
    }
}
