//! SQLite document store for Blue
//!
//! Persistence layer for RFCs, Spikes, ADRs, and other documents.

use std::path::Path;
use std::thread;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use tracing::{debug, info, warn};

/// Current schema version
const SCHEMA_VERSION: i32 = 2;

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
        UNIQUE(doc_type, title)
    );

    CREATE INDEX IF NOT EXISTS idx_documents_type ON documents(doc_type);
    CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(doc_type, status);

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

/// Document types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Rfc,
    Spike,
    Adr,
    Decision,
    Prd,
}

impl DocType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocType::Rfc => "rfc",
            DocType::Spike => "spike",
            DocType::Adr => "adr",
            DocType::Decision => "decision",
            DocType::Prd => "prd",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "rfc" => Some(DocType::Rfc),
            "spike" => Some(DocType::Spike),
            "adr" => Some(DocType::Adr),
            "decision" => Some(DocType::Decision),
            "prd" => Some(DocType::Prd),
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
    /// Generic reference
    References,
}

impl LinkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LinkType::SpikeToRfc => "spike_to_rfc",
            LinkType::RfcToAdr => "rfc_to_adr",
            LinkType::PrdToRfc => "prd_to_rfc",
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
        }
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
                self.conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    params![SCHEMA_VERSION],
                )?;
            }
            Some(v) if v == SCHEMA_VERSION => {
                debug!("Database is up to date (version {})", v);
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
                 FROM documents WHERE doc_type = ?1 AND title = ?2",
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
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::NotFound(title.to_string()),
                e => StoreError::Database(e),
            })
    }

    /// Get a document by ID
    pub fn get_document_by_id(&self, id: i64) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
                 FROM documents WHERE doc_type = ?1 AND number = ?2",
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
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
             FROM documents WHERE doc_type = ?1 AND LOWER(title) LIKE ?2
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

    /// List all documents of a given type
    pub fn list_documents(&self, doc_type: DocType) -> Result<Vec<Document>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
             FROM documents WHERE doc_type = ?1 ORDER BY number DESC, title ASC",
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
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// List documents by status
    pub fn list_documents_by_status(
        &self,
        doc_type: DocType,
        status: &str,
    ) -> Result<Vec<Document>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at
             FROM documents WHERE doc_type = ?1 AND status = ?2 ORDER BY number DESC, title ASC",
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
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Delete a document
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

    /// Get linked documents
    pub fn get_linked_documents(
        &self,
        source_id: i64,
        link_type: Option<LinkType>,
    ) -> Result<Vec<Document>, StoreError> {
        let query = match link_type {
            Some(lt) => format!(
                "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at
                 FROM documents d
                 JOIN document_links l ON l.target_id = d.id
                 WHERE l.source_id = ?1 AND l.link_type = '{}'",
                lt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at
                     FROM documents d
                     JOIN document_links l ON l.target_id = d.id
                     WHERE l.source_id = ?1".to_string(),
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

    /// Search documents using FTS5
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
                        d.created_at, d.updated_at, bm25(documents_fts) as score
                 FROM documents_fts fts
                 JOIN documents d ON d.id = fts.rowid
                 WHERE documents_fts MATCH ?1 AND d.doc_type = '{}'
                 ORDER BY score
                 LIMIT ?2",
                dt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path,
                            d.created_at, d.updated_at, bm25(documents_fts) as score
                     FROM documents_fts fts
                     JOIN documents d ON d.id = fts.rowid
                     WHERE documents_fts MATCH ?1
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
                },
                score: row.get(8)?,
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
