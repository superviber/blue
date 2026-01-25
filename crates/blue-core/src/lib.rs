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
pub mod daemon;
pub mod documents;
pub mod forge;
pub mod indexer;
pub mod llm;
pub mod manifest;
pub mod realm;
pub mod repo;
pub mod state;
pub mod store;
pub mod uri;
pub mod voice;
pub mod workflow;

pub use alignment::{AlignmentDialogue, AlignmentScore, DialogueStatus, Expert, ExpertResponse, ExpertTier, PanelTemplate, Perspective, PerspectiveStatus, Round, Tension, TensionStatus, build_expert_prompt, parse_expert_response};
pub use documents::{Adr, Audit, AuditFinding, AuditSeverity, AuditType, Decision, Rfc, Spike, SpikeOutcome, Status, Task, update_markdown_status};
pub use forge::{BlueConfig, CreatePrOpts, Forge, ForgeConfig, ForgeError, ForgeType, ForgejoForge, GitHubForge, GitUrl, MergeStrategy, PrState, PullRequest, create_forge, create_forge_cached, detect_forge_type, detect_forge_type_cached, get_token, parse_git_url};
pub use indexer::{Indexer, IndexerConfig, IndexerError, IndexResult, ParsedSymbol, is_indexable_file, should_skip_dir, DEFAULT_INDEX_MODEL, MAX_FILE_LINES};
pub use llm::{CompletionOptions, CompletionResult, LlmBackendChoice, LlmConfig, LlmError, LlmManager, LlmProvider, LlmProviderChoice, LocalLlmConfig, ApiLlmConfig, KeywordLlm, MockLlm, ProviderStatus};
pub use repo::{detect_blue, BlueHome, RepoError, WorktreeInfo};
pub use state::{ItemType, ProjectState, StateError, StatusSummary, WorkItem};
pub use store::{ContextInjection, DocType, Document, DocumentStore, EdgeType, FileIndexEntry, IndexSearchResult, IndexStatus, LinkType, RefreshPolicy, RefreshRateLimit, RelevanceEdge, Reminder, ReminderStatus, SearchResult, Session, SessionType, StagingLock, StagingLockQueueEntry, StagingLockResult, StalenessCheck, StalenessReason, StoreError, SymbolIndexEntry, Task as StoreTask, TaskProgress, Worktree, INDEX_PROMPT_VERSION};
pub use voice::*;
pub use workflow::{PrdStatus, RfcStatus, SpikeOutcome as WorkflowSpikeOutcome, SpikeStatus, WorkflowError, validate_rfc_transition};
pub use manifest::{ContextManifest, IdentityConfig, WorkflowConfig, ReferenceConfig, PluginConfig, SourceConfig, RefreshTrigger, SalienceTrigger, ManifestError, ManifestResolution, TierResolution, ResolvedSource};
pub use uri::{BlueUri, UriError, read_uri_content, estimate_tokens};
