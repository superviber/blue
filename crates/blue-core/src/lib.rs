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

pub mod alignment;
pub mod alignment_db;
pub mod daemon;
pub mod documents;
pub mod forge;
pub mod indexer;
pub mod llm;
pub mod manifest;
pub mod plan;
pub mod realm;
pub mod repo;
pub mod state;
pub mod store;
pub mod uri;
pub mod voice;
pub mod workflow;

pub use alignment::{AlignmentDialogue, AlignmentScore, DialogueStatus, Expert, ExpertResponse, ExpertTier, PanelTemplate, Perspective, PerspectiveStatus, Round, Tension, TensionStatus, build_expert_prompt, parse_expert_response};
pub use documents::{Adr, Audit, AuditFinding, AuditSeverity, AuditType, Decision, HeaderFormat, Rfc, Spike, SpikeOutcome, Status, Task, convert_inline_to_table_header, update_markdown_status, utc_timestamp, validate_rfc_header};
pub use forge::{AwsConfig, BlueConfig, ConfigError, CreatePrOpts, Forge, ForgeConfig, ForgeError, ForgeType, ForgejoForge, GitHubForge, GitUrl, MergeStrategy, PrState, PullRequest, ReleaseConfig, WorktreeConfig, create_forge, create_forge_cached, detect_forge_type, detect_forge_type_cached, get_token, parse_git_url};
pub use indexer::{Indexer, IndexerConfig, IndexerError, IndexResult, ParsedSymbol, is_indexable_file, should_skip_dir, DEFAULT_INDEX_MODEL, MAX_FILE_LINES};
pub use llm::{CompletionOptions, CompletionResult, LlmBackendChoice, LlmConfig, LlmError, LlmManager, LlmProvider, LlmProviderChoice, LocalLlmConfig, ApiLlmConfig, KeywordLlm, MockLlm, ProviderStatus};
pub use repo::{detect_blue, BlueHome, RepoError, WorktreeInfo};
pub use state::{ItemType, ProjectState, StateError, StatusSummary, WorkItem};
pub use store::{ContextInjection, DocType, Document, DocumentStore, EdgeType, FileIndexEntry, IndexSearchResult, IndexStatus, LinkType, ParsedDocument, ReconcileResult, RefreshPolicy, RefreshRateLimit, RelevanceEdge, Reminder, ReminderStatus, SearchResult, Session, SessionType, StagingLock, StagingLockQueueEntry, StagingLockResult, StalenessCheck, StalenessReason, StoreError, SymbolIndexEntry, Task as StoreTask, TaskProgress, Worktree, INDEX_PROMPT_VERSION, hash_content, parse_document_from_file, rebuild_filename, rename_for_status, status_suffix, title_to_slug};
pub use voice::*;
pub use workflow::{PrdStatus, RfcStatus, SpikeOutcome as WorkflowSpikeOutcome, SpikeStatus, WorkflowError, validate_rfc_transition};
pub use manifest::{ContextManifest, IdentityConfig, WorkflowConfig, ReferenceConfig, PluginConfig, SourceConfig, RefreshTrigger, SalienceTrigger, ManifestError, ManifestResolution, TierResolution, ResolvedSource};
pub use uri::{BlueUri, UriError, read_uri_content, estimate_tokens};
pub use plan::{PlanFile, PlanStatus, PlanTask, PlanError, parse_plan_markdown, generate_plan_markdown, plan_file_path, is_cache_stale, read_plan_file, write_plan_file, update_task_in_plan};
