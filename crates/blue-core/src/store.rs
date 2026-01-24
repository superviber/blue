//! SQLite document store for Blue
//!
//! Persistence layer for RFCs, Spikes, ADRs, and other documents.

use std::path::Path;
use std::thread;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use tracing::{debug, info, warn};

/// Current schema version
const SCHEMA_VERSION: i32 = 4;

/// Core database schema
const SCHEMA: &str = r#"
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY
    );

    CREATE TABLE IF NOT EXISTS documents (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        doc_type TEXT NOT NULL,
        number INTEGER,
        title TEXT NOT NULL,
        status TEXT NOT NULL,
        file_path TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        deleted_at TEXT,
        UNIQUE(doc_type, title)
    );

    CREATE INDEX IF NOT EXISTS idx_documents_type ON documents(doc_type);
    CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(doc_type, status);
    CREATE INDEX IF NOT EXISTS idx_documents_deleted ON documents(deleted_at) WHERE deleted_at IS NOT NULL;

    CREATE TABLE IF NOT EXISTS document_links (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_id INTEGER NOT NULL,
        target_id INTEGER NOT NULL,
        link_type TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY (source_id) REFERENCES documents(id) ON DELETE CASCADE,
        FOREIGN KEY (target_id) REFERENCES documents(id) ON DELETE CASCADE,
        UNIQUE(source_id, target_id, link_type)
    );

    CREATE TABLE IF NOT EXISTS tasks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_id INTEGER NOT NULL,
        task_index INTEGER NOT NULL,
        description TEXT NOT NULL,
        completed INTEGER NOT NULL DEFAULT 0,
        completed_at TEXT,
        FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
        UNIQUE(document_id, task_index)
    );

    CREATE TABLE IF NOT EXISTS worktrees (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_id INTEGER NOT NULL,
        branch_name TEXT NOT NULL,
        worktree_path TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
        UNIQUE(document_id)
    );

    CREATE TABLE IF NOT EXISTS metadata (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_id INTEGER NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE,
        UNIQUE(document_id, key)
    );

    CREATE TABLE IF NOT EXISTS sessions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        rfc_title TEXT NOT NULL,
        session_type TEXT NOT NULL DEFAULT 'implementation',
        started_at TEXT NOT NULL,
        last_heartbeat TEXT NOT NULL,
        ended_at TEXT,
        UNIQUE(rfc_title)
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(ended_at) WHERE ended_at IS NULL;

    CREATE TABLE IF NOT EXISTS reminders (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        title TEXT NOT NULL,
        context TEXT,
        gate TEXT,
        due_date TEXT,
        snooze_until TEXT,
        status TEXT NOT NULL DEFAULT 'pending',
        linked_doc_id INTEGER,
        created_at TEXT NOT NULL,
        cleared_at TEXT,
        resolution TEXT,
        FOREIGN KEY (linked_doc_id) REFERENCES documents(id) ON DELETE SET NULL
    );

    CREATE INDEX IF NOT EXISTS idx_reminders_status ON reminders(status);
    CREATE INDEX IF NOT EXISTS idx_reminders_due ON reminders(due_date) WHERE status = 'pending';

    CREATE TABLE IF NOT EXISTS staging_locks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        resource TEXT NOT NULL UNIQUE,
        locked_by TEXT NOT NULL,
        agent_id TEXT,
        locked_at TEXT NOT NULL,
        expires_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS staging_lock_queue (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        resource TEXT NOT NULL,
        requester TEXT NOT NULL,
        agent_id TEXT,
        requested_at TEXT NOT NULL,
        FOREIGN KEY (resource) REFERENCES staging_locks(resource) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_staging_locks_resource ON staging_locks(resource);
    CREATE INDEX IF NOT EXISTS idx_staging_queue_resource ON staging_lock_queue(resource);

    CREATE TABLE IF NOT EXISTS staging_deployments (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        iac_type TEXT NOT NULL,
        deploy_command TEXT NOT NULL,
        stacks TEXT,
        deployed_by TEXT NOT NULL,
        agent_id TEXT,
        deployed_at TEXT NOT NULL,
        ttl_expires_at TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'deployed',
        destroyed_at TEXT,
        metadata TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_staging_deployments_status ON staging_deployments(status);
    CREATE INDEX IF NOT EXISTS idx_staging_deployments_expires ON staging_deployments(ttl_expires_at);

    -- Semantic index for files (RFC 0010)
    CREATE TABLE IF NOT EXISTS file_index (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        realm TEXT NOT NULL,
        repo TEXT NOT NULL,
        file_path TEXT NOT NULL,
        file_hash TEXT NOT NULL,
        summary TEXT,
        relationships TEXT,
        indexed_at TEXT NOT NULL,
        prompt_version INTEGER DEFAULT 1,
        embedding BLOB,
        UNIQUE(realm, repo, file_path)
    );

    CREATE INDEX IF NOT EXISTS idx_file_index_realm ON file_index(realm);
    CREATE INDEX IF NOT EXISTS idx_file_index_repo ON file_index(realm, repo);
    CREATE INDEX IF NOT EXISTS idx_file_index_hash ON file_index(file_hash);

    -- Symbol-level index
    CREATE TABLE IF NOT EXISTS symbol_index (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        file_id INTEGER NOT NULL,
        name TEXT NOT NULL,
        kind TEXT NOT NULL,
        start_line INTEGER,
        end_line INTEGER,
        description TEXT,
        FOREIGN KEY (file_id) REFERENCES file_index(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_symbol_index_file ON symbol_index(file_id);
    CREATE INDEX IF NOT EXISTS idx_symbol_index_name ON symbol_index(name);
"#;

/// FTS5 schema for full-text search
const FTS5_SCHEMA: &str = r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
        title,
        content,
        doc_type,
        content=documents,
        content_rowid=id
    );

    CREATE TRIGGER IF NOT EXISTS documents_ai AFTER INSERT ON documents BEGIN
        INSERT INTO documents_fts(rowid, title, doc_type)
        VALUES (new.id, new.title, new.doc_type);
    END;

    CREATE TRIGGER IF NOT EXISTS documents_ad AFTER DELETE ON documents BEGIN
        INSERT INTO documents_fts(documents_fts, rowid, title, doc_type)
        VALUES ('delete', old.id, old.title, old.doc_type);
    END;

    CREATE TRIGGER IF NOT EXISTS documents_au AFTER UPDATE ON documents BEGIN
        INSERT INTO documents_fts(documents_fts, rowid, title, doc_type)
        VALUES ('delete', old.id, old.title, old.doc_type);
        INSERT INTO documents_fts(rowid, title, doc_type)
        VALUES (new.id, new.title, new.doc_type);
    END;
"#;

/// FTS5 schema for semantic file index (RFC 0010)
const FILE_INDEX_FTS5_SCHEMA: &str = r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS file_index_fts USING fts5(
        file_path,
        summary,
        relationships,
        content=file_index,
        content_rowid=id
    );

    CREATE TRIGGER IF NOT EXISTS file_index_ai AFTER INSERT ON file_index BEGIN
        INSERT INTO file_index_fts(rowid, file_path, summary, relationships)
        VALUES (new.id, new.file_path, new.summary, new.relationships);
    END;

    CREATE TRIGGER IF NOT EXISTS file_index_ad AFTER DELETE ON file_index BEGIN
        INSERT INTO file_index_fts(file_index_fts, rowid, file_path, summary, relationships)
        VALUES ('delete', old.id, old.file_path, old.summary, old.relationships);
    END;

    CREATE TRIGGER IF NOT EXISTS file_index_au AFTER UPDATE ON file_index BEGIN
        INSERT INTO file_index_fts(file_index_fts, rowid, file_path, summary, relationships)
        VALUES ('delete', old.id, old.file_path, old.summary, old.relationships);
        INSERT INTO file_index_fts(rowid, file_path, summary, relationships)
        VALUES (new.id, new.file_path, new.summary, new.relationships);
    END;

    CREATE VIRTUAL TABLE IF NOT EXISTS symbol_index_fts USING fts5(
        name,
        description,
        content=symbol_index,
        content_rowid=id
    );

    CREATE TRIGGER IF NOT EXISTS symbol_index_ai AFTER INSERT ON symbol_index BEGIN
        INSERT INTO symbol_index_fts(rowid, name, description)
        VALUES (new.id, new.name, new.description);
    END;

    CREATE TRIGGER IF NOT EXISTS symbol_index_ad AFTER DELETE ON symbol_index BEGIN
        INSERT INTO symbol_index_fts(symbol_index_fts, rowid, name, description)
        VALUES ('delete', old.id, old.name, old.description);
    END;

    CREATE TRIGGER IF NOT EXISTS symbol_index_au AFTER UPDATE ON symbol_index BEGIN
        INSERT INTO symbol_index_fts(symbol_index_fts, rowid, name, description)
        VALUES ('delete', old.id, old.name, old.description);
        INSERT INTO symbol_index_fts(rowid, name, description)
        VALUES (new.id, new.name, new.description);
    END;
"#;

/// Document types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Rfc,
    Spike,
    Adr,
    Decision,
    Prd,
    Postmortem,
    Runbook,
    Dialogue,
    Audit,
}

impl DocType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocType::Rfc => "rfc",
            DocType::Spike => "spike",
            DocType::Adr => "adr",
            DocType::Decision => "decision",
            DocType::Prd => "prd",
            DocType::Postmortem => "postmortem",
            DocType::Runbook => "runbook",
            DocType::Dialogue => "dialogue",
            DocType::Audit => "audit",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rfc" => Some(DocType::Rfc),
            "spike" => Some(DocType::Spike),
            "adr" => Some(DocType::Adr),
            "decision" => Some(DocType::Decision),
            "prd" => Some(DocType::Prd),
            "postmortem" => Some(DocType::Postmortem),
            "runbook" => Some(DocType::Runbook),
            "dialogue" => Some(DocType::Dialogue),
            "audit" => Some(DocType::Audit),
            _ => None,
        }
    }

    /// Human-readable plural for Blue's messages
    pub fn plural(&self) -> &'static str {
        match self {
            DocType::Rfc => "RFCs",
            DocType::Spike => "spikes",
            DocType::Adr => "ADRs",
            DocType::Decision => "decisions",
            DocType::Prd => "PRDs",
            DocType::Postmortem => "post-mortems",
            DocType::Runbook => "runbooks",
            DocType::Dialogue => "dialogues",
            DocType::Audit => "audits",
        }
    }
}

/// Link types between documents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    /// Spike leads to RFC
    SpikeToRfc,
    /// RFC leads to ADR
    RfcToAdr,
    /// PRD leads to RFC
    PrdToRfc,
    /// Dialogue documents an RFC implementation
    DialogueToRfc,
    /// Generic reference
    References,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::SpikeToRfc => "spike_to_rfc",
            LinkType::RfcToAdr => "rfc_to_adr",
            LinkType::PrdToRfc => "prd_to_rfc",
            LinkType::DialogueToRfc => "dialogue_to_rfc",
            LinkType::References => "references",
        }
    }
}

/// A document in the store
#[derive(Debug, Clone)]
pub struct Document {
    pub id: Option<i64>,
    pub doc_type: DocType,
    pub number: Option<i32>,
    pub title: String,
    pub status: String,
    pub file_path: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub deleted_at: Option<String>,
}

impl Document {
    /// Create a new document
    pub fn new(doc_type: DocType, title: &str, status: &str) -> Self {
        Self {
            id: None,
            doc_type,
            number: None,
            title: title.to_string(),
            status: status.to_string(),
            file_path: None,
            created_at: None,
            updated_at: None,
            deleted_at: None,
        }
    }

    /// Check if document is soft-deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// A task in a document's plan
#[derive(Debug, Clone)]
pub struct Task {
    pub id: Option<i64>,
    pub document_id: i64,
    pub task_index: i32,
    pub description: String,
    pub completed: bool,
    pub completed_at: Option<String>,
}

/// Task completion progress
#[derive(Debug, Clone)]
pub struct TaskProgress {
    pub completed: usize,
    pub total: usize,
    pub percentage: usize,
}

/// A worktree associated with a document
#[derive(Debug, Clone)]
pub struct Worktree {
    pub id: Option<i64>,
    pub document_id: i64,
    pub branch_name: String,
    pub worktree_path: String,
    pub created_at: Option<String>,
}

/// Search result with relevance score
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub document: Document,
    pub score: f64,
    pub snippet: Option<String>,
}

/// Session types for multi-agent coordination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Implementation,
    Review,
    Testing,
}

impl SessionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionType::Implementation => "implementation",
            SessionType::Review => "review",
            SessionType::Testing => "testing",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "implementation" => Some(SessionType::Implementation),
            "review" => Some(SessionType::Review),
            "testing" => Some(SessionType::Testing),
            _ => None,
        }
    }
}

/// An active session on an RFC
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Option<i64>,
    pub rfc_title: String,
    pub session_type: SessionType,
    pub started_at: String,
    pub last_heartbeat: String,
    pub ended_at: Option<String>,
}

/// Reminder status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderStatus {
    Pending,
    Snoozed,
    Cleared,
}

impl ReminderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReminderStatus::Pending => "pending",
            ReminderStatus::Snoozed => "snoozed",
            ReminderStatus::Cleared => "cleared",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(ReminderStatus::Pending),
            "snoozed" => Some(ReminderStatus::Snoozed),
            "cleared" => Some(ReminderStatus::Cleared),
            _ => None,
        }
    }
}

/// A reminder with optional gate condition
#[derive(Debug, Clone)]
pub struct Reminder {
    pub id: Option<i64>,
    pub title: String,
    pub context: Option<String>,
    pub gate: Option<String>,
    pub due_date: Option<String>,
    pub snooze_until: Option<String>,
    pub status: ReminderStatus,
    pub linked_doc_id: Option<i64>,
    pub created_at: Option<String>,
    pub cleared_at: Option<String>,
    pub resolution: Option<String>,
}

impl Reminder {
    pub fn new(title: &str) -> Self {
        Self {
            id: None,
            title: title.to_string(),
            context: None,
            gate: None,
            due_date: None,
            snooze_until: None,
            status: ReminderStatus::Pending,
            linked_doc_id: None,
            created_at: None,
            cleared_at: None,
            resolution: None,
        }
    }
}

/// A staging resource lock
#[derive(Debug, Clone)]
pub struct StagingLock {
    pub id: Option<i64>,
    pub resource: String,
    pub locked_by: String,
    pub agent_id: Option<String>,
    pub locked_at: String,
    pub expires_at: String,
}

/// A queued request for a staging lock
#[derive(Debug, Clone)]
pub struct StagingLockQueueEntry {
    pub id: Option<i64>,
    pub resource: String,
    pub requester: String,
    pub agent_id: Option<String>,
    pub requested_at: String,
}

/// Result of attempting to acquire a staging lock
#[derive(Debug)]
pub enum StagingLockResult {
    /// Lock was acquired
    Acquired { expires_at: String },
    /// Lock is held by someone else, added to queue
    Queued {
        position: usize,
        current_holder: String,
        expires_at: String,
    },
}

/// A tracked staging deployment with TTL
#[derive(Debug, Clone)]
pub struct StagingDeployment {
    pub id: Option<i64>,
    pub name: String,
    pub iac_type: String,
    pub deploy_command: String,
    pub stacks: Option<String>,
    pub deployed_by: String,
    pub agent_id: Option<String>,
    pub deployed_at: String,
    pub ttl_expires_at: String,
    pub status: String,
    pub destroyed_at: Option<String>,
    pub metadata: Option<String>,
}

/// Result of staging resource cleanup operation
#[derive(Debug, Clone)]
pub struct StagingCleanupResult {
    /// Number of expired locks deleted
    pub locks_cleaned: usize,
    /// Number of orphaned queue entries deleted
    pub queue_entries_cleaned: usize,
    /// Number of deployments marked as expired
    pub deployments_marked_expired: usize,
    /// Deployments that are expired but not yet destroyed
    pub expired_deployments_pending_destroy: Vec<ExpiredDeploymentInfo>,
}

/// Info about an expired deployment that needs to be destroyed
#[derive(Debug, Clone)]
pub struct ExpiredDeploymentInfo {
    pub name: String,
    pub iac_type: String,
    pub deploy_command: String,
    pub stacks: Option<String>,
}

// ==================== Semantic Index Types (RFC 0010) ====================

/// Current prompt version for indexing
pub const INDEX_PROMPT_VERSION: i32 = 1;

/// An indexed file entry
#[derive(Debug, Clone)]
pub struct FileIndexEntry {
    pub id: Option<i64>,
    pub realm: String,
    pub repo: String,
    pub file_path: String,
    pub file_hash: String,
    pub summary: Option<String>,
    pub relationships: Option<String>,
    pub indexed_at: Option<String>,
    pub prompt_version: i32,
}

impl FileIndexEntry {
    pub fn new(realm: &str, repo: &str, file_path: &str, file_hash: &str) -> Self {
        Self {
            id: None,
            realm: realm.to_string(),
            repo: repo.to_string(),
            file_path: file_path.to_string(),
            file_hash: file_hash.to_string(),
            summary: None,
            relationships: None,
            indexed_at: None,
            prompt_version: INDEX_PROMPT_VERSION,
        }
    }
}

/// A symbol within an indexed file
#[derive(Debug, Clone)]
pub struct SymbolIndexEntry {
    pub id: Option<i64>,
    pub file_id: i64,
    pub name: String,
    pub kind: String,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    pub description: Option<String>,
}

/// Index status summary
#[derive(Debug, Clone)]
pub struct IndexStatus {
    pub total_files: usize,
    pub indexed_files: usize,
    pub stale_files: usize,
    pub unindexed_files: usize,
    pub stale_paths: Vec<String>,
    pub unindexed_paths: Vec<String>,
}

/// Search result from the semantic index
#[derive(Debug, Clone)]
pub struct IndexSearchResult {
    pub file_entry: FileIndexEntry,
    pub score: f64,
    pub matched_symbols: Vec<SymbolIndexEntry>,
}

/// Store errors - in Blue's voice
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Can't find '{0}'. Check the name's spelled right?")]
    NotFound(String),

    #[error("Database hiccup: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("'{0}' already exists. Want to update it instead?")]
    AlreadyExists(String),

    #[error("Can't do that: {0}")]
    InvalidOperation(String),
}

/// Check if an error is a busy/locked error
fn is_busy_error(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseBusy,
                ..
            },
            _
        ) | rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseLocked,
                ..
            },
            _
        )
    )
}

/// SQLite-based document store
pub struct DocumentStore {
    conn: Connection,
}

impl std::fmt::Debug for DocumentStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentStore")
            .field("conn", &"<Connection>")
            .finish()
    }
}

impl DocumentStore {
    /// Open or create a document store
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        info!("Opening Blue's document store at {:?}", path);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(path)?;

        // Configure for concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let store = Self { conn };
        store.init_schema()?;

        Ok(store)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let store = Self { conn };
        store.init_schema()?;

        Ok(store)
    }

    /// Get a reference to the underlying connection
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<(), StoreError> {
        let version: Option<i32> = self
            .conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();

        match version {
            None => {
                debug!("Setting up Blue's database (version {})", SCHEMA_VERSION);
                self.conn.execute_batch(SCHEMA)?;
                self.conn.execute_batch(FTS5_SCHEMA)?;
                self.conn.execute_batch(FILE_INDEX_FTS5_SCHEMA)?;
                self.conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![SCHEMA_VERSION],
                )?;
            }
            Some(v) if v == SCHEMA_VERSION => {
                debug!("Database is up to date (version {})", v);
            }
            Some(v) if v < SCHEMA_VERSION => {
                info!("Migrating database from version {} to {}", v, SCHEMA_VERSION);
                self.run_migrations(v)?;
            }
            Some(v) => {
                warn!(
                    "Schema version {} found, expected {}. Migrations may be needed.",
                    v, SCHEMA_VERSION
                );
            }
        }

        Ok(())
    }

    /// Run migrations from old version to current
    fn run_migrations(&self, from_version: i32) -> Result<(), StoreError> {
        // Migration from v2 to v3: Add deleted_at column
        if from_version < 3 {
            debug!("Adding deleted_at column to documents table");
            // Check if column exists first
            let has_column: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('documents') WHERE name = 'deleted_at'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            )?;

            if !has_column {
                self.conn.execute(
                    "ALTER TABLE documents ADD COLUMN deleted_at TEXT",
                    [],
                )?;
                self.conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_documents_deleted ON documents(deleted_at) WHERE deleted_at IS NOT NULL",
                    [],
                )?;
            }
        }

        // Migration from v3 to v4: Add semantic index tables (RFC 0010)
        if from_version < 4 {
            debug!("Adding semantic index tables (RFC 0010)");

            // Create file_index table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS file_index (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    realm TEXT NOT NULL,
                    repo TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_hash TEXT NOT NULL,
                    summary TEXT,
                    relationships TEXT,
                    indexed_at TEXT NOT NULL,
                    prompt_version INTEGER DEFAULT 1,
                    embedding BLOB,
                    UNIQUE(realm, repo, file_path)
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_file_index_realm ON file_index(realm)",
                [],
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_file_index_repo ON file_index(realm, repo)",
                [],
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_file_index_hash ON file_index(file_hash)",
                [],
            )?;

            // Create symbol_index table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS symbol_index (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    start_line INTEGER,
                    end_line INTEGER,
                    description TEXT,
                    FOREIGN KEY (file_id) REFERENCES file_index(id) ON DELETE CASCADE
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_symbol_index_file ON symbol_index(file_id)",
                [],
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_symbol_index_name ON symbol_index(name)",
                [],
            )?;

            // Create FTS5 tables for semantic search
            self.conn.execute_batch(FILE_INDEX_FTS5_SCHEMA)?;
        }

        // Update schema version
        self.conn.execute(
            "UPDATE schema_version SET version = ?1",
            params![SCHEMA_VERSION],
        )?;

        Ok(())
    }

    /// Execute with retry on busy
    fn with_retry<F, T>(&self, f: F) -> Result<T, StoreError>
    where
        F: Fn() -> Result<T, StoreError>,
    {
        let mut attempts = 0;
        loop {
            match f() {
                Ok(result) => return Ok(result),
                Err(StoreError::Database(ref e)) if is_busy_error(e) && attempts < 3 => {
                    attempts += 1;
                    let delay = Duration::from_millis(100 * attempts as u64);
                    debug!("Database busy, retrying in {:?}", delay);
                    thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Begin a write transaction
    pub fn begin_write(&mut self) -> Result<Transaction<'_>, StoreError> {
        Ok(self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }

    // ==================== Document Operations ====================

    /// Add a new document
    pub fn add_document(&self, doc: &Document) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO documents (doc_type, number, title, status, file_path, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    doc.doc_type.as_str(),
                    doc.number,
                    doc.title,
                    doc.status,
                    doc.file_path,
                    now,
                    now,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get a document by type and title
    pub fn get_document(&self, doc_type: DocType, title: &str) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                 FROM documents WHERE doc_type = ?1 AND title = ?2 AND deleted_at IS NULL",
                params![doc_type.as_str(), title],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(title.to_string()),
                e => StoreError::Database(e),
            })
    }

    /// Get a document by ID (including soft-deleted)
    pub fn get_document_by_id(&self, id: i64) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                 FROM documents WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StoreError::NotFound(format!("document #{}", id))
                }
                e => StoreError::Database(e),
            })
    }

    /// Get a document by number
    pub fn get_document_by_number(
        &self,
        doc_type: DocType,
        number: i32,
    ) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                 FROM documents WHERE doc_type = ?1 AND number = ?2 AND deleted_at IS NULL",
                params![doc_type.as_str(), number],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StoreError::NotFound(format!("{} #{}", doc_type.as_str(), number))
                }
                e => StoreError::Database(e),
            })
    }

    /// Find a document using flexible matching
    pub fn find_document(&self, doc_type: DocType, query: &str) -> Result<Document, StoreError> {
        // Try exact match first
        if let Ok(doc) = self.get_document(doc_type, query) {
            return Ok(doc);
        }

        // Try number match
        let trimmed = query.trim_start_matches('0');
        if let Ok(num) = if trimmed.is_empty() {
            "0".parse()
        } else {
            trimmed.parse::<i32>()
        } {
            if let Ok(doc) = self.get_document_by_number(doc_type, num) {
                return Ok(doc);
            }
        }

        // Try substring match
        let pattern = format!("%{}%", query.to_lowercase());
        if let Ok(doc) = self.conn.query_row(
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
             FROM documents WHERE doc_type = ?1 AND LOWER(title) LIKE ?2 AND deleted_at IS NULL
             ORDER BY LENGTH(title) ASC LIMIT 1",
            params![doc_type.as_str(), pattern],
            |row| {
                Ok(Document {
                    id: Some(row.get(0)?),
                    doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                    number: row.get(2)?,
                    title: row.get(3)?,
                    status: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    deleted_at: row.get(8)?,
                })
            },
        ) {
            return Ok(doc);
        }

        Err(StoreError::NotFound(format!(
            "{} matching '{}'",
            doc_type.as_str(),
            query
        )))
    }

    /// Update a document's status
    pub fn update_document_status(
        &self,
        doc_type: DocType,
        title: &str,
        status: &str,
    ) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET status = ?1, updated_at = ?2 WHERE doc_type = ?3 AND title = ?4",
                params![status, now, doc_type.as_str(), title],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(title.to_string()));
            }
            Ok(())
        })
    }

    /// Update a document
    pub fn update_document(&self, doc: &Document) -> Result<(), StoreError> {
        let id = doc
            .id
            .ok_or_else(|| StoreError::InvalidOperation("Document has no ID".to_string()))?;

        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET doc_type = ?1, number = ?2, title = ?3, status = ?4,
                 file_path = ?5, updated_at = ?6 WHERE id = ?7",
                params![
                    doc.doc_type.as_str(),
                    doc.number,
                    doc.title,
                    doc.status,
                    doc.file_path,
                    now,
                    id
                ],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!("document #{}", id)));
            }
            Ok(())
        })
    }

    /// List all documents of a given type (excludes soft-deleted)
    pub fn list_documents(&self, doc_type: DocType) -> Result<Vec<Document>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
             FROM documents WHERE doc_type = ?1 AND deleted_at IS NULL ORDER BY number DESC, title ASC",
        )?;

        let rows = stmt.query_map(params![doc_type.as_str()], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// List documents by status (excludes soft-deleted)
    pub fn list_documents_by_status(
        &self,
        doc_type: DocType,
        status: &str,
    ) -> Result<Vec<Document>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
             FROM documents WHERE doc_type = ?1 AND status = ?2 AND deleted_at IS NULL ORDER BY number DESC, title ASC",
        )?;

        let rows = stmt.query_map(params![doc_type.as_str(), status], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Delete a document permanently
    pub fn delete_document(&self, doc_type: DocType, title: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let deleted = self.conn.execute(
                "DELETE FROM documents WHERE doc_type = ?1 AND title = ?2",
                params![doc_type.as_str(), title],
            )?;
            if deleted == 0 {
                return Err(StoreError::NotFound(title.to_string()));
            }
            Ok(())
        })
    }

    /// Soft-delete a document (set deleted_at timestamp)
    pub fn soft_delete_document(&self, doc_type: DocType, title: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET deleted_at = ?1, updated_at = ?1
                 WHERE doc_type = ?2 AND title = ?3 AND deleted_at IS NULL",
                params![now, doc_type.as_str(), title],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(title.to_string()));
            }
            Ok(())
        })
    }

    /// Restore a soft-deleted document
    pub fn restore_document(&self, doc_type: DocType, title: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET deleted_at = NULL, updated_at = ?1
                 WHERE doc_type = ?2 AND title = ?3 AND deleted_at IS NOT NULL",
                params![now, doc_type.as_str(), title],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!(
                    "soft-deleted {} '{}'",
                    doc_type.as_str(),
                    title
                )));
            }
            Ok(())
        })
    }

    /// Get a soft-deleted document by type and title
    pub fn get_deleted_document(&self, doc_type: DocType, title: &str) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                 FROM documents WHERE doc_type = ?1 AND title = ?2 AND deleted_at IS NOT NULL",
                params![doc_type.as_str(), title],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(format!(
                    "soft-deleted {} '{}'",
                    doc_type.as_str(),
                    title
                )),
                e => StoreError::Database(e),
            })
    }

    /// List soft-deleted documents
    pub fn list_deleted_documents(&self, doc_type: Option<DocType>) -> Result<Vec<Document>, StoreError> {
        let query = match doc_type {
            Some(dt) => format!(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                 FROM documents WHERE doc_type = '{}' AND deleted_at IS NOT NULL
                 ORDER BY deleted_at DESC",
                dt.as_str()
            ),
            None => "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at
                     FROM documents WHERE deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC".to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Permanently delete documents that have been soft-deleted for more than N days
    pub fn purge_old_deleted_documents(&self, days: i64) -> Result<usize, StoreError> {
        self.with_retry(|| {
            let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
            let cutoff_str = cutoff.to_rfc3339();

            let deleted = self.conn.execute(
                "DELETE FROM documents WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
                params![cutoff_str],
            )?;

            Ok(deleted)
        })
    }

    /// Check if a document has ADR dependents (documents that reference it via rfc_to_adr link)
    pub fn has_adr_dependents(&self, document_id: i64) -> Result<Vec<Document>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at
             FROM documents d
             JOIN document_links l ON l.source_id = d.id
             WHERE l.target_id = ?1 AND l.link_type = 'rfc_to_adr' AND d.deleted_at IS NULL",
        )?;

        let rows = stmt.query_map(params![document_id], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Get the next document number for a type
    pub fn next_number(&self, doc_type: DocType) -> Result<i32, StoreError> {
        let max: Option<i32> = self.conn.query_row(
            "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
            params![doc_type.as_str()],
            |row| row.get(0),
        )?;
        Ok(max.unwrap_or(0) + 1)
    }

    // ==================== Link Operations ====================

    /// Link two documents
    pub fn link_documents(
        &self,
        source_id: i64,
        target_id: i64,
        link_type: LinkType,
    ) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT OR IGNORE INTO document_links (source_id, target_id, link_type, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![source_id, target_id, link_type.as_str(), now],
            )?;
            Ok(())
        })
    }

    /// Get linked documents (excludes soft-deleted)
    pub fn get_linked_documents(
        &self,
        source_id: i64,
        link_type: Option<LinkType>,
    ) -> Result<Vec<Document>, StoreError> {
        let query = match link_type {
            Some(lt) => format!(
                "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at
                 FROM documents d
                 JOIN document_links l ON l.target_id = d.id
                 WHERE l.source_id = ?1 AND l.link_type = '{}' AND d.deleted_at IS NULL",
                lt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at
                     FROM documents d
                     JOIN document_links l ON l.target_id = d.id
                     WHERE l.source_id = ?1 AND d.deleted_at IS NULL".to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(params![source_id], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    // ==================== Task Operations ====================

    /// Set tasks for a document (replaces existing)
    pub fn set_tasks(&self, document_id: i64, tasks: &[String]) -> Result<(), StoreError> {
        self.with_retry(|| {
            self.conn
                .execute("DELETE FROM tasks WHERE document_id = ?1", params![document_id])?;

            for (idx, desc) in tasks.iter().enumerate() {
                self.conn.execute(
                    "INSERT INTO tasks (document_id, task_index, description, completed)
                     VALUES (?1, ?2, ?3, 0)",
                    params![document_id, (idx + 1) as i32, desc],
                )?;
            }

            Ok(())
        })
    }

    /// Mark a task as complete
    pub fn complete_task(&self, document_id: i64, task_index: i32) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE tasks SET completed = 1, completed_at = ?1
                 WHERE document_id = ?2 AND task_index = ?3",
                params![now, document_id, task_index],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!(
                    "task {} in document #{}",
                    task_index, document_id
                )));
            }
            Ok(())
        })
    }

    /// Get task progress
    pub fn get_task_progress(&self, document_id: i64) -> Result<TaskProgress, StoreError> {
        let (total, completed): (i64, i64) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(completed), 0) FROM tasks WHERE document_id = ?1",
            params![document_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        let total = total as usize;
        let completed = completed as usize;
        let percentage = if total > 0 {
            (completed * 100) / total
        } else {
            0
        };

        Ok(TaskProgress {
            completed,
            total,
            percentage,
        })
    }

    /// Get all tasks for a document
    pub fn get_tasks(&self, document_id: i64) -> Result<Vec<Task>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, task_index, description, completed, completed_at
             FROM tasks WHERE document_id = ?1 ORDER BY task_index",
        )?;

        let rows = stmt.query_map(params![document_id], |row| {
            Ok(Task {
                id: Some(row.get(0)?),
                document_id: row.get(1)?,
                task_index: row.get(2)?,
                description: row.get(3)?,
                completed: row.get::<_, i32>(4)? != 0,
                completed_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    // ==================== Worktree Operations ====================

    /// Add a worktree for a document
    pub fn add_worktree(&self, worktree: &Worktree) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO worktrees (document_id, branch_name, worktree_path, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    worktree.document_id,
                    worktree.branch_name,
                    worktree.worktree_path,
                    now
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get worktree for a document
    pub fn get_worktree(&self, document_id: i64) -> Result<Option<Worktree>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, document_id, branch_name, worktree_path, created_at
                 FROM worktrees WHERE document_id = ?1",
                params![document_id],
                |row| {
                    Ok(Worktree {
                        id: Some(row.get(0)?),
                        document_id: row.get(1)?,
                        branch_name: row.get(2)?,
                        worktree_path: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Remove a worktree
    pub fn remove_worktree(&self, document_id: i64) -> Result<(), StoreError> {
        self.with_retry(|| {
            self.conn.execute(
                "DELETE FROM worktrees WHERE document_id = ?1",
                params![document_id],
            )?;
            Ok(())
        })
    }

    /// List all worktrees
    pub fn list_worktrees(&self) -> Result<Vec<Worktree>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, branch_name, worktree_path, created_at FROM worktrees",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Worktree {
                id: Some(row.get(0)?),
                document_id: row.get(1)?,
                branch_name: row.get(2)?,
                worktree_path: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    // ==================== Search Operations ====================

    /// Search documents using FTS5 (excludes soft-deleted)
    pub fn search_documents(
        &self,
        query: &str,
        doc_type: Option<DocType>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, StoreError> {
        let escaped = query.replace('"', "\"\"");
        let fts_query = format!("\"{}\"*", escaped);

        let sql = match doc_type {
            Some(dt) => format!(
                "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path,
                        d.created_at, d.updated_at, d.deleted_at, bm25(documents_fts) as score
                 FROM documents_fts fts
                 JOIN documents d ON d.id = fts.rowid
                 WHERE documents_fts MATCH ?1 AND d.doc_type = '{}' AND d.deleted_at IS NULL
                 ORDER BY score
                 LIMIT ?2",
                dt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path,
                            d.created_at, d.updated_at, d.deleted_at, bm25(documents_fts) as score
                     FROM documents_fts fts
                     JOIN documents d ON d.id = fts.rowid
                     WHERE documents_fts MATCH ?1 AND d.deleted_at IS NULL
                     ORDER BY score
                     LIMIT ?2"
                .to_string(),
        };

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![fts_query, limit as i32], |row| {
            Ok(SearchResult {
                document: Document {
                    id: Some(row.get(0)?),
                    doc_type: DocType::from_str(row.get::<_, String>(1)?.as_str()).unwrap(),
                    number: row.get(2)?,
                    title: row.get(3)?,
                    status: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    deleted_at: row.get(8)?,
                },
                score: row.get(9)?,
                snippet: None,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    // ==================== Session Operations ====================

    /// Start or update a session
    pub fn upsert_session(&self, session: &Session) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();

            // Try to get existing session
            let existing: Option<i64> = self.conn
                .query_row(
                    "SELECT id FROM sessions WHERE rfc_title = ?1 AND ended_at IS NULL",
                    params![session.rfc_title],
                    |row| row.get(0),
                )
                .optional()?;

            match existing {
                Some(id) => {
                    // Update heartbeat
                    self.conn.execute(
                        "UPDATE sessions SET last_heartbeat = ?1, session_type = ?2 WHERE id = ?3",
                        params![now, session.session_type.as_str(), id],
                    )?;
                    Ok(id)
                }
                None => {
                    // Create new session
                    self.conn.execute(
                        "INSERT INTO sessions (rfc_title, session_type, started_at, last_heartbeat)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            session.rfc_title,
                            session.session_type.as_str(),
                            now,
                            now
                        ],
                    )?;
                    Ok(self.conn.last_insert_rowid())
                }
            }
        })
    }

    /// End a session
    pub fn end_session(&self, rfc_title: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE sessions SET ended_at = ?1 WHERE rfc_title = ?2 AND ended_at IS NULL",
                params![now, rfc_title],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!("active session for '{}'", rfc_title)));
            }
            Ok(())
        })
    }

    /// Get active session for an RFC
    pub fn get_active_session(&self, rfc_title: &str) -> Result<Option<Session>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, rfc_title, session_type, started_at, last_heartbeat, ended_at
                 FROM sessions WHERE rfc_title = ?1 AND ended_at IS NULL",
                params![rfc_title],
                |row| {
                    Ok(Session {
                        id: Some(row.get(0)?),
                        rfc_title: row.get(1)?,
                        session_type: SessionType::from_str(&row.get::<_, String>(2)?).unwrap_or(SessionType::Implementation),
                        started_at: row.get(3)?,
                        last_heartbeat: row.get(4)?,
                        ended_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// List all active sessions
    pub fn list_active_sessions(&self) -> Result<Vec<Session>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, rfc_title, session_type, started_at, last_heartbeat, ended_at
             FROM sessions WHERE ended_at IS NULL ORDER BY started_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Session {
                id: Some(row.get(0)?),
                rfc_title: row.get(1)?,
                session_type: SessionType::from_str(&row.get::<_, String>(2)?).unwrap_or(SessionType::Implementation),
                started_at: row.get(3)?,
                last_heartbeat: row.get(4)?,
                ended_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Clean up stale sessions (no heartbeat in 5+ minutes)
    pub fn cleanup_stale_sessions(&self, timeout_minutes: i64) -> Result<usize, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now();
            let cutoff = now - chrono::Duration::minutes(timeout_minutes);
            let cutoff_str = cutoff.to_rfc3339();

            let count = self.conn.execute(
                "UPDATE sessions SET ended_at = ?1
                 WHERE ended_at IS NULL AND last_heartbeat < ?2",
                params![now.to_rfc3339(), cutoff_str],
            )?;
            Ok(count)
        })
    }

    // ==================== Reminder Operations ====================

    /// Add a reminder
    pub fn add_reminder(&self, reminder: &Reminder) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO reminders (title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    reminder.title,
                    reminder.context,
                    reminder.gate,
                    reminder.due_date,
                    reminder.snooze_until,
                    reminder.status.as_str(),
                    reminder.linked_doc_id,
                    now,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get a reminder by ID
    pub fn get_reminder(&self, id: i64) -> Result<Reminder, StoreError> {
        self.conn
            .query_row(
                "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Reminder {
                        id: Some(row.get(0)?),
                        title: row.get(1)?,
                        context: row.get(2)?,
                        gate: row.get(3)?,
                        due_date: row.get(4)?,
                        snooze_until: row.get(5)?,
                        status: ReminderStatus::from_str(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
                        linked_doc_id: row.get(7)?,
                        created_at: row.get(8)?,
                        cleared_at: row.get(9)?,
                        resolution: row.get(10)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(format!("reminder #{}", id)),
                e => StoreError::Database(e),
            })
    }

    /// Find reminder by title (partial match)
    pub fn find_reminder(&self, title: &str) -> Result<Reminder, StoreError> {
        // Try exact match first
        if let Ok(reminder) = self.conn.query_row(
            "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
             FROM reminders WHERE title = ?1 AND status != 'cleared'",
            params![title],
            |row| {
                Ok(Reminder {
                    id: Some(row.get(0)?),
                    title: row.get(1)?,
                    context: row.get(2)?,
                    gate: row.get(3)?,
                    due_date: row.get(4)?,
                    snooze_until: row.get(5)?,
                    status: ReminderStatus::from_str(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
                    linked_doc_id: row.get(7)?,
                    created_at: row.get(8)?,
                    cleared_at: row.get(9)?,
                    resolution: row.get(10)?,
                })
            },
        ) {
            return Ok(reminder);
        }

        // Try partial match
        let pattern = format!("%{}%", title.to_lowercase());
        self.conn
            .query_row(
                "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders WHERE LOWER(title) LIKE ?1 AND status != 'cleared'
                 ORDER BY LENGTH(title) ASC LIMIT 1",
                params![pattern],
                |row| {
                    Ok(Reminder {
                        id: Some(row.get(0)?),
                        title: row.get(1)?,
                        context: row.get(2)?,
                        gate: row.get(3)?,
                        due_date: row.get(4)?,
                        snooze_until: row.get(5)?,
                        status: ReminderStatus::from_str(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
                        linked_doc_id: row.get(7)?,
                        created_at: row.get(8)?,
                        cleared_at: row.get(9)?,
                        resolution: row.get(10)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(format!("reminder matching '{}'", title)),
                e => StoreError::Database(e),
            })
    }

    /// List reminders by status
    pub fn list_reminders(&self, status: Option<ReminderStatus>, include_future: bool) -> Result<Vec<Reminder>, StoreError> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        let query = match (status, include_future) {
            (Some(s), true) => format!(
                "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders WHERE status = '{}' ORDER BY due_date ASC, created_at ASC",
                s.as_str()
            ),
            (Some(s), false) => format!(
                "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders WHERE status = '{}' AND (snooze_until IS NULL OR snooze_until <= '{}')
                 ORDER BY due_date ASC, created_at ASC",
                s.as_str(), today
            ),
            (None, true) => "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders ORDER BY due_date ASC, created_at ASC".to_string(),
            (None, false) => format!(
                "SELECT id, title, context, gate, due_date, snooze_until, status, linked_doc_id, created_at, cleared_at, resolution
                 FROM reminders WHERE snooze_until IS NULL OR snooze_until <= '{}'
                 ORDER BY due_date ASC, created_at ASC",
                today
            ),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            Ok(Reminder {
                id: Some(row.get(0)?),
                title: row.get(1)?,
                context: row.get(2)?,
                gate: row.get(3)?,
                due_date: row.get(4)?,
                snooze_until: row.get(5)?,
                status: ReminderStatus::from_str(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
                linked_doc_id: row.get(7)?,
                created_at: row.get(8)?,
                cleared_at: row.get(9)?,
                resolution: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Snooze a reminder
    pub fn snooze_reminder(&self, id: i64, until: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let updated = self.conn.execute(
                "UPDATE reminders SET snooze_until = ?1, status = 'snoozed' WHERE id = ?2",
                params![until, id],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!("reminder #{}", id)));
            }
            Ok(())
        })
    }

    /// Clear a reminder
    pub fn clear_reminder(&self, id: i64, resolution: Option<&str>) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE reminders SET status = 'cleared', cleared_at = ?1, resolution = ?2 WHERE id = ?3",
                params![now, resolution, id],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!("reminder #{}", id)));
            }
            Ok(())
        })
    }

    // ==================== Staging Lock Operations ====================

    /// Acquire a staging lock or join queue
    pub fn acquire_staging_lock(
        &self,
        resource: &str,
        locked_by: &str,
        agent_id: Option<&str>,
        duration_minutes: i64,
    ) -> Result<StagingLockResult, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();
            let expires_at = now + chrono::Duration::minutes(duration_minutes);
            let expires_str = expires_at.to_rfc3339();

            // First, clean up expired locks
            self.conn.execute(
                "DELETE FROM staging_locks WHERE expires_at < ?1",
                params![now_str],
            )?;

            // Check if lock exists
            let existing: Option<(String, String)> = self.conn
                .query_row(
                    "SELECT locked_by, expires_at FROM staging_locks WHERE resource = ?1",
                    params![resource],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            match existing {
                Some((holder, holder_expires)) => {
                    // Lock exists, add to queue
                    self.conn.execute(
                        "INSERT INTO staging_lock_queue (resource, requester, agent_id, requested_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![resource, locked_by, agent_id, now_str],
                    )?;

                    // Get queue position
                    let position: i64 = self.conn.query_row(
                        "SELECT COUNT(*) FROM staging_lock_queue WHERE resource = ?1",
                        params![resource],
                        |row| row.get(0),
                    )?;

                    Ok(StagingLockResult::Queued {
                        position: position as usize,
                        current_holder: holder,
                        expires_at: holder_expires,
                    })
                }
                None => {
                    // No lock, acquire it
                    self.conn.execute(
                        "INSERT INTO staging_locks (resource, locked_by, agent_id, locked_at, expires_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![resource, locked_by, agent_id, now_str, expires_str],
                    )?;

                    Ok(StagingLockResult::Acquired {
                        expires_at: expires_str,
                    })
                }
            }
        })
    }

    /// Release a staging lock
    pub fn release_staging_lock(&self, resource: &str, locked_by: &str) -> Result<Option<String>, StoreError> {
        self.with_retry(|| {
            // Verify the lock is held by the requester
            let holder: Option<String> = self.conn
                .query_row(
                    "SELECT locked_by FROM staging_locks WHERE resource = ?1",
                    params![resource],
                    |row| row.get(0),
                )
                .optional()?;

            match holder {
                Some(h) if h == locked_by => {
                    // Get next in queue BEFORE deleting lock (CASCADE would remove queue entries)
                    let next: Option<String> = self.conn
                        .query_row(
                            "SELECT requester FROM staging_lock_queue WHERE resource = ?1 ORDER BY requested_at ASC LIMIT 1",
                            params![resource],
                            |row| row.get(0),
                        )
                        .optional()?;

                    // Release the lock (CASCADE will clean up queue)
                    self.conn.execute(
                        "DELETE FROM staging_locks WHERE resource = ?1",
                        params![resource],
                    )?;

                    Ok(next)
                }
                Some(_) => Err(StoreError::InvalidOperation(format!(
                    "Lock for '{}' is not held by '{}'",
                    resource, locked_by
                ))),
                None => Err(StoreError::NotFound(format!("lock for '{}'", resource))),
            }
        })
    }

    /// Get current staging lock for a resource
    pub fn get_staging_lock(&self, resource: &str) -> Result<Option<StagingLock>, StoreError> {
        // First clean up expired locks
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "DELETE FROM staging_locks WHERE expires_at < ?1",
            params![now],
        )?;

        self.conn
            .query_row(
                "SELECT id, resource, locked_by, agent_id, locked_at, expires_at
                 FROM staging_locks WHERE resource = ?1",
                params![resource],
                |row| {
                    Ok(StagingLock {
                        id: Some(row.get(0)?),
                        resource: row.get(1)?,
                        locked_by: row.get(2)?,
                        agent_id: row.get(3)?,
                        locked_at: row.get(4)?,
                        expires_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Get queue for a staging lock
    pub fn get_staging_lock_queue(&self, resource: &str) -> Result<Vec<StagingLockQueueEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, resource, requester, agent_id, requested_at
             FROM staging_lock_queue WHERE resource = ?1 ORDER BY requested_at ASC",
        )?;

        let rows = stmt.query_map(params![resource], |row| {
            Ok(StagingLockQueueEntry {
                id: Some(row.get(0)?),
                resource: row.get(1)?,
                requester: row.get(2)?,
                agent_id: row.get(3)?,
                requested_at: row.get(4)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// List all active staging locks
    pub fn list_staging_locks(&self) -> Result<Vec<StagingLock>, StoreError> {
        // First clean up expired locks
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "DELETE FROM staging_locks WHERE expires_at < ?1",
            params![now],
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT id, resource, locked_by, agent_id, locked_at, expires_at
             FROM staging_locks ORDER BY locked_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(StagingLock {
                id: Some(row.get(0)?),
                resource: row.get(1)?,
                locked_by: row.get(2)?,
                agent_id: row.get(3)?,
                locked_at: row.get(4)?,
                expires_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Clean up expired staging locks and orphaned queue entries
    pub fn cleanup_expired_staging(&self) -> Result<(usize, usize), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();

            // Clean expired locks
            let locks_cleaned = self.conn.execute(
                "DELETE FROM staging_locks WHERE expires_at < ?1",
                params![now],
            )?;

            // Clean orphaned queue entries (for resources with no lock)
            let queue_cleaned = self.conn.execute(
                "DELETE FROM staging_lock_queue WHERE resource NOT IN (SELECT resource FROM staging_locks)",
                [],
            )?;

            Ok((locks_cleaned, queue_cleaned))
        })
    }

    // ===== Staging Deployments =====

    /// Record a new staging deployment
    pub fn record_staging_deployment(
        &self,
        name: &str,
        iac_type: &str,
        deploy_command: &str,
        stacks: Option<&str>,
        deployed_by: &str,
        agent_id: Option<&str>,
        ttl_hours: u32,
        metadata: Option<&str>,
    ) -> Result<StagingDeployment, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now();
            let ttl_expires = now + chrono::Duration::hours(ttl_hours as i64);

            self.conn.execute(
                "INSERT OR REPLACE INTO staging_deployments
                 (name, iac_type, deploy_command, stacks, deployed_by, agent_id, deployed_at, ttl_expires_at, status, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'deployed', ?9)",
                params![
                    name,
                    iac_type,
                    deploy_command,
                    stacks,
                    deployed_by,
                    agent_id,
                    now.to_rfc3339(),
                    ttl_expires.to_rfc3339(),
                    metadata
                ],
            )?;

            Ok(StagingDeployment {
                id: Some(self.conn.last_insert_rowid()),
                name: name.to_string(),
                iac_type: iac_type.to_string(),
                deploy_command: deploy_command.to_string(),
                stacks: stacks.map(|s| s.to_string()),
                deployed_by: deployed_by.to_string(),
                agent_id: agent_id.map(|s| s.to_string()),
                deployed_at: now.to_rfc3339(),
                ttl_expires_at: ttl_expires.to_rfc3339(),
                status: "deployed".to_string(),
                destroyed_at: None,
                metadata: metadata.map(|s| s.to_string()),
            })
        })
    }

    /// List all staging deployments, optionally filtered by status
    pub fn list_staging_deployments(
        &self,
        status: Option<&str>,
    ) -> Result<Vec<StagingDeployment>, StoreError> {
        let query = if status.is_some() {
            "SELECT id, name, iac_type, deploy_command, stacks, deployed_by, agent_id,
                    deployed_at, ttl_expires_at, status, destroyed_at, metadata
             FROM staging_deployments WHERE status = ?1 ORDER BY deployed_at DESC"
        } else {
            "SELECT id, name, iac_type, deploy_command, stacks, deployed_by, agent_id,
                    deployed_at, ttl_expires_at, status, destroyed_at, metadata
             FROM staging_deployments ORDER BY deployed_at DESC"
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = if let Some(s) = status {
            stmt.query_map(params![s], Self::map_staging_deployment)?
        } else {
            stmt.query_map([], Self::map_staging_deployment)?
        };

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Get expired deployments that need cleanup
    pub fn get_expired_deployments(&self) -> Result<Vec<StagingDeployment>, StoreError> {
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = self.conn.prepare(
            "SELECT id, name, iac_type, deploy_command, stacks, deployed_by, agent_id,
                    deployed_at, ttl_expires_at, status, destroyed_at, metadata
             FROM staging_deployments
             WHERE status = 'deployed' AND ttl_expires_at < ?1
             ORDER BY ttl_expires_at",
        )?;

        let rows = stmt.query_map(params![now], Self::map_staging_deployment)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Mark a deployment as destroyed
    pub fn mark_deployment_destroyed(&self, name: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();

            let updated = self.conn.execute(
                "UPDATE staging_deployments SET status = 'destroyed', destroyed_at = ?1
                 WHERE name = ?2 AND status = 'deployed'",
                params![now, name],
            )?;

            if updated == 0 {
                return Err(StoreError::NotFound(format!(
                    "Deployment '{}' not found or already destroyed",
                    name
                )));
            }

            Ok(())
        })
    }

    /// Mark expired deployments as expired (for auto-cleanup tracking)
    pub fn mark_expired_deployments(&self) -> Result<Vec<StagingDeployment>, StoreError> {
        let expired = self.get_expired_deployments()?;

        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "UPDATE staging_deployments SET status = 'expired'
                 WHERE status = 'deployed' AND ttl_expires_at < ?1",
                params![now],
            )?;
            Ok(())
        })?;

        Ok(expired)
    }

    /// Get a specific deployment by name
    pub fn get_staging_deployment(
        &self,
        name: &str,
    ) -> Result<Option<StagingDeployment>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, name, iac_type, deploy_command, stacks, deployed_by, agent_id,
                    deployed_at, ttl_expires_at, status, destroyed_at, metadata
                 FROM staging_deployments WHERE name = ?1",
                params![name],
                Self::map_staging_deployment,
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Helper to map a row to StagingDeployment
    fn map_staging_deployment(row: &rusqlite::Row) -> rusqlite::Result<StagingDeployment> {
        Ok(StagingDeployment {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            iac_type: row.get(2)?,
            deploy_command: row.get(3)?,
            stacks: row.get(4)?,
            deployed_by: row.get(5)?,
            agent_id: row.get(6)?,
            deployed_at: row.get(7)?,
            ttl_expires_at: row.get(8)?,
            status: row.get(9)?,
            destroyed_at: row.get(10)?,
            metadata: row.get(11)?,
        })
    }

    /// Clean up all expired staging resources (locks, deployments, queue entries)
    pub fn cleanup_expired_staging_resources(&self) -> Result<StagingCleanupResult, StoreError> {
        let now = chrono::Utc::now().to_rfc3339();

        // Clean up expired locks
        let locks_cleaned = self.conn.execute(
            "DELETE FROM staging_locks WHERE expires_at < ?1",
            params![now],
        )?;

        // Clean up orphaned queue entries
        let queue_cleaned = self.conn.execute(
            "DELETE FROM staging_lock_queue
             WHERE resource NOT IN (SELECT resource FROM staging_locks)",
            [],
        )?;

        // Mark expired deployments
        let deployments_marked = self.conn.execute(
            "UPDATE staging_deployments SET status = 'expired'
             WHERE status = 'deployed' AND ttl_expires_at < ?1",
            params![now],
        )?;

        // Get list of expired deployments that need cleanup commands
        let mut stmt = self.conn.prepare(
            "SELECT name, iac_type, deploy_command, stacks
             FROM staging_deployments
             WHERE status = 'expired' AND destroyed_at IS NULL",
        )?;

        let expired_deployments: Vec<ExpiredDeploymentInfo> = stmt
            .query_map([], |row| {
                Ok(ExpiredDeploymentInfo {
                    name: row.get(0)?,
                    iac_type: row.get(1)?,
                    deploy_command: row.get(2)?,
                    stacks: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(StagingCleanupResult {
            locks_cleaned,
            queue_entries_cleaned: queue_cleaned,
            deployments_marked_expired: deployments_marked,
            expired_deployments_pending_destroy: expired_deployments,
        })
    }

    // ==================== Semantic Index Operations (RFC 0010) ====================

    /// Upsert a file index entry
    pub fn upsert_file_index(&self, entry: &FileIndexEntry) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();

            self.conn.execute(
                "INSERT INTO file_index (realm, repo, file_path, file_hash, summary, relationships, indexed_at, prompt_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(realm, repo, file_path) DO UPDATE SET
                     file_hash = excluded.file_hash,
                     summary = excluded.summary,
                     relationships = excluded.relationships,
                     indexed_at = excluded.indexed_at,
                     prompt_version = excluded.prompt_version",
                params![
                    entry.realm,
                    entry.repo,
                    entry.file_path,
                    entry.file_hash,
                    entry.summary,
                    entry.relationships,
                    now,
                    entry.prompt_version,
                ],
            )?;

            // Get the ID (either new or existing)
            let id: i64 = self.conn.query_row(
                "SELECT id FROM file_index WHERE realm = ?1 AND repo = ?2 AND file_path = ?3",
                params![entry.realm, entry.repo, entry.file_path],
                |row| row.get(0),
            )?;

            Ok(id)
        })
    }

    /// Get a file index entry
    pub fn get_file_index(&self, realm: &str, repo: &str, file_path: &str) -> Result<Option<FileIndexEntry>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, realm, repo, file_path, file_hash, summary, relationships, indexed_at, prompt_version
                 FROM file_index WHERE realm = ?1 AND repo = ?2 AND file_path = ?3",
                params![realm, repo, file_path],
                |row| {
                    Ok(FileIndexEntry {
                        id: Some(row.get(0)?),
                        realm: row.get(1)?,
                        repo: row.get(2)?,
                        file_path: row.get(3)?,
                        file_hash: row.get(4)?,
                        summary: row.get(5)?,
                        relationships: row.get(6)?,
                        indexed_at: row.get(7)?,
                        prompt_version: row.get(8)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Delete a file index entry and its symbols
    pub fn delete_file_index(&self, realm: &str, repo: &str, file_path: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            self.conn.execute(
                "DELETE FROM file_index WHERE realm = ?1 AND repo = ?2 AND file_path = ?3",
                params![realm, repo, file_path],
            )?;
            Ok(())
        })
    }

    /// Add symbols for a file (replaces existing)
    pub fn set_file_symbols(&self, file_id: i64, symbols: &[SymbolIndexEntry]) -> Result<(), StoreError> {
        self.with_retry(|| {
            // Delete existing symbols
            self.conn.execute(
                "DELETE FROM symbol_index WHERE file_id = ?1",
                params![file_id],
            )?;

            // Insert new symbols
            for symbol in symbols {
                self.conn.execute(
                    "INSERT INTO symbol_index (file_id, name, kind, start_line, end_line, description)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        file_id,
                        symbol.name,
                        symbol.kind,
                        symbol.start_line,
                        symbol.end_line,
                        symbol.description,
                    ],
                )?;
            }

            Ok(())
        })
    }

    /// Get symbols for a file
    pub fn get_file_symbols(&self, file_id: i64) -> Result<Vec<SymbolIndexEntry>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, start_line, end_line, description
             FROM symbol_index WHERE file_id = ?1 ORDER BY start_line",
        )?;

        let rows = stmt.query_map(params![file_id], |row| {
            Ok(SymbolIndexEntry {
                id: Some(row.get(0)?),
                file_id: row.get(1)?,
                name: row.get(2)?,
                kind: row.get(3)?,
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                description: row.get(6)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// List all indexed files in a realm/repo
    pub fn list_file_index(&self, realm: &str, repo: Option<&str>) -> Result<Vec<FileIndexEntry>, StoreError> {
        let query = match repo {
            Some(_) => "SELECT id, realm, repo, file_path, file_hash, summary, relationships, indexed_at, prompt_version
                        FROM file_index WHERE realm = ?1 AND repo = ?2 ORDER BY file_path",
            None => "SELECT id, realm, repo, file_path, file_hash, summary, relationships, indexed_at, prompt_version
                     FROM file_index WHERE realm = ?1 ORDER BY repo, file_path",
        };

        let mut stmt = self.conn.prepare(query)?;

        let rows = match repo {
            Some(r) => stmt.query_map(params![realm, r], Self::map_file_index_entry)?,
            None => stmt.query_map(params![realm], Self::map_file_index_entry)?,
        };

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Helper to map a row to FileIndexEntry
    fn map_file_index_entry(row: &rusqlite::Row) -> rusqlite::Result<FileIndexEntry> {
        Ok(FileIndexEntry {
            id: Some(row.get(0)?),
            realm: row.get(1)?,
            repo: row.get(2)?,
            file_path: row.get(3)?,
            file_hash: row.get(4)?,
            summary: row.get(5)?,
            relationships: row.get(6)?,
            indexed_at: row.get(7)?,
            prompt_version: row.get(8)?,
        })
    }

    /// Search the file index using FTS5
    pub fn search_file_index(
        &self,
        realm: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<IndexSearchResult>, StoreError> {
        let escaped = query.replace('"', "\"\"");
        let fts_query = format!("\"{}\"*", escaped);

        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.realm, f.repo, f.file_path, f.file_hash, f.summary, f.relationships,
                    f.indexed_at, f.prompt_version, bm25(file_index_fts) as score
             FROM file_index_fts fts
             JOIN file_index f ON f.id = fts.rowid
             WHERE file_index_fts MATCH ?1 AND f.realm = ?2
             ORDER BY score
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![fts_query, realm, limit as i32], |row| {
            Ok(IndexSearchResult {
                file_entry: FileIndexEntry {
                    id: Some(row.get(0)?),
                    realm: row.get(1)?,
                    repo: row.get(2)?,
                    file_path: row.get(3)?,
                    file_hash: row.get(4)?,
                    summary: row.get(5)?,
                    relationships: row.get(6)?,
                    indexed_at: row.get(7)?,
                    prompt_version: row.get(8)?,
                },
                score: row.get(9)?,
                matched_symbols: vec![],
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Search symbols using FTS5
    pub fn search_symbols(
        &self,
        realm: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(SymbolIndexEntry, FileIndexEntry)>, StoreError> {
        let escaped = query.replace('"', "\"\"");
        let fts_query = format!("\"{}\"*", escaped);

        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.file_id, s.name, s.kind, s.start_line, s.end_line, s.description,
                    f.id, f.realm, f.repo, f.file_path, f.file_hash, f.summary, f.relationships,
                    f.indexed_at, f.prompt_version
             FROM symbol_index_fts sfts
             JOIN symbol_index s ON s.id = sfts.rowid
             JOIN file_index f ON f.id = s.file_id
             WHERE symbol_index_fts MATCH ?1 AND f.realm = ?2
             ORDER BY bm25(symbol_index_fts)
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![fts_query, realm, limit as i32], |row| {
            Ok((
                SymbolIndexEntry {
                    id: Some(row.get(0)?),
                    file_id: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    start_line: row.get(4)?,
                    end_line: row.get(5)?,
                    description: row.get(6)?,
                },
                FileIndexEntry {
                    id: Some(row.get(7)?),
                    realm: row.get(8)?,
                    repo: row.get(9)?,
                    file_path: row.get(10)?,
                    file_hash: row.get(11)?,
                    summary: row.get(12)?,
                    relationships: row.get(13)?,
                    indexed_at: row.get(14)?,
                    prompt_version: row.get(15)?,
                },
            ))
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Get index statistics for a realm
    pub fn get_index_stats(&self, realm: &str) -> Result<(usize, usize), StoreError> {
        let file_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM file_index WHERE realm = ?1",
            params![realm],
            |row| row.get(0),
        )?;

        let symbol_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM symbol_index s
             JOIN file_index f ON f.id = s.file_id
             WHERE f.realm = ?1",
            params![realm],
            |row| row.get(0),
        )?;

        Ok((file_count as usize, symbol_count as usize))
    }

    /// Check if a file needs re-indexing (hash mismatch or prompt version outdated)
    pub fn is_file_stale(&self, realm: &str, repo: &str, file_path: &str, current_hash: &str) -> Result<bool, StoreError> {
        let result: Option<(String, i32)> = self.conn
            .query_row(
                "SELECT file_hash, prompt_version FROM file_index
                 WHERE realm = ?1 AND repo = ?2 AND file_path = ?3",
                params![realm, repo, file_path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match result {
            Some((hash, version)) => Ok(hash != current_hash || version < INDEX_PROMPT_VERSION),
            None => Ok(true), // Not indexed = stale
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_find_document() {
        let store = DocumentStore::open_in_memory().unwrap();

        let doc = Document::new(DocType::Rfc, "test-feature", "draft");
        let id = store.add_document(&doc).unwrap();

        let found = store.find_document(DocType::Rfc, "test-feature").unwrap();
        assert_eq!(found.id, Some(id));
        assert_eq!(found.title, "test-feature");
    }

    #[test]
    fn test_task_progress() {
        let store = DocumentStore::open_in_memory().unwrap();

        let doc = Document::new(DocType::Rfc, "task-test", "draft");
        let id = store.add_document(&doc).unwrap();

        store
            .set_tasks(id, &["Task 1".into(), "Task 2".into(), "Task 3".into()])
            .unwrap();

        let progress = store.get_task_progress(id).unwrap();
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.percentage, 0);

        store.complete_task(id, 1).unwrap();

        let progress = store.get_task_progress(id).unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.percentage, 33);
    }
}
