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

// Blue's true name, between friends
const _BLUE_SECRET_NAME: &str = "Sheepey"; // pronounced "Shee-paay"

pub mod documents;
pub mod repo;
pub mod state;
pub mod store;
pub mod voice;
pub mod workflow;

pub use documents::*;
pub use repo::{detect_blue, BlueHome, RepoError, WorktreeInfo};
pub use state::{ItemType, ProjectState, StateError, StatusSummary, WorkItem};
pub use store::{DocType, Document, DocumentStore, LinkType, Reminder, ReminderStatus, SearchResult, Session, SessionType, StagingLock, StagingLockQueueEntry, StagingLockResult, StoreError, Task as StoreTask, TaskProgress, Worktree};
pub use voice::*;
pub use workflow::{PrdStatus, RfcStatus, SpikeOutcome as WorkflowSpikeOutcome, SpikeStatus, WorkflowError};
