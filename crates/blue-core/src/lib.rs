//! Blue Core - The heart of the philosophy
//!
//! Core data structures and logic for Blue.
//!
//! This crate provides:
//! - Document types (RFC, Spike, ADR, Decision)
//! - SQLite persistence layer
//! - Git worktree operations
//! - Project state management
//! - Blue's voice and tone
//! - Daemon for cross-repo coordination

// Blue's true name, between friends
const _BLUE_SECRET_NAME: &str = "Sheepey"; // pronounced "Shee-paay"

pub mod handler_error;
pub mod handlers;
pub mod alignment;
pub mod alignment_db;
pub mod daemon;
pub mod documents;
pub mod forge;
pub mod indexer;
pub mod install;

pub mod manifest;
pub mod org;
pub mod plan;
pub mod pm;
pub mod realm;
pub mod repo;
pub mod state;
pub mod store;
pub mod tracker;
pub mod uri;
pub mod voice;
pub mod workflow;

pub use alignment::{
    build_expert_prompt, parse_expert_response, AlignmentDialogue, AlignmentScore, DialogueStatus,
    Expert, ExpertResponse, ExpertTier, PanelTemplate, Perspective, PerspectiveStatus, Round,
    Tension, TensionStatus,
};
pub use documents::{
    convert_inline_to_table_header, update_markdown_status, utc_timestamp, validate_rfc_header,
    Adr, Audit, AuditFinding, AuditSeverity, AuditType, Decision, HeaderFormat, Rfc, Spike,
    SpikeOutcome, Status, Task,
};
pub use forge::{
    create_forge, create_forge_cached, detect_forge_type, detect_forge_type_cached, get_token,
    parse_git_url, AwsConfig, BlueConfig, ConfigError, CreatePrOpts, Forge, ForgeConfig,
    ForgeError, ForgeType, ForgejoForge, GitHubForge, GitUrl, MergeStrategy, PrState, PullRequest,
    ReleaseConfig, WorktreeConfig,
};
pub use indexer::{
    generate_index_prompt, is_indexable_file, parse_index_response, should_skip_dir, IndexResult,
    Indexer, IndexerConfig, IndexerError, ParsedSymbol, DEFAULT_INDEX_MODEL, MAX_FILE_LINES,
};

pub use manifest::{
    ContextManifest, IdentityConfig, ManifestError, ManifestResolution, PluginConfig,
    ReferenceConfig, RefreshTrigger, ResolvedSource, SalienceTrigger, SourceConfig, TierResolution,
    WorkflowConfig,
};
pub use plan::{
    generate_plan_markdown, is_cache_stale, parse_plan_markdown, plan_file_path, read_plan_file,
    update_task_in_plan, write_plan_file, PlanError, PlanFile, PlanStatus, PlanTask,
};
pub use repo::{detect_blue, BlueHome, RepoError, WorktreeInfo};
pub use state::{ItemType, ProjectState, StateError, StatusSummary, WorkItem};
pub use store::{
    hash_content, parse_document_from_file, rebuild_filename, rename_for_status, status_suffix,
    title_to_slug, ContextInjection, DocType, Document, DocumentStore, EdgeType, FileIndexEntry,
    IndexSearchResult, IndexStatus, LinkType, ParsedDocument, ReconcileResult, RefreshPolicy,
    RefreshRateLimit, RelevanceEdge, Reminder, ReminderStatus, SearchResult, Session, SessionType,
    StagingLock, StagingLockQueueEntry, StagingLockResult, StalenessCheck, StalenessReason,
    StoreError, SymbolIndexEntry, Task as StoreTask, TaskProgress, Worktree, INDEX_PROMPT_VERSION,
};
pub use tracker::{
    check_for_jira_credentials, create_tracker_from_env, parse_jira_binding, parse_rfc_status,
    parse_rfc_title, rfc_status_to_jira, run_sync, update_jira_binding, AuthStatus,
    CreateIssueOpts, CreateProjectOpts, CredentialStore, DriftPolicy, DriftReport, ImportScan, Issue, IssueStatus,
    IssueTracker, IssueType, JiraBinding, JiraCloudTracker, LintSeverity, LintWarning,
    StatusCategory, SyncAction, SyncConfig, SyncReport, SyncResult, TrackerConfig,
    TrackerCredentials, TrackerError, TrackerProject, TrackerType, TransitionOpts,
};
pub use pm::domain::{PmDomain, PmDomainError, RepoEntry};
pub use pm::id::{format_epic_id, format_story_id, next_epic_id, next_story_id, parse_id, IdError};
pub use pm::locator::{locate_pm_repo, locate_pm_repo_from_org, locate_pm_repo_with_config, LocatorError, PmRepoLocation};
pub use pm::sync::{discover_pm_items, run_pm_sync, parse_pm_front_matter, PmFrontMatter, PmItem};
pub use org::{
    clone_repo, clone_repo_by_name, config_path as org_config_path, detect_org_from_repo,
    execute_move, parse_remote_url, scan_for_migration, BlueGlobalConfig, HomeConfig,
    MigrationMove, Org, OrgError, OrgManifest, Provider,
};
pub use uri::{estimate_tokens, read_uri_content, BlueUri, UriError};
pub use handler_error::HandlerError;
pub use voice::*;
pub use workflow::{
    validate_rfc_transition, PrdStatus, RfcStatus, SpikeOutcome as WorkflowSpikeOutcome,
    SpikeStatus, WorkflowError,
};
