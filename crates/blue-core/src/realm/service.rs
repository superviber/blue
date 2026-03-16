//! Realm service for managing realms
//!
//! Handles creating, joining, and syncing realms using local git repos.

use git2::{Repository, Signature};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::info;

use super::{
    Binding, BindingRole, Contract, Domain, ImportStatus, LocalRepoConfig, RealmConfig, RealmError,
    RepoConfig,
};
use crate::daemon::{Realm, RealmStatus};

/// Cache entry for realm data
struct CacheEntry<T> {
    data: T,
    cached_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            cached_at: Instant::now(),
        }
    }

    fn is_valid(&self, ttl: Duration) -> bool {
        self.cached_at.elapsed() < ttl
    }
}

/// Cache for realm data
struct RealmCache {
    /// Cached realm configs
    configs: HashMap<String, CacheEntry<RealmConfig>>,
    /// Cached repo lists
    repos: HashMap<String, CacheEntry<Vec<RepoConfig>>>,
    /// Cached domain details
    domains: HashMap<String, CacheEntry<Vec<DomainDetails>>>,
    /// Time-to-live for cache entries
    ttl: Duration,
}

impl RealmCache {
    fn new(ttl: Duration) -> Self {
        Self {
            configs: HashMap::new(),
            repos: HashMap::new(),
            domains: HashMap::new(),
            ttl,
        }
    }

    /// Invalidate all cached data for a realm
    fn invalidate(&mut self, realm_name: &str) {
        self.configs.remove(realm_name);
        self.repos.remove(realm_name);
        self.domains.remove(realm_name);
    }

    /// Invalidate all cached data
    fn invalidate_all(&mut self) {
        self.configs.clear();
        self.repos.clear();
        self.domains.clear();
    }
}

/// Service for managing realms
pub struct RealmService {
    /// Base path for realm clones (~/.blue/realms/)
    realms_path: PathBuf,
    /// In-memory cache for realm data
    cache: Arc<RwLock<RealmCache>>,
}

/// Default cache TTL (5 minutes)
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);

impl RealmService {
    /// Create a new realm service
    pub fn new(realms_path: PathBuf) -> Self {
        Self::with_cache_ttl(realms_path, DEFAULT_CACHE_TTL)
    }

    /// Create a new realm service with custom cache TTL
    pub fn with_cache_ttl(realms_path: PathBuf, cache_ttl: Duration) -> Self {
        Self {
            realms_path,
            cache: Arc::new(RwLock::new(RealmCache::new(cache_ttl))),
        }
    }

    /// Invalidate the cache for a specific realm
    pub fn invalidate_cache(&self, realm_name: &str) {
        if let Ok(mut cache) = self.cache.write() {
            cache.invalidate(realm_name);
        }
    }

    /// Invalidate all cached data
    pub fn invalidate_all_caches(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.invalidate_all();
        }
    }

    /// Initialize a new realm
    ///
    /// Creates a local git repo with the realm structure.
    pub fn init_realm(&self, name: &str) -> Result<RealmInfo, RealmError> {
        let realm_path = self.realms_path.join(name);

        // Check if already exists
        if realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' already exists at {}",
                name,
                realm_path.display()
            )));
        }

        // Create directory structure
        std::fs::create_dir_all(&realm_path).map_err(|e| RealmError::WriteFile {
            path: realm_path.display().to_string(),
            source: e,
        })?;

        let repos_path = realm_path.join("repos");
        let domains_path = realm_path.join("domains");
        std::fs::create_dir_all(&repos_path).map_err(|e| RealmError::WriteFile {
            path: repos_path.display().to_string(),
            source: e,
        })?;
        std::fs::create_dir_all(&domains_path).map_err(|e| RealmError::WriteFile {
            path: domains_path.display().to_string(),
            source: e,
        })?;

        // Create realm.yaml
        let config = RealmConfig::new(name);
        let config_path = realm_path.join("realm.yaml");
        config.save(&config_path)?;

        // Initialize git repo
        let repo = Repository::init(&realm_path)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to init git repo: {}", e)))?;

        // Create initial commit
        self.create_initial_commit(&repo, name)?;

        // Invalidate any stale cache for this realm name
        self.invalidate_cache(name);

        info!(realm = %name, path = %realm_path.display(), "Realm created");

        Ok(RealmInfo {
            name: name.to_string(),
            path: realm_path,
            config,
        })
    }

    /// Join a repo to an existing realm
    ///
    /// Registers the repo in the realm and creates local config.
    pub fn join_realm(
        &self,
        realm_name: &str,
        repo_name: &str,
        repo_path: &Path,
    ) -> Result<(), RealmError> {
        let realm_path = self.realms_path.join(realm_name);

        // Verify realm exists
        if !realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' not found. Run 'blue realm admin init --name {}' first.",
                realm_name, realm_name
            )));
        }

        // Create repo registration in realm (with org detection per RFC 0067)
        let mut repo_config = RepoConfig::local(repo_name, repo_path.display().to_string());
        if let Some((org_name, _, _)) = crate::org::detect_org_from_repo(repo_path) {
            repo_config = repo_config.with_org(&org_name);
        }
        let repo_config_path = realm_path.join("repos").join(format!("{}.yaml", repo_name));
        repo_config.save(&repo_config_path)?;

        // Create local .blue directory and config
        let blue_dir = repo_path.join(".blue");
        std::fs::create_dir_all(&blue_dir).map_err(|e| RealmError::WriteFile {
            path: blue_dir.display().to_string(),
            source: e,
        })?;

        let local_config = LocalRepoConfig::new(
            realm_name,
            format!("file://{}", realm_path.display()), // Local file URL for now
            repo_name,
        );
        let local_config_path = blue_dir.join("config.yaml");
        local_config.save(&local_config_path)?;

        // Commit the repo registration to realm
        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;
        self.commit_changes(&repo, &format!("Add repo: {}", repo_name))?;

        // Invalidate cache - repos list changed
        self.invalidate_cache(realm_name);

        info!(
            realm = %realm_name,
            repo = %repo_name,
            "Repo joined realm"
        );

        Ok(())
    }

    /// Create a domain in a realm
    pub fn create_domain(
        &self,
        realm_name: &str,
        domain_name: &str,
        members: &[String],
    ) -> Result<(), RealmError> {
        let realm_path = self.realms_path.join(realm_name);

        // Verify realm exists
        if !realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' not found",
                realm_name
            )));
        }

        // Create domain directory
        let domain_path = realm_path.join("domains").join(domain_name);
        std::fs::create_dir_all(&domain_path).map_err(|e| RealmError::WriteFile {
            path: domain_path.display().to_string(),
            source: e,
        })?;

        // Create contracts and bindings directories
        std::fs::create_dir_all(domain_path.join("contracts")).map_err(|e| {
            RealmError::WriteFile {
                path: domain_path.join("contracts").display().to_string(),
                source: e,
            }
        })?;
        std::fs::create_dir_all(domain_path.join("bindings")).map_err(|e| {
            RealmError::WriteFile {
                path: domain_path.join("bindings").display().to_string(),
                source: e,
            }
        })?;

        // Create domain.yaml
        let mut domain = Domain::new(domain_name);
        for member in members {
            domain.add_member(member);
        }
        domain.save(&domain_path.join("domain.yaml"))?;

        // Commit changes
        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;
        self.commit_changes(&repo, &format!("Add domain: {}", domain_name))?;

        // Invalidate cache - domains list changed
        self.invalidate_cache(realm_name);

        info!(
            realm = %realm_name,
            domain = %domain_name,
            members = ?members,
            "Domain created"
        );

        Ok(())
    }

    /// Create a contract in a domain
    pub fn create_contract(
        &self,
        realm_name: &str,
        domain_name: &str,
        contract_name: &str,
        owner: &str,
    ) -> Result<(), RealmError> {
        let realm_path = self.realms_path.join(realm_name);
        let domain_path = realm_path.join("domains").join(domain_name);

        // Verify domain exists
        if !domain_path.exists() {
            return Err(RealmError::DomainNotFound(domain_name.to_string()));
        }

        // Create contract
        let contract = Contract::new(contract_name, owner);
        let contract_path = domain_path
            .join("contracts")
            .join(format!("{}.yaml", contract_name));
        contract.save(&contract_path)?;

        // Commit changes
        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;
        self.commit_changes(
            &repo,
            &format!("Add contract: {}/{}", domain_name, contract_name),
        )?;

        // Invalidate cache - domain contracts changed
        self.invalidate_cache(realm_name);

        info!(
            realm = %realm_name,
            domain = %domain_name,
            contract = %contract_name,
            owner = %owner,
            "Contract created"
        );

        Ok(())
    }

    /// Create a binding for a repo in a domain
    pub fn create_binding(
        &self,
        realm_name: &str,
        domain_name: &str,
        repo_name: &str,
        role: BindingRole,
    ) -> Result<(), RealmError> {
        let realm_path = self.realms_path.join(realm_name);
        let domain_path = realm_path.join("domains").join(domain_name);

        // Verify domain exists
        if !domain_path.exists() {
            return Err(RealmError::DomainNotFound(domain_name.to_string()));
        }

        // Create binding
        let binding = match role {
            BindingRole::Provider => Binding::provider(repo_name),
            BindingRole::Consumer => Binding::consumer(repo_name),
            BindingRole::Both => {
                let mut b = Binding::provider(repo_name);
                b.role = BindingRole::Both;
                b
            }
        };
        let binding_path = domain_path
            .join("bindings")
            .join(format!("{}.yaml", repo_name));
        binding.save(&binding_path)?;

        // Commit changes
        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;
        self.commit_changes(
            &repo,
            &format!("Add binding: {}/{}", domain_name, repo_name),
        )?;

        // Invalidate cache - domain bindings changed
        self.invalidate_cache(realm_name);

        info!(
            realm = %realm_name,
            domain = %domain_name,
            repo = %repo_name,
            role = ?role,
            "Binding created"
        );

        Ok(())
    }

    /// Load realm config from path (cached)
    pub fn load_realm(&self, name: &str) -> Result<RealmInfo, RealmError> {
        let realm_path = self.realms_path.join(name);

        if !realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' not found",
                name
            )));
        }

        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.configs.get(name) {
                if entry.is_valid(cache.ttl) {
                    return Ok(RealmInfo {
                        name: name.to_string(),
                        path: realm_path.clone(),
                        config: entry.data.clone(),
                    });
                }
            }
        }

        // Load from disk
        let config = RealmConfig::load(&realm_path.join("realm.yaml"))?;

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache
                .configs
                .insert(name.to_string(), CacheEntry::new(config.clone()));
        }

        Ok(RealmInfo {
            name: name.to_string(),
            path: realm_path,
            config,
        })
    }

    /// List all realms
    pub fn list_realms(&self) -> Result<Vec<String>, RealmError> {
        if !self.realms_path.exists() {
            return Ok(Vec::new());
        }

        let mut realms = Vec::new();
        let entries = std::fs::read_dir(&self.realms_path).map_err(|e| RealmError::ReadFile {
            path: self.realms_path.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| RealmError::ReadFile {
                path: self.realms_path.display().to_string(),
                source: e,
            })?;

            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Check if it's a valid realm (has realm.yaml)
                    if entry.path().join("realm.yaml").exists() {
                        realms.push(name.to_string());
                    }
                }
            }
        }

        Ok(realms)
    }

    /// Load complete realm details including domains, contracts, and bindings
    pub fn load_realm_details(&self, name: &str) -> Result<RealmDetails, RealmError> {
        let info = self.load_realm(name)?;

        // Load repos
        let repos = self.load_repos(name)?;

        // Load domains with their contracts and bindings
        let domains = self.load_domains(name)?;

        Ok(RealmDetails {
            info,
            repos,
            domains,
        })
    }

    /// Load all registered repos in a realm (cached)
    pub fn load_repos(&self, realm_name: &str) -> Result<Vec<RepoConfig>, RealmError> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.repos.get(realm_name) {
                if entry.is_valid(cache.ttl) {
                    return Ok(entry.data.clone());
                }
            }
        }

        let repos_path = self.realms_path.join(realm_name).join("repos");
        if !repos_path.exists() {
            return Ok(Vec::new());
        }

        let mut repos = Vec::new();
        let entries = std::fs::read_dir(&repos_path).map_err(|e| RealmError::ReadFile {
            path: repos_path.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| RealmError::ReadFile {
                path: repos_path.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                let repo = RepoConfig::load(&path)?;
                repos.push(repo);
            }
        }

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache
                .repos
                .insert(realm_name.to_string(), CacheEntry::new(repos.clone()));
        }

        Ok(repos)
    }

    /// Load all domains in a realm with their contracts and bindings (cached)
    pub fn load_domains(&self, realm_name: &str) -> Result<Vec<DomainDetails>, RealmError> {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(entry) = cache.domains.get(realm_name) {
                if entry.is_valid(cache.ttl) {
                    return Ok(entry.data.clone());
                }
            }
        }

        let domains_path = self.realms_path.join(realm_name).join("domains");
        if !domains_path.exists() {
            return Ok(Vec::new());
        }

        let mut domains = Vec::new();
        let entries = std::fs::read_dir(&domains_path).map_err(|e| RealmError::ReadFile {
            path: domains_path.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| RealmError::ReadFile {
                path: domains_path.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.is_dir() {
                let domain_yaml = path.join("domain.yaml");
                if domain_yaml.exists() {
                    let domain = Domain::load(&domain_yaml)?;
                    let contracts = self.load_contracts(&path)?;
                    let bindings = self.load_bindings(&path)?;
                    domains.push(DomainDetails {
                        domain,
                        contracts,
                        bindings,
                    });
                }
            }
        }

        // Update cache
        if let Ok(mut cache) = self.cache.write() {
            cache
                .domains
                .insert(realm_name.to_string(), CacheEntry::new(domains.clone()));
        }

        Ok(domains)
    }

    /// Load contracts in a domain
    fn load_contracts(&self, domain_path: &Path) -> Result<Vec<Contract>, RealmError> {
        let contracts_path = domain_path.join("contracts");
        if !contracts_path.exists() {
            return Ok(Vec::new());
        }

        let mut contracts = Vec::new();
        let entries = std::fs::read_dir(&contracts_path).map_err(|e| RealmError::ReadFile {
            path: contracts_path.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| RealmError::ReadFile {
                path: contracts_path.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                let contract = Contract::load(&path)?;
                contracts.push(contract);
            }
        }

        Ok(contracts)
    }

    /// Load bindings in a domain
    fn load_bindings(&self, domain_path: &Path) -> Result<Vec<Binding>, RealmError> {
        let bindings_path = domain_path.join("bindings");
        if !bindings_path.exists() {
            return Ok(Vec::new());
        }

        let mut bindings = Vec::new();
        let entries = std::fs::read_dir(&bindings_path).map_err(|e| RealmError::ReadFile {
            path: bindings_path.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| RealmError::ReadFile {
                path: bindings_path.display().to_string(),
                source: e,
            })?;

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                let binding = Binding::load(&path)?;
                bindings.push(binding);
            }
        }

        Ok(bindings)
    }

    /// Sync a realm - commit pending changes and return status
    pub fn sync_realm(&self, name: &str, _force: bool) -> Result<SyncResult, RealmError> {
        let realm_path = self.realms_path.join(name);

        if !realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' not found",
                name
            )));
        }

        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;

        // Check for uncommitted changes
        let statuses = repo.statuses(None).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to get git status: {}", e))
        })?;

        let has_changes = statuses.iter().any(|s| {
            s.status().intersects(
                git2::Status::INDEX_NEW
                    | git2::Status::INDEX_MODIFIED
                    | git2::Status::INDEX_DELETED
                    | git2::Status::WT_NEW
                    | git2::Status::WT_MODIFIED
                    | git2::Status::WT_DELETED,
            )
        });

        if has_changes {
            // Commit changes
            self.commit_changes(&repo, "Sync: auto-commit pending changes")?;
            // Invalidate cache after sync
            self.invalidate_cache(name);
            info!(realm = %name, "Committed pending changes");
        }

        // Get latest commit info
        let head = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let last_commit = head.as_ref().map(|c| {
            format!(
                "{}: {}",
                &c.id().to_string()[..7],
                c.summary().unwrap_or("(no message)")
            )
        });

        Ok(SyncResult {
            realm: name.to_string(),
            changes_committed: has_changes,
            last_commit,
            message: if has_changes {
                "Changes committed".to_string()
            } else {
                "Already up to date".to_string()
            },
        })
    }

    /// Check a realm for validation issues
    pub fn check_realm(&self, name: &str) -> Result<CheckResult, RealmError> {
        let details = self.load_realm_details(name)?;
        let mut result = CheckResult {
            realm: name.to_string(),
            ..Default::default()
        };

        for domain_detail in &details.domains {
            let domain_name = &domain_detail.domain.name;

            // Validate each contract
            for contract in &domain_detail.contracts {
                if let Err(e) = contract.validate() {
                    result.errors.push(CheckIssue {
                        domain: domain_name.clone(),
                        kind: CheckIssueKind::ContractInvalid,
                        message: format!("Contract '{}' invalid: {}", contract.name, e),
                    });
                }
            }

            // Check each binding
            for binding in &domain_detail.bindings {
                // Check exports reference valid contracts
                for export in &binding.exports {
                    let contract_exists = domain_detail
                        .contracts
                        .iter()
                        .any(|c| c.name == export.contract);
                    if !contract_exists {
                        result.errors.push(CheckIssue {
                            domain: domain_name.clone(),
                            kind: CheckIssueKind::BindingBroken,
                            message: format!(
                                "Binding '{}' exports contract '{}' which doesn't exist",
                                binding.repo, export.contract
                            ),
                        });
                    }
                }

                // Check imports are satisfied
                for import in &binding.imports {
                    let contract = domain_detail
                        .contracts
                        .iter()
                        .find(|c| c.name == import.contract);

                    match contract {
                        None => {
                            result.errors.push(CheckIssue {
                                domain: domain_name.clone(),
                                kind: CheckIssueKind::BindingBroken,
                                message: format!(
                                    "Binding '{}' imports contract '{}' which doesn't exist",
                                    binding.repo, import.contract
                                ),
                            });
                        }
                        Some(c) => {
                            // Check version compatibility
                            match import.satisfies(&c.version) {
                                Ok(true) => {}
                                Ok(false) => {
                                    result.errors.push(CheckIssue {
                                        domain: domain_name.clone(),
                                        kind: CheckIssueKind::VersionMismatch,
                                        message: format!(
                                            "Binding '{}' requires '{}' but contract is at v{}",
                                            binding.repo, import.version, c.version
                                        ),
                                    });
                                }
                                Err(e) => {
                                    result.warnings.push(CheckIssue {
                                        domain: domain_name.clone(),
                                        kind: CheckIssueKind::VersionMismatch,
                                        message: format!(
                                            "Binding '{}' has invalid version requirement '{}': {}",
                                            binding.repo, import.version, e
                                        ),
                                    });
                                }
                            }
                        }
                    }

                    // Check for broken imports
                    if import.status == ImportStatus::Broken {
                        result.errors.push(CheckIssue {
                            domain: domain_name.clone(),
                            kind: CheckIssueKind::ImportUnsatisfied,
                            message: format!(
                                "Binding '{}' has broken import for '{}'",
                                binding.repo, import.contract
                            ),
                        });
                    }
                }
            }
        }

        Ok(result)
    }

    /// Get the sync status of a realm without making changes
    pub fn realm_sync_status(&self, name: &str) -> Result<RealmSyncStatus, RealmError> {
        let realm_path = self.realms_path.join(name);

        if !realm_path.exists() {
            return Err(RealmError::ValidationFailed(format!(
                "Realm '{}' not found",
                name
            )));
        }

        let repo = Repository::open(&realm_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open realm repo: {}", e))
        })?;

        // Check for uncommitted changes
        let statuses = repo.statuses(None).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to get git status: {}", e))
        })?;

        let mut modified_files = Vec::new();
        let mut new_files = Vec::new();
        let mut deleted_files = Vec::new();

        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("unknown").to_string();
            let status = entry.status();

            if status.intersects(git2::Status::INDEX_NEW | git2::Status::WT_NEW) {
                new_files.push(path);
            } else if status.intersects(git2::Status::INDEX_MODIFIED | git2::Status::WT_MODIFIED) {
                modified_files.push(path);
            } else if status.intersects(git2::Status::INDEX_DELETED | git2::Status::WT_DELETED) {
                deleted_files.push(path);
            }
        }

        // Get head commit
        let head = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let head_commit = head.map(|c| c.id().to_string());

        Ok(RealmSyncStatus {
            realm: name.to_string(),
            head_commit,
            modified_files,
            new_files,
            deleted_files,
        })
    }

    // ─── Repo Path Resolution (RFC 0067) ──────────────────────────────────

    /// Resolve a repo's local path using RFC 0067 fallback chain:
    /// 1. RepoConfig.path (absolute, existing behavior)
    /// 2. Global config org resolution: {home}/{org}/{repo}
    /// 3. Global config flat fallback: {home}/{repo}
    pub fn resolve_repo_path(&self, repo: &RepoConfig) -> Option<std::path::PathBuf> {
        // Tier 1: explicit absolute path from RepoConfig
        if let Some(path_str) = &repo.path {
            let path = std::path::PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }

        // Tier 2+3: org-relative via global config
        if let Some(org) = &repo.org {
            let config = crate::org::BlueGlobalConfig::load();
            if let Some(path) = config.resolve_repo_path(org, &repo.name) {
                return Some(path);
            }
        }

        None
    }

    // ─── Worktree Management ────────────────────────────────────────────────

    /// Create a worktree for a repo at a given RFC branch
    ///
    /// Creates a new branch and worktree at `~/.blue/worktrees/{realm}/{rfc}/{repo}`
    pub fn create_worktree(
        &self,
        realm_name: &str,
        repo_name: &str,
        rfc_name: &str,
        repo_path: &Path,
    ) -> Result<WorktreeInfo, RealmError> {
        // Open the repo
        let repo = Repository::open(repo_path).map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to open repo '{}': {}", repo_name, e))
        })?;

        // Branch name from RFC
        let branch_name = rfc_name.to_string();

        // Worktree path
        let worktree_base = self.realms_path.parent().unwrap_or(&self.realms_path);
        let worktree_path = worktree_base
            .join("worktrees")
            .join(realm_name)
            .join(rfc_name)
            .join(repo_name);

        // Check if worktree already exists
        if worktree_path.exists() {
            return Ok(WorktreeInfo {
                repo: repo_name.to_string(),
                rfc: rfc_name.to_string(),
                path: worktree_path,
                branch: branch_name,
                already_existed: true,
            });
        }

        // Create parent directories
        std::fs::create_dir_all(worktree_path.parent().unwrap()).map_err(|e| {
            RealmError::WriteFile {
                path: worktree_path.display().to_string(),
                source: e,
            }
        })?;

        // Get HEAD commit to base the branch on
        let head = repo
            .head()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to get HEAD: {}", e)))?;
        let commit = head
            .peel_to_commit()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to get commit: {}", e)))?;

        // Create branch if it doesn't exist
        let branch = match repo.find_branch(&branch_name, git2::BranchType::Local) {
            Ok(branch) => branch,
            Err(_) => repo.branch(&branch_name, &commit, false).map_err(|e| {
                RealmError::ValidationFailed(format!("Failed to create branch: {}", e))
            })?,
        };

        // Create worktree
        let branch_ref = branch.into_reference();
        let branch_ref_name = branch_ref
            .name()
            .ok_or_else(|| RealmError::ValidationFailed("Branch has invalid name".to_string()))?;

        repo.worktree(
            &format!("{}-{}", rfc_name, repo_name),
            &worktree_path,
            Some(git2::WorktreeAddOptions::new().reference(Some(
                &repo.find_reference(branch_ref_name).map_err(|e| {
                    RealmError::ValidationFailed(format!("Failed to find branch ref: {}", e))
                })?,
            ))),
        )
        .map_err(|e| RealmError::ValidationFailed(format!("Failed to create worktree: {}", e)))?;

        info!(
            repo = %repo_name,
            rfc = %rfc_name,
            path = %worktree_path.display(),
            "Worktree created"
        );

        Ok(WorktreeInfo {
            repo: repo_name.to_string(),
            rfc: rfc_name.to_string(),
            path: worktree_path,
            branch: branch_name,
            already_existed: false,
        })
    }

    /// List worktrees for a realm
    pub fn list_worktrees(&self, realm_name: &str) -> Result<Vec<WorktreeInfo>, RealmError> {
        let worktree_base = self.realms_path.parent().unwrap_or(&self.realms_path);
        let realm_worktrees = worktree_base.join("worktrees").join(realm_name);

        if !realm_worktrees.exists() {
            return Ok(Vec::new());
        }

        let mut worktrees = Vec::new();

        // Iterate RFC directories
        let rfc_dirs = std::fs::read_dir(&realm_worktrees).map_err(|e| RealmError::ReadFile {
            path: realm_worktrees.display().to_string(),
            source: e,
        })?;

        for rfc_entry in rfc_dirs {
            let rfc_entry = rfc_entry.map_err(|e| RealmError::ReadFile {
                path: realm_worktrees.display().to_string(),
                source: e,
            })?;

            if !rfc_entry.path().is_dir() {
                continue;
            }

            let rfc_name = rfc_entry.file_name().to_string_lossy().to_string();

            // Iterate repo directories
            let repo_dirs =
                std::fs::read_dir(rfc_entry.path()).map_err(|e| RealmError::ReadFile {
                    path: rfc_entry.path().display().to_string(),
                    source: e,
                })?;

            for repo_entry in repo_dirs {
                let repo_entry = repo_entry.map_err(|e| RealmError::ReadFile {
                    path: rfc_entry.path().display().to_string(),
                    source: e,
                })?;

                if !repo_entry.path().is_dir() {
                    continue;
                }

                let repo_name = repo_entry.file_name().to_string_lossy().to_string();

                worktrees.push(WorktreeInfo {
                    repo: repo_name,
                    rfc: rfc_name.clone(),
                    path: repo_entry.path(),
                    branch: rfc_name.clone(),
                    already_existed: true,
                });
            }
        }

        Ok(worktrees)
    }

    /// Remove worktrees for an RFC
    pub fn remove_worktrees(
        &self,
        realm_name: &str,
        rfc_name: &str,
    ) -> Result<Vec<String>, RealmError> {
        let worktree_base = self.realms_path.parent().unwrap_or(&self.realms_path);
        let rfc_worktrees = worktree_base
            .join("worktrees")
            .join(realm_name)
            .join(rfc_name);

        if !rfc_worktrees.exists() {
            return Ok(Vec::new());
        }

        let mut removed = Vec::new();

        // List repos in this RFC worktree dir
        let repo_dirs = std::fs::read_dir(&rfc_worktrees).map_err(|e| RealmError::ReadFile {
            path: rfc_worktrees.display().to_string(),
            source: e,
        })?;

        for repo_entry in repo_dirs {
            let repo_entry = repo_entry.map_err(|e| RealmError::ReadFile {
                path: rfc_worktrees.display().to_string(),
                source: e,
            })?;

            let repo_name = repo_entry.file_name().to_string_lossy().to_string();
            removed.push(repo_name);
        }

        // Remove the RFC worktree directory
        std::fs::remove_dir_all(&rfc_worktrees).map_err(|e| RealmError::WriteFile {
            path: rfc_worktrees.display().to_string(),
            source: e,
        })?;

        info!(
            realm = %realm_name,
            rfc = %rfc_name,
            repos = ?removed,
            "Worktrees removed"
        );

        Ok(removed)
    }

    // ─── PR Workflow ────────────────────────────────────────────────────────

    /// Get PR status for all worktrees in an RFC
    pub fn pr_status(
        &self,
        realm_name: &str,
        rfc_name: &str,
    ) -> Result<Vec<WorktreePrStatus>, RealmError> {
        let worktrees = self.list_worktrees(realm_name)?;
        let rfc_worktrees: Vec<_> = worktrees
            .into_iter()
            .filter(|wt| wt.rfc == rfc_name)
            .collect();

        let mut statuses = Vec::new();

        for wt in rfc_worktrees {
            let repo = match Repository::open(&wt.path) {
                Ok(r) => r,
                Err(e) => {
                    // Worktree exists but can't open git repo
                    statuses.push(WorktreePrStatus {
                        repo: wt.repo,
                        rfc: wt.rfc,
                        path: wt.path,
                        branch: wt.branch,
                        has_uncommitted: false,
                        modified_files: vec![format!("Error opening repo: {}", e)],
                        commits_ahead: 0,
                    });
                    continue;
                }
            };

            // Check for uncommitted changes
            let git_statuses = repo.statuses(None).map_err(|e| {
                RealmError::ValidationFailed(format!("Failed to get git status: {}", e))
            })?;

            let mut modified_files = Vec::new();
            for entry in git_statuses.iter() {
                let path = entry.path().unwrap_or("unknown").to_string();
                let status = entry.status();
                if status.intersects(
                    git2::Status::INDEX_NEW
                        | git2::Status::INDEX_MODIFIED
                        | git2::Status::INDEX_DELETED
                        | git2::Status::WT_NEW
                        | git2::Status::WT_MODIFIED
                        | git2::Status::WT_DELETED,
                ) {
                    modified_files.push(path);
                }
            }

            // Count commits ahead of main/master
            let commits_ahead = self.count_commits_ahead(&repo, &wt.branch).unwrap_or(0);

            statuses.push(WorktreePrStatus {
                repo: wt.repo,
                rfc: wt.rfc,
                path: wt.path,
                branch: wt.branch,
                has_uncommitted: !modified_files.is_empty(),
                modified_files,
                commits_ahead,
            });
        }

        Ok(statuses)
    }

    /// Prepare worktrees for PR by committing uncommitted changes
    pub fn pr_prepare(
        &self,
        realm_name: &str,
        rfc_name: &str,
        message: Option<&str>,
    ) -> Result<Vec<(String, bool)>, RealmError> {
        let worktrees = self.list_worktrees(realm_name)?;
        let rfc_worktrees: Vec<_> = worktrees
            .into_iter()
            .filter(|wt| wt.rfc == rfc_name)
            .collect();

        let default_message = format!("WIP: {}", rfc_name);
        let commit_message = message.unwrap_or(&default_message);
        let mut results = Vec::new();

        for wt in rfc_worktrees {
            let repo = match Repository::open(&wt.path) {
                Ok(r) => r,
                Err(_) => {
                    results.push((wt.repo, false));
                    continue;
                }
            };

            // Check if there are uncommitted changes
            let statuses = repo.statuses(None).ok();
            let has_changes = statuses
                .map(|s| {
                    s.iter().any(|e| {
                        e.status().intersects(
                            git2::Status::INDEX_NEW
                                | git2::Status::INDEX_MODIFIED
                                | git2::Status::INDEX_DELETED
                                | git2::Status::WT_NEW
                                | git2::Status::WT_MODIFIED
                                | git2::Status::WT_DELETED,
                        )
                    })
                })
                .unwrap_or(false);

            if has_changes {
                // Commit changes
                match self.commit_changes(&repo, commit_message) {
                    Ok(_) => {
                        info!(repo = %wt.repo, rfc = %rfc_name, "Changes committed");
                        results.push((wt.repo, true));
                    }
                    Err(_) => {
                        results.push((wt.repo, false));
                    }
                }
            } else {
                // No changes to commit
                results.push((wt.repo, false));
            }
        }

        Ok(results)
    }

    /// Count commits ahead of main/master branch
    fn count_commits_ahead(
        &self,
        repo: &Repository,
        branch_name: &str,
    ) -> Result<usize, RealmError> {
        // Try to find main or master
        let base_branch = repo
            .find_branch("main", git2::BranchType::Local)
            .or_else(|_| repo.find_branch("master", git2::BranchType::Local))
            .ok();

        let base_commit = match base_branch {
            Some(b) => b.into_reference().peel_to_commit().ok(),
            None => return Ok(0),
        };

        let current_branch = repo.find_branch(branch_name, git2::BranchType::Local).ok();
        let current_commit = match current_branch {
            Some(b) => b.into_reference().peel_to_commit().ok(),
            None => return Ok(0),
        };

        match (base_commit, current_commit) {
            (Some(base), Some(current)) => {
                // Count commits from base to current
                let mut count = 0;
                let mut revwalk = repo.revwalk().map_err(|e| {
                    RealmError::ValidationFailed(format!("Failed to create revwalk: {}", e))
                })?;
                revwalk.push(current.id()).ok();
                revwalk.hide(base.id()).ok();

                for _ in revwalk {
                    count += 1;
                }
                Ok(count)
            }
            _ => Ok(0),
        }
    }

    /// Convert realm info to daemon Realm struct
    pub fn to_daemon_realm(&self, info: &RealmInfo) -> Realm {
        Realm {
            name: info.name.clone(),
            forgejo_url: format!("file://{}", info.path.display()),
            local_path: info.path.display().to_string(),
            last_sync: None,
            status: RealmStatus::Active,
        }
    }

    // ─── Private Helpers ────────────────────────────────────────────────────

    fn create_initial_commit(&self, repo: &Repository, realm_name: &str) -> Result<(), RealmError> {
        let sig = Signature::now("Blue", "blue@local").map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to create signature: {}", e))
        })?;

        let mut index = repo
            .index()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to get index: {}", e)))?;

        // Add all files
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to add files: {}", e)))?;

        index
            .write()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to write index: {}", e)))?;

        let tree_id = index
            .write_tree()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to write tree: {}", e)))?;

        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to find tree: {}", e)))?;

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Initialize realm: {}", realm_name),
            &tree,
            &[],
        )
        .map_err(|e| RealmError::ValidationFailed(format!("Failed to commit: {}", e)))?;

        Ok(())
    }

    fn commit_changes(&self, repo: &Repository, message: &str) -> Result<(), RealmError> {
        let sig = Signature::now("Blue", "blue@local").map_err(|e| {
            RealmError::ValidationFailed(format!("Failed to create signature: {}", e))
        })?;

        let mut index = repo
            .index()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to get index: {}", e)))?;

        // Add all files
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to add files: {}", e)))?;

        index
            .write()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to write index: {}", e)))?;

        let tree_id = index
            .write_tree()
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to write tree: {}", e)))?;

        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to find tree: {}", e)))?;

        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

        let parents: Vec<&git2::Commit> = parent.iter().collect();

        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .map_err(|e| RealmError::ValidationFailed(format!("Failed to commit: {}", e)))?;

        Ok(())
    }
}

/// Information about a realm
#[derive(Debug)]
pub struct RealmInfo {
    pub name: String,
    pub path: PathBuf,
    pub config: RealmConfig,
}

/// Detailed realm info including domains, contracts, and bindings
#[derive(Debug)]
pub struct RealmDetails {
    pub info: RealmInfo,
    pub repos: Vec<RepoConfig>,
    pub domains: Vec<DomainDetails>,
}

/// Domain details including contracts and bindings
#[derive(Debug, Clone)]
pub struct DomainDetails {
    pub domain: Domain,
    pub contracts: Vec<Contract>,
    pub bindings: Vec<Binding>,
}

/// Result of a sync operation
#[derive(Debug)]
pub struct SyncResult {
    pub realm: String,
    pub changes_committed: bool,
    pub last_commit: Option<String>,
    pub message: String,
}

/// Status of a realm's sync state
#[derive(Debug)]
pub struct RealmSyncStatus {
    pub realm: String,
    pub head_commit: Option<String>,
    pub modified_files: Vec<String>,
    pub new_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

impl RealmSyncStatus {
    pub fn has_changes(&self) -> bool {
        !self.modified_files.is_empty()
            || !self.new_files.is_empty()
            || !self.deleted_files.is_empty()
    }
}

/// Result of checking a realm
#[derive(Debug, Default)]
pub struct CheckResult {
    pub realm: String,
    pub errors: Vec<CheckIssue>,
    pub warnings: Vec<CheckIssue>,
}

impl CheckResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// An issue found during check
#[derive(Debug)]
pub struct CheckIssue {
    pub domain: String,
    pub kind: CheckIssueKind,
    pub message: String,
}

/// Kind of issue
#[derive(Debug)]
pub enum CheckIssueKind {
    ContractInvalid,
    BindingBroken,
    ImportUnsatisfied,
    OwnershipViolation,
    VersionMismatch,
}

/// Information about a worktree
#[derive(Debug)]
pub struct WorktreeInfo {
    pub repo: String,
    pub rfc: String,
    pub path: PathBuf,
    pub branch: String,
    pub already_existed: bool,
}

/// Status of a worktree for PR purposes
#[derive(Debug)]
pub struct WorktreePrStatus {
    pub repo: String,
    pub rfc: String,
    pub path: PathBuf,
    pub branch: String,
    pub has_uncommitted: bool,
    pub modified_files: Vec<String>,
    pub commits_ahead: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_service() -> (RealmService, TempDir) {
        let tmp = TempDir::new().unwrap();
        let service = RealmService::new(tmp.path().to_path_buf());
        (service, tmp)
    }

    #[test]
    fn test_init_realm() {
        let (service, _tmp) = test_service();

        let info = service.init_realm("test-realm").unwrap();
        assert_eq!(info.name, "test-realm");
        assert!(info.path.join("realm.yaml").exists());
        assert!(info.path.join("repos").exists());
        assert!(info.path.join("domains").exists());

        // Verify git repo was created
        assert!(info.path.join(".git").exists());
    }

    #[test]
    fn test_init_realm_already_exists() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();
        let result = service.init_realm("test-realm");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_realms() {
        let (service, _tmp) = test_service();

        service.init_realm("realm-a").unwrap();
        service.init_realm("realm-b").unwrap();

        let realms = service.list_realms().unwrap();
        assert_eq!(realms.len(), 2);
        assert!(realms.contains(&"realm-a".to_string()));
        assert!(realms.contains(&"realm-b".to_string()));
    }

    #[test]
    fn test_join_realm() {
        let (service, tmp) = test_service();

        // Create realm
        service.init_realm("test-realm").unwrap();

        // Create a fake repo directory
        let repo_path = tmp.path().join("my-repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        // Join realm
        service
            .join_realm("test-realm", "my-repo", &repo_path)
            .unwrap();

        // Verify repo registration
        let repo_config_path = tmp
            .path()
            .join("test-realm")
            .join("repos")
            .join("my-repo.yaml");
        assert!(repo_config_path.exists());

        // Verify local config
        let local_config_path = repo_path.join(".blue").join("config.yaml");
        assert!(local_config_path.exists());
    }

    #[test]
    fn test_create_domain() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();
        service
            .create_domain(
                "test-realm",
                "s3-access",
                &["aperture".to_string(), "fungal".to_string()],
            )
            .unwrap();

        let domain_path = service
            .realms_path
            .join("test-realm")
            .join("domains")
            .join("s3-access");
        assert!(domain_path.join("domain.yaml").exists());
        assert!(domain_path.join("contracts").exists());
        assert!(domain_path.join("bindings").exists());
    }

    #[test]
    fn test_sync_clean_realm() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();

        // Sync a clean realm
        let result = service.sync_realm("test-realm", false).unwrap();
        assert!(!result.changes_committed);
        assert_eq!(result.message, "Already up to date");
        assert!(result.last_commit.is_some());
    }

    #[test]
    fn test_sync_with_changes() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();

        // Create a new file
        let realm_path = service.realms_path.join("test-realm");
        std::fs::write(realm_path.join("test.txt"), "hello").unwrap();

        // Check status shows changes
        let status = service.realm_sync_status("test-realm").unwrap();
        assert!(status.has_changes());
        assert!(status.new_files.iter().any(|f| f.contains("test.txt")));

        // Sync should commit the changes
        let result = service.sync_realm("test-realm", false).unwrap();
        assert!(result.changes_committed);
        assert_eq!(result.message, "Changes committed");

        // Status should now be clean
        let status = service.realm_sync_status("test-realm").unwrap();
        assert!(!status.has_changes());
    }

    #[test]
    fn test_load_realm_details() {
        let (service, tmp) = test_service();

        // Create realm with domain
        service.init_realm("test-realm").unwrap();

        // Create a repo directory and join
        let repo_path = tmp.path().join("my-repo");
        std::fs::create_dir_all(&repo_path).unwrap();
        service
            .join_realm("test-realm", "my-repo", &repo_path)
            .unwrap();

        // Create domain
        service
            .create_domain("test-realm", "s3-access", &["my-repo".to_string()])
            .unwrap();

        // Load details
        let details = service.load_realm_details("test-realm").unwrap();
        assert_eq!(details.info.name, "test-realm");
        assert_eq!(details.repos.len(), 1);
        assert_eq!(details.repos[0].name, "my-repo");
        assert_eq!(details.domains.len(), 1);
        assert_eq!(details.domains[0].domain.name, "s3-access");
    }

    #[test]
    fn test_check_empty_realm() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();

        let result = service.check_realm("test-realm").unwrap();
        assert!(result.is_ok());
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_check_with_valid_contract() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();
        service
            .create_domain("test-realm", "s3-access", &["aperture".to_string()])
            .unwrap();
        service
            .create_contract("test-realm", "s3-access", "s3-permissions", "aperture")
            .unwrap();

        let result = service.check_realm("test-realm").unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cache_returns_same_data() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();
        service
            .create_domain("test-realm", "s3-access", &["aperture".to_string()])
            .unwrap();

        // First call loads from disk
        let domains1 = service.load_domains("test-realm").unwrap();

        // Second call should return cached data
        let domains2 = service.load_domains("test-realm").unwrap();

        assert_eq!(domains1.len(), domains2.len());
        assert_eq!(domains1[0].domain.name, domains2[0].domain.name);
    }

    #[test]
    fn test_cache_invalidation() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();

        // Load repos (should cache empty list)
        let repos1 = service.load_repos("test-realm").unwrap();
        assert_eq!(repos1.len(), 0);

        // Create a repo directory and join (this should invalidate cache)
        let repo_path = service.realms_path.join("my-repo");
        std::fs::create_dir_all(&repo_path).unwrap();
        service
            .join_realm("test-realm", "my-repo", &repo_path)
            .unwrap();

        // Load repos again (should see new repo since cache was invalidated)
        let repos2 = service.load_repos("test-realm").unwrap();
        assert_eq!(repos2.len(), 1);
    }

    #[test]
    fn test_manual_cache_invalidation() {
        let (service, _tmp) = test_service();

        service.init_realm("test-realm").unwrap();

        // Load to populate cache
        let info1 = service.load_realm("test-realm").unwrap();

        // Manually invalidate
        service.invalidate_cache("test-realm");

        // Load again - should still work
        let info2 = service.load_realm("test-realm").unwrap();

        assert_eq!(info1.name, info2.name);
    }

    #[test]
    fn test_cache_with_custom_ttl() {
        let tmp = TempDir::new().unwrap();
        let service = RealmService::with_cache_ttl(
            tmp.path().to_path_buf(),
            Duration::from_millis(100), // Very short TTL
        );

        service.init_realm("test-realm").unwrap();

        // Load to populate cache
        let _info1 = service.load_realm("test-realm").unwrap();

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(150));

        // Should reload from disk (no error means success)
        let _info2 = service.load_realm("test-realm").unwrap();
    }
}
