//! Realm data model for cross-repo coordination
//!
//! This module defines the data structures for:
//! - Realms (groups of coordinated repos)
//! - Domains (coordination contexts between repos)
//! - Contracts (schemas defining shared data)
//! - Bindings (export/import declarations)
//!
//! See RFC 0001: Cross-Repo Coordination with Realms

mod config;
mod contract;
mod domain;
mod repo;
mod service;

pub use config::{
    AdmissionPolicy, BreakingChangePolicy, Governance, RealmConfig, TrustConfig, TrustMode,
};
pub use contract::{Compatibility, Contract, ContractValue, EvolutionEntry, ValidationConfig};
pub use domain::{Binding, BindingRole, Domain, ExportBinding, ImportBinding, ImportStatus};
pub use repo::{
    LocalRealmDependencies, LocalRealmMembership, LocalRepoConfig, RealmRef, RepoConfig,
    RfcDependencies,
};
pub use service::{
    CheckIssue, CheckIssueKind, CheckResult, DomainDetails, RealmDetails, RealmInfo, RealmService,
    RealmSyncStatus, SyncResult, WorktreeInfo, WorktreePrStatus,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RealmError {
    #[error("Failed to read file {path}: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Invalid semver version: {0}")]
    InvalidVersion(#[from] semver::Error),

    #[error("Contract not found: {0}")]
    ContractNotFound(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("Repo not found: {0}")]
    RepoNotFound(String),

    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    #[error("Ownership violation: {contract} is owned by {owner}, not {attempted}")]
    OwnershipViolation {
        contract: String,
        owner: String,
        attempted: String,
    },

    #[error("Cycle detected: {0}")]
    CycleDetected(String),
}
