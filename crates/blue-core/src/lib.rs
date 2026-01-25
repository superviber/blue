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

pub mod daemon;
pub mod documents;
pub mod forge;
pub mod indexer;
pub mod llm;
pub mod realm;
pub mod repo;
pub mod state;
pub mod store;
pub mod voice;
pub mod workflow;

pub use documents::{Adr, Audit, AuditFinding, AuditSeverity, AuditType, Decision, Rfc, Spike, SpikeOutcome, Status, Task, update_markdown_status};
pub use forge::{CreatePrOpts, Forge, ForgeConfig, ForgeError, ForgeType, ForgejoForge, GitHubForge, GitUrl, MergeStrategy, PrState, PullRequest, create_forge, detect_forge_type, get_token, parse_git_url};
pub use indexer::{Indexer, IndexerConfig, IndexerError, IndexResult, ParsedSymbol, is_indexable_file, should_skip_dir, DEFAULT_INDEX_MODEL, MAX_FILE_LINES};
pub use llm::{CompletionOptions, CompletionResult, LlmBackendChoice, LlmConfig, LlmError, LlmManager, LlmProvider, LlmProviderChoice, LocalLlmConfig, ApiLlmConfig, KeywordLlm, MockLlm, ProviderStatus};
pub use repo::{detect_blue, BlueHome, RepoError, WorktreeInfo};
pub use state::{ItemType, ProjectState, StateError, StatusSummary, WorkItem};
pub use store::{DocType, Document, DocumentStore, FileIndexEntry, IndexSearchResult, IndexStatus, LinkType, Reminder, ReminderStatus, SearchResult, Session, SessionType, StagingLock, StagingLockQueueEntry, StagingLockResult, StoreError, SymbolIndexEntry, Task as StoreTask, TaskProgress, Worktree, INDEX_PROMPT_VERSION};
pub use voice::*;
pub use workflow::{PrdStatus, RfcStatus, SpikeOutcome as WorkflowSpikeOutcome, SpikeStatus, WorkflowError};
