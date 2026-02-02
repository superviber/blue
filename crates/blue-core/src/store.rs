//! SQLite document store for Blue
//!
//! Persistence layer for RFCs, Spikes, ADRs, and other documents.

use std::path::Path;
use std::thread;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Sha256, Digest};
use tracing::{debug, info, warn};

/// Compute a SHA-256 hash of content for staleness detection (RFC 0018)
pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Current schema version
const SCHEMA_VERSION: i32 = 9;

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
        content_hash TEXT,
        indexed_at TEXT,
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

    -- Context injection audit log (RFC 0016)
    CREATE TABLE IF NOT EXISTS context_injections (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        timestamp TEXT NOT NULL,
        tier TEXT NOT NULL,
        source_uri TEXT NOT NULL,
        content_hash TEXT NOT NULL,
        token_count INTEGER
    );

    CREATE INDEX IF NOT EXISTS idx_context_injections_session ON context_injections(session_id);
    CREATE INDEX IF NOT EXISTS idx_context_injections_timestamp ON context_injections(timestamp);

    -- Relevance graph edges (RFC 0017)
    CREATE TABLE IF NOT EXISTS relevance_edges (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_uri TEXT NOT NULL,
        target_uri TEXT NOT NULL,
        edge_type TEXT NOT NULL,
        weight REAL DEFAULT 1.0,
        created_at TEXT NOT NULL,
        UNIQUE(source_uri, target_uri, edge_type)
    );

    CREATE INDEX IF NOT EXISTS idx_relevance_source ON relevance_edges(source_uri);
    CREATE INDEX IF NOT EXISTS idx_relevance_target ON relevance_edges(target_uri);

    -- Staleness tracking index for documents (RFC 0017)
    CREATE INDEX IF NOT EXISTS idx_documents_staleness ON documents(
        doc_type,
        updated_at
    ) WHERE deleted_at IS NULL;

    -- Plan cache for tracking plan file sync state (RFC 0017)
    CREATE TABLE IF NOT EXISTS plan_cache (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        document_id INTEGER NOT NULL UNIQUE,
        cache_mtime TEXT NOT NULL,
        FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_plan_cache_document ON plan_cache(document_id);
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

    pub fn parse(s: &str) -> Option<Self> {
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

    /// Subdirectory in .blue/docs/ (RFC 0018)
    pub fn subdir(&self) -> &'static str {
        match self {
            DocType::Rfc => "rfcs",
            DocType::Spike => "spikes",
            DocType::Adr => "adrs",
            DocType::Decision => "decisions",
            DocType::Prd => "prds",
            DocType::Postmortem => "postmortems",
            DocType::Runbook => "runbooks",
            DocType::Dialogue => "dialogues",
            DocType::Audit => "audits",
        }
    }
}

/// Convert a title to a kebab-case slug for filenames and matching (RFC 0022)
/// "Filesystem Authority" → "filesystem-authority"
/// "foo's bar!" → "foo-s-bar"
pub fn title_to_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Known status suffixes that can appear in filenames (RFC 0031)
const KNOWN_SUFFIXES: &[&str] = &[
    "done", "impl", "super", "accepted", "approved", "wip", "resolved",
    "closed", "pub", "archived", "draft", "open", "recorded", "active",
];

/// Map (DocType, status) → optional filename suffix (RFC 0031)
///
/// Returns `None` for the default/initial status of each doc type,
/// meaning no suffix should be appended.
pub fn status_suffix(doc_type: DocType, status: &str) -> Option<&'static str> {
    match (doc_type, status.to_lowercase().as_str()) {
        // Spike
        (DocType::Spike, "in-progress") => Some("wip"),
        (DocType::Spike, "complete") => Some("done"),
        (DocType::Spike, "resolved") => Some("resolved"),

        // RFC
        (DocType::Rfc, "draft") => Some("draft"),
        (DocType::Rfc, "accepted") => Some("accepted"),
        (DocType::Rfc, "in-progress") => Some("wip"),
        (DocType::Rfc, "implemented") => Some("impl"),
        (DocType::Rfc, "superseded") => Some("super"),

        // ADR
        (DocType::Adr, "accepted") => Some("accepted"),
        (DocType::Adr, "superseded") => Some("super"),

        // Decision
        (DocType::Decision, "recorded") => Some("recorded"),

        // PRD
        (DocType::Prd, "draft") => Some("draft"),
        (DocType::Prd, "approved") => Some("approved"),
        (DocType::Prd, "implemented") => Some("impl"),

        // Postmortem
        (DocType::Postmortem, "open") => Some("open"),
        (DocType::Postmortem, "closed") => Some("closed"),

        // Runbook
        (DocType::Runbook, "active") => Some("active"),
        (DocType::Runbook, "published") => Some("pub"),
        (DocType::Runbook, "archived") => Some("archived"),

        // Dialogue
        (DocType::Dialogue, "recorded") => Some("recorded"),
        (DocType::Dialogue, "published") => Some("pub"),

        // Audit
        (DocType::Audit, "in-progress") => Some("wip"),
        (DocType::Audit, "complete") => Some("done"),

        // Anything else: no suffix
        _ => None,
    }
}

/// Rebuild a filename with a new status suffix (RFC 0031)
///
/// Handles:
/// - Regular files: `spikes/2026-01-26T0856Z-slug.md` → `spikes/2026-01-26T0856Z-slug.done.md`
/// - Dialogue double extension: `dialogues/2026-01-26T0856Z-slug.dialogue.md` → `dialogues/2026-01-26T0856Z-slug.dialogue.done.md`
/// - Stripping old suffix before adding new one
pub fn rebuild_filename(old_path: &str, doc_type: DocType, new_status: &str) -> String {
    let suffix = status_suffix(doc_type, new_status);

    // Detect dialogue double extension
    let is_dialogue = old_path.ends_with(".dialogue.md")
        || KNOWN_SUFFIXES.iter().any(|s| old_path.ends_with(&format!(".dialogue.{}.md", s)));

    if is_dialogue {
        // Strip old suffix: foo.dialogue.done.md → foo.dialogue.md
        let base = strip_dialogue_suffix(old_path);
        match suffix {
            Some(s) => {
                // foo.dialogue.md → foo.dialogue.{suffix}.md
                let without_md = base.strip_suffix(".dialogue.md").unwrap_or(&base);
                format!("{}.dialogue.{}.md", without_md, s)
            }
            None => base,
        }
    } else {
        // Strip old suffix: foo.done.md → foo.md
        let base = strip_regular_suffix(old_path);
        match suffix {
            Some(s) => {
                let without_md = base.strip_suffix(".md").unwrap_or(&base);
                format!("{}.{}.md", without_md, s)
            }
            None => base,
        }
    }
}

/// Strip a known status suffix from a dialogue filename
/// `foo.dialogue.done.md` → `foo.dialogue.md`
fn strip_dialogue_suffix(path: &str) -> String {
    for suffix in KNOWN_SUFFIXES {
        let pattern = format!(".dialogue.{}.md", suffix);
        if path.ends_with(&pattern) {
            let base = &path[..path.len() - pattern.len()];
            return format!("{}.dialogue.md", base);
        }
    }
    path.to_string()
}

/// Strip a known status suffix from a regular filename
/// `foo.done.md` → `foo.md`
fn strip_regular_suffix(path: &str) -> String {
    for suffix in KNOWN_SUFFIXES {
        let pattern = format!(".{}.md", suffix);
        if path.ends_with(&pattern) {
            let base = &path[..path.len() - pattern.len()];
            return format!("{}.md", base);
        }
    }
    path.to_string()
}

/// Rename a document file to reflect its new status (RFC 0031)
///
/// Filesystem-first with store rollback:
/// 1. Compute new filename via `rebuild_filename()`
/// 2. `fs::rename()` old → new
/// 3. `store.update_document_file_path()` — on failure, rollback the rename
///
/// Returns `Ok(Some(new_relative_path))` if renamed, `Ok(None)` if no change needed.
pub fn rename_for_status(
    docs_path: &Path,
    store: &DocumentStore,
    doc: &Document,
    new_status: &str,
) -> Result<Option<String>, StoreError> {
    let old_rel = match doc.file_path.as_ref() {
        Some(p) => p.clone(),
        None => return Ok(None),
    };

    let new_rel = rebuild_filename(&old_rel, doc.doc_type, new_status);
    if new_rel == old_rel {
        return Ok(None);
    }

    let old_abs = docs_path.join(&old_rel);
    let new_abs = docs_path.join(&new_rel);

    // Only rename if the old file actually exists
    if !old_abs.exists() {
        return Ok(None);
    }

    // Filesystem rename
    std::fs::rename(&old_abs, &new_abs).map_err(|e| {
        StoreError::InvalidOperation(format!("Failed to rename {} → {}: {}", old_rel, new_rel, e))
    })?;

    // Update store
    if let Err(e) = store.update_document_file_path(doc.doc_type, &doc.title, &new_rel) {
        // Rollback filesystem rename
        let _ = std::fs::rename(&new_abs, &old_abs);
        return Err(e);
    }

    Ok(Some(new_rel))
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
    /// Content hash for staleness detection (RFC 0018)
    pub content_hash: Option<String>,
    /// When the document was last indexed from filesystem (RFC 0018)
    pub indexed_at: Option<String>,
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
            content_hash: None,
            indexed_at: None,
        }
    }

    /// Check if document is soft-deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Check if document is stale based on content hash (RFC 0018)
    pub fn is_stale(&self, file_path: &Path) -> bool {
        use std::fs;

        // If no file exists, document isn't stale (it's orphaned, handled separately)
        if !file_path.exists() {
            return false;
        }

        // If we have no hash, document is stale (needs indexing)
        let Some(ref stored_hash) = self.content_hash else {
            return true;
        };

        // Fast path: check mtime if we have indexed_at
        if let Some(ref indexed_at) = self.indexed_at {
            if let Ok(metadata) = fs::metadata(file_path) {
                if let Ok(modified) = metadata.modified() {
                    let file_mtime: chrono::DateTime<chrono::Utc> = modified.into();
                    if let Ok(indexed_time) = chrono::DateTime::parse_from_rfc3339(indexed_at) {
                        // File hasn't changed since indexing
                        if file_mtime <= indexed_time {
                            return false;
                        }
                    }
                }
            }
        }

        // Slow path: verify with hash
        if let Ok(content) = fs::read_to_string(file_path) {
            let current_hash = hash_content(&content);
            return current_hash != *stored_hash;
        }

        // If we can't read the file, assume not stale
        false
    }
}

/// Result of parsing a document from a file (RFC 0018)
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub doc_type: DocType,
    pub title: String,
    pub number: Option<i32>,
    pub status: String,
    pub content_hash: String,
}

/// Parse document metadata from a markdown file's frontmatter (RFC 0018)
///
/// Extracts title, number, status from the header table format:
/// ```markdown
/// # RFC 0042: My Feature
///
/// | | |
/// |---|---|
/// | **Status** | Draft |
/// ```
pub fn parse_document_from_file(file_path: &Path) -> Result<ParsedDocument, StoreError> {
    use std::fs;

    let content = fs::read_to_string(file_path)
        .map_err(|e| StoreError::IoError(e.to_string()))?;

    // Determine doc type from path
    let path_str = file_path.to_string_lossy();
    let doc_type = if path_str.contains("/rfcs/") {
        DocType::Rfc
    } else if path_str.contains("/spikes/") {
        DocType::Spike
    } else if path_str.contains("/adrs/") {
        DocType::Adr
    } else if path_str.contains("/decisions/") {
        DocType::Decision
    } else if path_str.contains("/postmortems/") {
        DocType::Postmortem
    } else if path_str.contains("/runbooks/") {
        DocType::Runbook
    } else if path_str.contains("/dialogues/") {
        DocType::Dialogue
    } else if path_str.contains("/audits/") {
        DocType::Audit
    } else if path_str.contains("/prds/") {
        DocType::Prd
    } else {
        return Err(StoreError::InvalidOperation(
            format!("Unknown document type for path: {}", path_str)
        ));
    };

    // Extract title from first line: # Type NNNN: Title or # Type: Title
    let title_re = regex::Regex::new(r"^#\s+(?:\w+)\s*(?:(\d+):?)?\s*:?\s*(.+)$").unwrap();
    let title_line = content.lines().next()
        .ok_or_else(|| StoreError::InvalidOperation("Empty file".to_string()))?;

    let (number, title) = if let Some(caps) = title_re.captures(title_line) {
        let num = caps.get(1).and_then(|m| m.as_str().parse().ok());
        let title = caps.get(2)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        (num, title)
    } else {
        // Fallback: use filename as title
        let stem = file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled");
        // Try to extract number from filename like "0042-my-feature.md"
        let num_re = regex::Regex::new(r"^(\d+)-(.+)$").unwrap();
        if let Some(caps) = num_re.captures(stem) {
            let num = caps.get(1).and_then(|m| m.as_str().parse().ok());
            let title = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_else(|| stem.to_string());
            (num, title)
        } else {
            (None, stem.to_string())
        }
    };

    // Extract status from table format: | **Status** | Draft |
    let status_re = regex::Regex::new(r"\|\s*\*\*Status\*\*\s*\|\s*([^|]+)\s*\|").unwrap();
    let status = content.lines()
        .find_map(|line| {
            status_re.captures(line)
                .map(|c| c.get(1).unwrap().as_str().trim().to_lowercase())
        })
        .unwrap_or_else(|| "draft".to_string());

    let content_hash = hash_content(&content);

    Ok(ParsedDocument {
        doc_type,
        title,
        number,
        status,
        content_hash,
    })
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

/// Result of reconciling database with filesystem (RFC 0018)
#[derive(Debug, Clone, Default)]
pub struct ReconcileResult {
    /// Files found on filesystem but not in database
    pub unindexed: Vec<String>,
    /// DB records with no corresponding file
    pub orphaned: Vec<String>,
    /// Files that have changed since last index
    pub stale: Vec<String>,
    /// Number of documents added (when not dry_run)
    pub added: usize,
    /// Number of documents updated (when not dry_run)
    pub updated: usize,
    /// Number of documents soft-deleted (when not dry_run)
    pub soft_deleted: usize,
}

impl ReconcileResult {
    /// Check if there is any drift between filesystem and database
    pub fn has_drift(&self) -> bool {
        !self.unindexed.is_empty() || !self.orphaned.is_empty() || !self.stale.is_empty()
    }

    /// Total count of issues found
    pub fn drift_count(&self) -> usize {
        self.unindexed.len() + self.orphaned.len() + self.stale.len()
    }
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

    pub fn parse(s: &str) -> Option<Self> {
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

    pub fn parse(s: &str) -> Option<Self> {
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

/// Parameters for recording a new staging deployment
pub struct StagingDeploymentParams<'a> {
    pub name: &'a str,
    pub iac_type: &'a str,
    pub deploy_command: &'a str,
    pub stacks: Option<&'a str>,
    pub deployed_by: &'a str,
    pub agent_id: Option<&'a str>,
    pub ttl_hours: u32,
    pub metadata: Option<&'a str>,
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

// ==================== Context Injection Types (RFC 0016) ====================

/// A logged context injection event
#[derive(Debug, Clone)]
pub struct ContextInjection {
    pub id: Option<i64>,
    pub session_id: String,
    pub timestamp: String,
    pub tier: String,
    pub source_uri: String,
    pub content_hash: String,
    pub token_count: Option<i32>,
}

impl ContextInjection {
    pub fn new(session_id: &str, tier: &str, source_uri: &str, content_hash: &str, token_count: Option<i32>) -> Self {
        Self {
            id: None,
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tier: tier.to_string(),
            source_uri: source_uri.to_string(),
            content_hash: content_hash.to_string(),
            token_count,
        }
    }
}

// ==================== Dynamic Context Activation Types (RFC 0017) ====================

/// A relevance edge connecting two documents
#[derive(Debug, Clone)]
pub struct RelevanceEdge {
    pub id: Option<i64>,
    pub source_uri: String,
    pub target_uri: String,
    pub edge_type: EdgeType,
    pub weight: f64,
    pub created_at: String,
}

/// Types of relevance edges
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    /// Explicitly declared relationship (e.g., "References: ADR 0005")
    Explicit,
    /// Keyword-based similarity
    Keyword,
    /// Learned from co-access patterns
    Learned,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeType::Explicit => "explicit",
            EdgeType::Keyword => "keyword",
            EdgeType::Learned => "learned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "explicit" => Some(EdgeType::Explicit),
            "keyword" => Some(EdgeType::Keyword),
            "learned" => Some(EdgeType::Learned),
            _ => None,
        }
    }
}

impl RelevanceEdge {
    pub fn new(source_uri: &str, target_uri: &str, edge_type: EdgeType) -> Self {
        Self {
            id: None,
            source_uri: source_uri.to_string(),
            target_uri: target_uri.to_string(),
            edge_type,
            weight: 1.0,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Staleness check result for a document
#[derive(Debug, Clone)]
pub struct StalenessCheck {
    pub uri: String,
    pub is_stale: bool,
    pub reason: StalenessReason,
    pub last_injected: Option<String>,
    pub current_hash: String,
    pub injected_hash: Option<String>,
}

/// Reason why a document is considered stale
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StalenessReason {
    /// Document was never injected in this session
    NeverInjected,
    /// Content hash changed since last injection
    ContentChanged,
    /// Document is fresh (not stale)
    Fresh,
}

/// Refresh policy for different document types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshPolicy {
    /// Refresh only at session start
    SessionStart,
    /// Refresh whenever content changes
    OnChange,
    /// Refresh only on explicit request
    OnRequest,
    /// Never automatically refresh
    Never,
}

/// Rate limiter state for refresh operations
#[derive(Debug, Clone)]
pub struct RefreshRateLimit {
    pub session_id: String,
    pub last_refresh: Option<String>,
    pub cooldown_secs: u64,
}

impl RefreshRateLimit {
    pub const DEFAULT_COOLDOWN_SECS: u64 = 30;

    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            last_refresh: None,
            cooldown_secs: Self::DEFAULT_COOLDOWN_SECS,
        }
    }

    pub fn is_allowed(&self) -> bool {
        match &self.last_refresh {
            None => true,
            Some(last) => {
                if let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(last) {
                    let elapsed = chrono::Utc::now().signed_duration_since(last_time);
                    elapsed.num_seconds() >= self.cooldown_secs as i64
                } else {
                    true
                }
            }
        }
    }
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

    #[error("File system error: {0}")]
    IoError(String),
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

        // Migration from v4 to v5: Add context injection audit table (RFC 0016)
        if from_version < 5 {
            debug!("Adding context injection audit table (RFC 0016)");

            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS context_injections (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    timestamp TEXT NOT NULL,
                    tier TEXT NOT NULL,
                    source_uri TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    token_count INTEGER
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_context_injections_session ON context_injections(session_id)",
                [],
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_context_injections_timestamp ON context_injections(timestamp)",
                [],
            )?;
        }

        // Migration from v5 to v6: Add relevance graph and staleness tracking (RFC 0017)
        if from_version < 6 {
            debug!("Adding relevance graph and staleness tracking (RFC 0017)");

            // Create relevance_edges table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS relevance_edges (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_uri TEXT NOT NULL,
                    target_uri TEXT NOT NULL,
                    edge_type TEXT NOT NULL,
                    weight REAL DEFAULT 1.0,
                    created_at TEXT NOT NULL,
                    UNIQUE(source_uri, target_uri, edge_type)
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_relevance_source ON relevance_edges(source_uri)",
                [],
            )?;
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_relevance_target ON relevance_edges(target_uri)",
                [],
            )?;

            // Add staleness tracking index
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_documents_staleness ON documents(doc_type, updated_at) WHERE deleted_at IS NULL",
                [],
            )?;
        }

        // Migration from v6 to v7: Add plan_cache table (RFC 0017 - Plan File Authority)
        if from_version < 7 {
            debug!("Adding plan_cache table (RFC 0017)");

            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS plan_cache (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    document_id INTEGER NOT NULL UNIQUE,
                    cache_mtime TEXT NOT NULL,
                    FOREIGN KEY (document_id) REFERENCES documents(id) ON DELETE CASCADE
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_plan_cache_document ON plan_cache(document_id)",
                [],
            )?;
        }

        // Migration from v7 to v8: Add content_hash and indexed_at columns (RFC 0018)
        if from_version < 8 {
            debug!("Adding content_hash and indexed_at columns (RFC 0018)");

            // Check if columns exist first
            let has_content_hash: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('documents') WHERE name = 'content_hash'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            )?;

            if !has_content_hash {
                self.conn.execute(
                    "ALTER TABLE documents ADD COLUMN content_hash TEXT",
                    [],
                )?;
            }

            let has_indexed_at: bool = self.conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('documents') WHERE name = 'indexed_at'",
                [],
                |row| Ok(row.get::<_, i64>(0)? > 0),
            )?;

            if !has_indexed_at {
                self.conn.execute(
                    "ALTER TABLE documents ADD COLUMN indexed_at TEXT",
                    [],
                )?;
            }

            // Add index for staleness checking
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_documents_content_hash ON documents(content_hash)",
                [],
            )?;
        }

        // Migration from v8 to v9: Add alignment dialogue tables (RFC 0051)
        if from_version < 9 {
            debug!("Adding alignment dialogue tables (RFC 0051)");

            // Root table for dialogues
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_dialogues (
                    dialogue_id     TEXT PRIMARY KEY,
                    title           TEXT NOT NULL,
                    question        TEXT,
                    status          TEXT NOT NULL DEFAULT 'open',
                    created_at      TEXT NOT NULL,
                    converged_at    TEXT,
                    total_rounds    INTEGER DEFAULT 0,
                    total_alignment INTEGER DEFAULT 0,
                    output_dir      TEXT,
                    calibrated      INTEGER DEFAULT 0,
                    domain_id       TEXT,
                    ethos_id        TEXT,
                    background      TEXT,
                    CHECK (status IN ('open', 'converging', 'converged', 'abandoned'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_alignment_dialogues_title_created
                 ON alignment_dialogues(title, created_at)",
                [],
            )?;

            // Experts table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_experts (
                    dialogue_id   TEXT NOT NULL,
                    expert_slug   TEXT NOT NULL,
                    role          TEXT NOT NULL,
                    description   TEXT,
                    focus         TEXT,
                    tier          TEXT NOT NULL,
                    source        TEXT NOT NULL,
                    relevance     REAL,
                    creation_reason TEXT,
                    color         TEXT,
                    scores        TEXT,
                    raw_content   TEXT,
                    total_score   INTEGER DEFAULT 0,
                    first_round   INTEGER,
                    created_at    TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, expert_slug),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (tier IN ('Core', 'Adjacent', 'Wildcard')),
                    CHECK (source IN ('pool', 'created', 'retained'))
                )",
                [],
            )?;

            // Rounds table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_rounds (
                    dialogue_id   TEXT NOT NULL,
                    round         INTEGER NOT NULL,
                    title         TEXT,
                    score         INTEGER NOT NULL,
                    summary       TEXT,
                    status        TEXT NOT NULL DEFAULT 'open',
                    created_at    TEXT NOT NULL,
                    completed_at  TEXT,
                    PRIMARY KEY (dialogue_id, round),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('open', 'in_progress', 'completed'))
                )",
                [],
            )?;

            // Perspectives table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_perspectives (
                    dialogue_id    TEXT NOT NULL,
                    round          INTEGER NOT NULL,
                    seq            INTEGER NOT NULL,
                    label          TEXT NOT NULL,
                    content        TEXT NOT NULL,
                    contributors   TEXT NOT NULL,
                    status         TEXT NOT NULL DEFAULT 'open',
                    refs           TEXT,
                    created_at     TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('open', 'refined', 'conceded', 'merged'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_perspectives_dialogue_round
                 ON alignment_perspectives(dialogue_id, round, created_at)",
                [],
            )?;

            // Perspective events
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_perspective_events (
                    dialogue_id       TEXT NOT NULL,
                    perspective_round INTEGER NOT NULL,
                    perspective_seq   INTEGER NOT NULL,
                    event_type        TEXT NOT NULL,
                    event_round       INTEGER NOT NULL,
                    actors            TEXT NOT NULL,
                    result_id         TEXT,
                    created_at        TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, perspective_round, perspective_seq, created_at),
                    FOREIGN KEY (dialogue_id, perspective_round, perspective_seq)
                      REFERENCES alignment_perspectives(dialogue_id, round, seq),
                    CHECK (event_type IN ('created', 'refined', 'conceded', 'merged'))
                )",
                [],
            )?;

            // Tensions table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_tensions (
                    dialogue_id      TEXT NOT NULL,
                    round            INTEGER NOT NULL,
                    seq              INTEGER NOT NULL,
                    label            TEXT NOT NULL,
                    description      TEXT NOT NULL,
                    contributors     TEXT NOT NULL,
                    status           TEXT NOT NULL DEFAULT 'open',
                    refs             TEXT,
                    created_at       TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('open', 'addressed', 'resolved', 'reopened'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_tensions_status
                 ON alignment_tensions(dialogue_id, status)",
                [],
            )?;

            // Tension events
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_tension_events (
                    dialogue_id    TEXT NOT NULL,
                    tension_round  INTEGER NOT NULL,
                    tension_seq    INTEGER NOT NULL,
                    event_type     TEXT NOT NULL,
                    event_round    INTEGER NOT NULL,
                    actors         TEXT NOT NULL,
                    reason         TEXT,
                    reference      TEXT,
                    created_at     TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, tension_round, tension_seq, created_at),
                    FOREIGN KEY (dialogue_id, tension_round, tension_seq)
                      REFERENCES alignment_tensions(dialogue_id, round, seq),
                    CHECK (event_type IN ('created', 'addressed', 'resolved', 'reopened', 'commented'))
                )",
                [],
            )?;

            // Recommendations table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_recommendations (
                    dialogue_id        TEXT NOT NULL,
                    round              INTEGER NOT NULL,
                    seq                INTEGER NOT NULL,
                    label              TEXT NOT NULL,
                    content            TEXT NOT NULL,
                    contributors       TEXT NOT NULL,
                    parameters         TEXT,
                    status             TEXT NOT NULL DEFAULT 'proposed',
                    refs               TEXT,
                    adopted_in_verdict TEXT,
                    created_at         TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('proposed', 'amended', 'adopted', 'rejected'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_recommendations_status
                 ON alignment_recommendations(dialogue_id, status)",
                [],
            )?;

            // Recommendation events
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_recommendation_events (
                    dialogue_id     TEXT NOT NULL,
                    rec_round       INTEGER NOT NULL,
                    rec_seq         INTEGER NOT NULL,
                    event_type      TEXT NOT NULL,
                    event_round     INTEGER NOT NULL,
                    actors          TEXT NOT NULL,
                    result_id       TEXT,
                    created_at      TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, rec_round, rec_seq, created_at),
                    FOREIGN KEY (dialogue_id, rec_round, rec_seq)
                      REFERENCES alignment_recommendations(dialogue_id, round, seq),
                    CHECK (event_type IN ('created', 'amended', 'adopted', 'rejected'))
                )",
                [],
            )?;

            // Evidence table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_evidence (
                    dialogue_id    TEXT NOT NULL,
                    round          INTEGER NOT NULL,
                    seq            INTEGER NOT NULL,
                    label          TEXT NOT NULL,
                    content        TEXT NOT NULL,
                    contributors   TEXT NOT NULL,
                    status         TEXT NOT NULL DEFAULT 'cited',
                    refs           TEXT,
                    created_at     TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('cited', 'challenged', 'confirmed', 'refuted'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_evidence_status
                 ON alignment_evidence(dialogue_id, status)",
                [],
            )?;

            // Claims table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_claims (
                    dialogue_id    TEXT NOT NULL,
                    round          INTEGER NOT NULL,
                    seq            INTEGER NOT NULL,
                    label          TEXT NOT NULL,
                    content        TEXT NOT NULL,
                    contributors   TEXT NOT NULL,
                    status         TEXT NOT NULL DEFAULT 'asserted',
                    refs           TEXT,
                    created_at     TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (status IN ('asserted', 'supported', 'opposed', 'adopted', 'withdrawn'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_claims_status
                 ON alignment_claims(dialogue_id, status)",
                [],
            )?;

            // Cross-references table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_refs (
                    dialogue_id   TEXT NOT NULL,
                    source_type   TEXT NOT NULL,
                    source_id     TEXT NOT NULL,
                    ref_type      TEXT NOT NULL,
                    target_type   TEXT NOT NULL,
                    target_id     TEXT NOT NULL,
                    created_at    TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, source_id, ref_type, target_id),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (source_type IN ('P', 'R', 'T', 'E', 'C')),
                    CHECK (target_type IN ('P', 'R', 'T', 'E', 'C')),
                    CHECK (ref_type IN ('support', 'oppose', 'refine', 'address', 'resolve', 'reopen', 'question', 'depend'))
                )",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_refs_target
                 ON alignment_refs(dialogue_id, target_id, ref_type)",
                [],
            )?;

            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_alignment_refs_source
                 ON alignment_refs(dialogue_id, source_id)",
                [],
            )?;

            // Dialogue moves table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_moves (
                    dialogue_id   TEXT NOT NULL,
                    round         INTEGER NOT NULL,
                    seq           INTEGER NOT NULL,
                    expert_slug   TEXT NOT NULL,
                    move_type     TEXT NOT NULL,
                    targets       TEXT NOT NULL,
                    context       TEXT,
                    created_at    TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, round, expert_slug, seq),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (move_type IN ('defend', 'challenge', 'bridge', 'request', 'concede', 'converge'))
                )",
                [],
            )?;

            // Verdicts table
            self.conn.execute(
                "CREATE TABLE IF NOT EXISTS alignment_verdicts (
                    dialogue_id     TEXT NOT NULL,
                    verdict_id      TEXT NOT NULL,
                    verdict_type    TEXT NOT NULL,
                    round           INTEGER NOT NULL,
                    author_expert   TEXT,
                    recommendation  TEXT NOT NULL,
                    description     TEXT NOT NULL,
                    conditions      TEXT,
                    vote            TEXT,
                    confidence      TEXT,
                    tensions_resolved TEXT,
                    tensions_accepted TEXT,
                    recommendations_adopted TEXT,
                    key_evidence    TEXT,
                    key_claims      TEXT,
                    supporting_experts TEXT,
                    ethos_compliance TEXT,
                    created_at      TEXT NOT NULL,
                    PRIMARY KEY (dialogue_id, verdict_id),
                    FOREIGN KEY (dialogue_id) REFERENCES alignment_dialogues(dialogue_id),
                    CHECK (verdict_type IN ('interim', 'final', 'minority', 'dissent'))
                )",
                [],
            )?;
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
                "INSERT INTO documents (doc_type, number, title, status, file_path, created_at, updated_at, content_hash, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    doc.doc_type.as_str(),
                    doc.number,
                    doc.title,
                    doc.status,
                    doc.file_path,
                    now,
                    now,
                    doc.content_hash,
                    doc.indexed_at.as_ref().unwrap_or(&now),
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get a document by type and title
    pub fn get_document(&self, doc_type: DocType, title: &str) -> Result<Document, StoreError> {
        self.conn
            .query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE doc_type = ?1 AND title = ?2 AND deleted_at IS NULL",
                params![doc_type.as_str(), title],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE doc_type = ?1 AND number = ?2 AND deleted_at IS NULL",
                params![doc_type.as_str(), number],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
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

        // Try slug match (RFC 0022) - "filesystem-authority" matches "Filesystem Authority"
        let slug_as_title = query.replace('-', " ");
        if slug_as_title != *query {
            if let Ok(doc) = self.get_document(doc_type, &slug_as_title) {
                return Ok(doc);
            }
            // Case-insensitive match on deslugified query
            let pattern = format!("%{}%", slug_as_title.to_lowercase());
            if let Ok(doc) = self.conn.query_row(
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE doc_type = ?1 AND LOWER(title) LIKE ?2 AND deleted_at IS NULL
                 ORDER BY LENGTH(title) ASC LIMIT 1",
                params![doc_type.as_str(), pattern],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
                    })
                },
            ) {
                return Ok(doc);
            }
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
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
             FROM documents WHERE doc_type = ?1 AND LOWER(title) LIKE ?2 AND deleted_at IS NULL
             ORDER BY LENGTH(title) ASC LIMIT 1",
            params![doc_type.as_str(), pattern],
            |row| {
                Ok(Document {
                    id: Some(row.get(0)?),
                    doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                    number: row.get(2)?,
                    title: row.get(3)?,
                    status: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    deleted_at: row.get(8)?,
                    content_hash: row.get(9)?,
                    indexed_at: row.get(10)?,
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

    /// Find a document with filesystem fallback (RFC 0018)
    ///
    /// First tries the database, then falls back to scanning the filesystem
    /// if the document isn't found. Any document found on filesystem is
    /// automatically registered in the database.
    pub fn find_document_with_fallback(
        &self,
        doc_type: DocType,
        query: &str,
        docs_path: &Path,
    ) -> Result<Document, StoreError> {
        // Try database first (fast path)
        if let Ok(doc) = self.find_document(doc_type, query) {
            return Ok(doc);
        }

        // Fall back to filesystem scan
        self.scan_and_register(doc_type, query, docs_path)
    }

    /// Scan filesystem for a document and register it (RFC 0018)
    pub fn scan_and_register(
        &self,
        doc_type: DocType,
        query: &str,
        docs_path: &Path,
    ) -> Result<Document, StoreError> {
        use std::fs;

        let subdir = match doc_type {
            DocType::Rfc => "rfcs",
            DocType::Spike => "spikes",
            DocType::Adr => "adrs",
            DocType::Decision => "decisions",
            DocType::Dialogue => "dialogues",
            DocType::Audit => "audits",
            DocType::Runbook => "runbooks",
            DocType::Postmortem => "postmortems",
            DocType::Prd => "prds",
        };

        let search_dir = docs_path.join(subdir);
        if !search_dir.exists() {
            return Err(StoreError::NotFound(format!(
                "{} matching '{}' (directory {} not found)",
                doc_type.as_str(),
                query,
                search_dir.display()
            )));
        }

        let query_lower = query.to_lowercase();

        // Try to parse query as a number
        let query_num: Option<i32> = query.trim_start_matches('0')
            .parse()
            .ok()
            .or_else(|| if query == "0" { Some(0) } else { None });

        // Scan directory for matching files
        let entries = fs::read_dir(&search_dir)
            .map_err(|e| StoreError::IoError(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                // Skip .plan.md files
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".plan.md") {
                        continue;
                    }
                }

                // Try to parse the file
                if let Ok(parsed) = parse_document_from_file(&path) {
                    if parsed.doc_type != doc_type {
                        continue;
                    }

                    // Check if this file matches the query
                    let matches = parsed.title.to_lowercase().contains(&query_lower)
                        || query_num.map(|n| parsed.number == Some(n)).unwrap_or(false);

                    if matches {
                        // Register this document in the database
                        let relative_path = path.strip_prefix(docs_path)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| path.to_string_lossy().to_string());

                        let doc = Document {
                            id: None,
                            doc_type: parsed.doc_type,
                            number: parsed.number,
                            title: parsed.title,
                            status: parsed.status,
                            file_path: Some(relative_path),
                            created_at: None,
                            updated_at: None,
                            deleted_at: None,
                            content_hash: Some(parsed.content_hash),
                            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
                        };

                        let id = self.add_document(&doc)?;
                        return self.get_document_by_id(id);
                    }
                }
            }
        }

        Err(StoreError::NotFound(format!(
            "{} matching '{}'",
            doc_type.as_str(),
            query
        )))
    }

    /// Register a document from a file path (RFC 0018)
    pub fn register_from_file(&self, file_path: &Path, docs_path: &Path) -> Result<Document, StoreError> {
        let parsed = parse_document_from_file(file_path)?;

        let relative_path = file_path.strip_prefix(docs_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        let doc = Document {
            id: None,
            doc_type: parsed.doc_type,
            number: parsed.number,
            title: parsed.title,
            status: parsed.status,
            file_path: Some(relative_path),
            created_at: None,
            updated_at: None,
            deleted_at: None,
            content_hash: Some(parsed.content_hash),
            indexed_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        let id = self.add_document(&doc)?;
        self.get_document_by_id(id)
    }

    /// Reconcile database with filesystem (RFC 0018)
    ///
    /// Scans the filesystem for documents and reconciles with the database:
    /// - Files without DB records: create records
    /// - DB records without files: soft-delete records
    /// - Hash mismatch: update DB from file
    pub fn reconcile(
        &self,
        docs_path: &Path,
        doc_type: Option<DocType>,
        dry_run: bool,
    ) -> Result<ReconcileResult, StoreError> {
        use std::collections::HashSet;
        use std::fs;

        let mut result = ReconcileResult::default();

        let subdirs: Vec<(&str, DocType)> = match doc_type {
            Some(dt) => vec![(dt.subdir(), dt)],
            None => vec![
                ("rfcs", DocType::Rfc),
                ("spikes", DocType::Spike),
                ("adrs", DocType::Adr),
                ("decisions", DocType::Decision),
                ("dialogues", DocType::Dialogue),
                ("audits", DocType::Audit),
                ("runbooks", DocType::Runbook),
                ("postmortems", DocType::Postmortem),
                ("prds", DocType::Prd),
            ],
        };

        for (subdir, dt) in subdirs {
            let search_dir = docs_path.join(subdir);
            if !search_dir.exists() {
                continue;
            }

            // Track files we've seen
            let mut seen_files: HashSet<String> = HashSet::new();

            // Scan filesystem
            if let Ok(entries) = fs::read_dir(&search_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        // Skip .plan.md files
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if name.ends_with(".plan.md") {
                                continue;
                            }
                        }

                        let relative_path = path.strip_prefix(docs_path)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| path.to_string_lossy().to_string());

                        seen_files.insert(relative_path.clone());

                        // Check if file is in database
                        if let Ok(parsed) = parse_document_from_file(&path) {
                            if parsed.doc_type != dt {
                                continue;
                            }

                            // Try to find existing document
                            let existing = self.list_documents(dt)
                                .unwrap_or_default()
                                .into_iter()
                                .find(|d| d.file_path.as_ref() == Some(&relative_path));

                            match existing {
                                None => {
                                    // File exists but no DB record
                                    result.unindexed.push(relative_path.clone());
                                    if !dry_run {
                                        if let Ok(_doc) = self.register_from_file(&path, docs_path) {
                                            result.added += 1;
                                        }
                                    }
                                }
                                Some(doc) => {
                                    // Check if stale
                                    if doc.content_hash.as_ref() != Some(&parsed.content_hash) {
                                        result.stale.push(relative_path.clone());
                                        if !dry_run {
                                            if let Some(id) = doc.id {
                                                let _ = self.update_document_index(id, &parsed.content_hash);
                                                // Also update status if it changed
                                                if doc.status.to_lowercase() != parsed.status.to_lowercase() {
                                                    let _ = self.update_document_status(dt, &doc.title, &parsed.status);
                                                }
                                                result.updated += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Check for orphan records
            for doc in self.list_documents(dt).unwrap_or_default() {
                if let Some(ref file_path) = doc.file_path {
                    if !seen_files.contains(file_path) {
                        let full_path = docs_path.join(file_path);
                        if !full_path.exists() {
                            result.orphaned.push(file_path.clone());
                            if !dry_run {
                                let _ = self.soft_delete_document(dt, &doc.title);
                                result.soft_deleted += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
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

    /// Update a document's file_path in the store (RFC 0031)
    pub fn update_document_file_path(
        &self,
        doc_type: DocType,
        title: &str,
        new_file_path: &str,
    ) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET file_path = ?1, updated_at = ?2 WHERE doc_type = ?3 AND title = ?4",
                params![new_file_path, now, doc_type.as_str(), title],
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
                 file_path = ?5, updated_at = ?6, content_hash = ?7, indexed_at = ?8 WHERE id = ?9",
                params![
                    doc.doc_type.as_str(),
                    doc.number,
                    doc.title,
                    doc.status,
                    doc.file_path,
                    now,
                    doc.content_hash,
                    doc.indexed_at.as_ref().unwrap_or(&now),
                    id
                ],
            )?;
            if updated == 0 {
                return Err(StoreError::NotFound(format!("document #{}", id)));
            }
            Ok(())
        })
    }

    /// Update a document's content hash and indexed_at timestamp (RFC 0018)
    pub fn update_document_index(
        &self,
        id: i64,
        content_hash: &str,
    ) -> Result<(), StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            let updated = self.conn.execute(
                "UPDATE documents SET content_hash = ?1, indexed_at = ?2 WHERE id = ?3",
                params![content_hash, now, id],
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
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
             FROM documents WHERE doc_type = ?1 AND deleted_at IS NULL ORDER BY number DESC, title ASC",
        )?;

        let rows = stmt.query_map(params![doc_type.as_str()], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                content_hash: row.get(9)?,
                indexed_at: row.get(10)?,
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
            "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
             FROM documents WHERE doc_type = ?1 AND status = ?2 AND deleted_at IS NULL ORDER BY number DESC, title ASC",
        )?;

        let rows = stmt.query_map(params![doc_type.as_str(), status], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                content_hash: row.get(9)?,
                indexed_at: row.get(10)?,
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE doc_type = ?1 AND title = ?2 AND deleted_at IS NOT NULL",
                params![doc_type.as_str(), title],
                |row| {
                    Ok(Document {
                        id: Some(row.get(0)?),
                        doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                        number: row.get(2)?,
                        title: row.get(3)?,
                        status: row.get(4)?,
                        file_path: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        deleted_at: row.get(8)?,
                        content_hash: row.get(9)?,
                        indexed_at: row.get(10)?,
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
                "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                 FROM documents WHERE doc_type = '{}' AND deleted_at IS NOT NULL
                 ORDER BY deleted_at DESC",
                dt.as_str()
            ),
            None => "SELECT id, doc_type, number, title, status, file_path, created_at, updated_at, deleted_at, content_hash, indexed_at
                     FROM documents WHERE deleted_at IS NOT NULL
                     ORDER BY deleted_at DESC".to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map([], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                content_hash: row.get(9)?,
                indexed_at: row.get(10)?,
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
            "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at, d.content_hash, d.indexed_at
             FROM documents d
             JOIN document_links l ON l.source_id = d.id
             WHERE l.target_id = ?1 AND l.link_type = 'rfc_to_adr' AND d.deleted_at IS NULL",
        )?;

        let rows = stmt.query_map(params![document_id], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                content_hash: row.get(9)?,
                indexed_at: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }

    /// Get the next document number for a type (RFC 0022)
    ///
    /// Scans both database AND filesystem, taking the maximum.
    /// Filesystem is truth - prevents numbering collisions when
    /// files exist on disk but aren't yet indexed.
    pub fn next_number(&self, doc_type: DocType) -> Result<i32, StoreError> {
        let db_max: Option<i32> = self.conn.query_row(
            "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
            params![doc_type.as_str()],
            |row| row.get(0),
        )?;
        Ok(db_max.unwrap_or(0) + 1)
    }

    /// Get the next document number, scanning filesystem too (RFC 0022)
    ///
    /// Use this instead of `next_number()` when you have access to the docs path.
    pub fn next_number_with_fs(&self, doc_type: DocType, docs_path: &Path) -> Result<i32, StoreError> {
        // Database max (fast, possibly stale)
        let db_max: Option<i32> = self.conn.query_row(
            "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
            params![doc_type.as_str()],
            |row| row.get(0),
        )?;

        // Filesystem max (authoritative)
        let fs_max = self.scan_filesystem_max(doc_type, docs_path)?;

        // Take max of both - filesystem wins
        Ok(std::cmp::max(db_max.unwrap_or(0), fs_max) + 1)
    }

    /// Scan filesystem directory for the highest document number (RFC 0022)
    fn scan_filesystem_max(&self, doc_type: DocType, docs_path: &Path) -> Result<i32, StoreError> {
        use regex::Regex;
        use std::fs;

        let dir = docs_path.join(doc_type.subdir());
        if !dir.exists() {
            return Ok(0);
        }

        let pattern = Regex::new(r"^(\d{4})-.*\.md$")
            .map_err(|e| StoreError::IoError(e.to_string()))?;
        let mut max = 0;

        let entries = fs::read_dir(&dir)
            .map_err(|e| StoreError::IoError(e.to_string()))?;

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                // Skip .plan.md files
                if name.ends_with(".plan.md") {
                    continue;
                }
                if let Some(caps) = pattern.captures(name) {
                    if let Ok(num) = caps[1].parse::<i32>() {
                        max = std::cmp::max(max, num);
                    }
                }
            }
        }

        Ok(max)
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
                "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at, d.content_hash, d.indexed_at
                 FROM documents d
                 JOIN document_links l ON l.target_id = d.id
                 WHERE l.source_id = ?1 AND l.link_type = '{}' AND d.deleted_at IS NULL",
                lt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path, d.created_at, d.updated_at, d.deleted_at, d.content_hash, d.indexed_at
                     FROM documents d
                     JOIN document_links l ON l.target_id = d.id
                     WHERE l.source_id = ?1 AND d.deleted_at IS NULL".to_string(),
        };

        let mut stmt = self.conn.prepare(&query)?;
        let rows = stmt.query_map(params![source_id], |row| {
            Ok(Document {
                id: Some(row.get(0)?),
                doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                number: row.get(2)?,
                title: row.get(3)?,
                status: row.get(4)?,
                file_path: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                deleted_at: row.get(8)?,
                content_hash: row.get(9)?,
                indexed_at: row.get(10)?,
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

    // ==================== Plan Cache Operations (RFC 0017) ====================

    /// Get the cached mtime for a plan file
    pub fn get_plan_cache_mtime(&self, document_id: i64) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row(
                "SELECT cache_mtime FROM plan_cache WHERE document_id = ?1",
                params![document_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Update the cached mtime for a plan file
    pub fn update_plan_cache_mtime(&self, document_id: i64, mtime: &str) -> Result<(), StoreError> {
        self.with_retry(|| {
            self.conn.execute(
                "INSERT INTO plan_cache (document_id, cache_mtime) VALUES (?1, ?2)
                 ON CONFLICT(document_id) DO UPDATE SET cache_mtime = excluded.cache_mtime",
                params![document_id, mtime],
            )?;
            Ok(())
        })
    }

    /// Rebuild tasks from plan file data (RFC 0017 - authority inversion)
    pub fn rebuild_tasks_from_plan(
        &self,
        document_id: i64,
        tasks: &[crate::plan::PlanTask],
    ) -> Result<(), StoreError> {
        self.with_retry(|| {
            // Delete existing tasks
            self.conn
                .execute("DELETE FROM tasks WHERE document_id = ?1", params![document_id])?;

            // Insert tasks from plan file
            for (index, task) in tasks.iter().enumerate() {
                let completed_at = if task.completed {
                    Some(chrono::Utc::now().to_rfc3339())
                } else {
                    None
                };

                self.conn.execute(
                    "INSERT INTO tasks (document_id, task_index, description, completed, completed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        document_id,
                        index as i32,
                        task.description,
                        task.completed as i32,
                        completed_at
                    ],
                )?;
            }

            Ok(())
        })
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
                        d.created_at, d.updated_at, d.deleted_at, d.content_hash, d.indexed_at, bm25(documents_fts) as score
                 FROM documents_fts fts
                 JOIN documents d ON d.id = fts.rowid
                 WHERE documents_fts MATCH ?1 AND d.doc_type = '{}' AND d.deleted_at IS NULL
                 ORDER BY score
                 LIMIT ?2",
                dt.as_str()
            ),
            None => "SELECT d.id, d.doc_type, d.number, d.title, d.status, d.file_path,
                            d.created_at, d.updated_at, d.deleted_at, d.content_hash, d.indexed_at, bm25(documents_fts) as score
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
                    doc_type: DocType::parse(row.get::<_, String>(1)?.as_str()).unwrap(),
                    number: row.get(2)?,
                    title: row.get(3)?,
                    status: row.get(4)?,
                    file_path: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    deleted_at: row.get(8)?,
                    content_hash: row.get(9)?,
                    indexed_at: row.get(10)?,
                },
                score: row.get(11)?,
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
                        session_type: SessionType::parse(&row.get::<_, String>(2)?).unwrap_or(SessionType::Implementation),
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
                session_type: SessionType::parse(&row.get::<_, String>(2)?).unwrap_or(SessionType::Implementation),
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
                        status: ReminderStatus::parse(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
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
                    status: ReminderStatus::parse(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
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
                        status: ReminderStatus::parse(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
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
                status: ReminderStatus::parse(&row.get::<_, String>(6)?).unwrap_or(ReminderStatus::Pending),
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
        params: &StagingDeploymentParams<'_>,
    ) -> Result<StagingDeployment, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now();
            let ttl_expires = now + chrono::Duration::hours(params.ttl_hours as i64);

            self.conn.execute(
                "INSERT OR REPLACE INTO staging_deployments
                 (name, iac_type, deploy_command, stacks, deployed_by, agent_id, deployed_at, ttl_expires_at, status, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'deployed', ?9)",
                params![
                    params.name,
                    params.iac_type,
                    params.deploy_command,
                    params.stacks,
                    params.deployed_by,
                    params.agent_id,
                    now.to_rfc3339(),
                    ttl_expires.to_rfc3339(),
                    params.metadata
                ],
            )?;

            Ok(StagingDeployment {
                id: Some(self.conn.last_insert_rowid()),
                name: params.name.to_string(),
                iac_type: params.iac_type.to_string(),
                deploy_command: params.deploy_command.to_string(),
                stacks: params.stacks.map(|s| s.to_string()),
                deployed_by: params.deployed_by.to_string(),
                agent_id: params.agent_id.map(|s| s.to_string()),
                deployed_at: now.to_rfc3339(),
                ttl_expires_at: ttl_expires.to_rfc3339(),
                status: "deployed".to_string(),
                destroyed_at: None,
                metadata: params.metadata.map(|s| s.to_string()),
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

    // ==================== Context Injection Methods (RFC 0016) ====================

    /// Log a context injection event
    pub fn log_injection(
        &self,
        session_id: &str,
        tier: &str,
        source_uri: &str,
        content_hash: &str,
        token_count: Option<i32>,
    ) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT INTO context_injections (session_id, timestamp, tier, source_uri, content_hash, token_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![session_id, now, tier, source_uri, content_hash, token_count],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get injection history for a session
    pub fn get_injection_history(&self, session_id: &str) -> Result<Vec<ContextInjection>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, timestamp, tier, source_uri, content_hash, token_count
             FROM context_injections
             WHERE session_id = ?1
             ORDER BY timestamp ASC",
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            Ok(ContextInjection {
                id: Some(row.get(0)?),
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                tier: row.get(3)?,
                source_uri: row.get(4)?,
                content_hash: row.get(5)?,
                token_count: row.get(6)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get recent injections across all sessions (for debugging)
    pub fn get_recent_injections(&self, limit: usize) -> Result<Vec<ContextInjection>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, timestamp, tier, source_uri, content_hash, token_count
             FROM context_injections
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ContextInjection {
                id: Some(row.get(0)?),
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                tier: row.get(3)?,
                source_uri: row.get(4)?,
                content_hash: row.get(5)?,
                token_count: row.get(6)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get injection stats for a session
    pub fn get_injection_stats(&self, session_id: &str) -> Result<(usize, i64), StoreError> {
        let result = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(token_count), 0)
             FROM context_injections
             WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get::<_, i64>(0)? as usize, row.get::<_, i64>(1)?)),
        )?;
        Ok(result)
    }

    /// Get the last injection for a URI in a session
    pub fn get_last_injection(&self, session_id: &str, uri: &str) -> Result<Option<ContextInjection>, StoreError> {
        self.conn
            .query_row(
                "SELECT id, session_id, timestamp, tier, source_uri, content_hash, token_count
                 FROM context_injections
                 WHERE session_id = ?1 AND source_uri = ?2
                 ORDER BY timestamp DESC
                 LIMIT 1",
                params![session_id, uri],
                |row| {
                    Ok(ContextInjection {
                        id: Some(row.get(0)?),
                        session_id: row.get(1)?,
                        timestamp: row.get(2)?,
                        tier: row.get(3)?,
                        source_uri: row.get(4)?,
                        content_hash: row.get(5)?,
                        token_count: row.get(6)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::Database)
    }

    /// Get the last refresh time for a session (for rate limiting)
    pub fn get_last_refresh_time(&self, session_id: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row(
                "SELECT MAX(timestamp) FROM context_injections WHERE session_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::Database)
            .map(|opt| opt.flatten())
    }

    /// Get recent injections for a session
    pub fn get_session_injections(&self, session_id: &str, limit: usize) -> Result<Vec<ContextInjection>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, timestamp, tier, source_uri, content_hash, token_count
             FROM context_injections
             WHERE session_id = ?1
             ORDER BY timestamp DESC
             LIMIT ?2"
        ).map_err(StoreError::Database)?;

        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(ContextInjection {
                id: Some(row.get(0)?),
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                tier: row.get(3)?,
                source_uri: row.get(4)?,
                content_hash: row.get(5)?,
                token_count: row.get(6)?,
            })
        }).map_err(StoreError::Database)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(StoreError::Database)
    }

    // ==================== Relevance Graph Methods (RFC 0017) ====================

    /// Add a relevance edge
    pub fn add_relevance_edge(&self, edge: &RelevanceEdge) -> Result<i64, StoreError> {
        self.with_retry(|| {
            let now = chrono::Utc::now().to_rfc3339();
            self.conn.execute(
                "INSERT OR REPLACE INTO relevance_edges (source_uri, target_uri, edge_type, weight, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    edge.source_uri,
                    edge.target_uri,
                    edge.edge_type.as_str(),
                    edge.weight,
                    now,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        })
    }

    /// Get relevance edges from a source URI
    pub fn get_relevance_edges(&self, source_uri: &str) -> Result<Vec<RelevanceEdge>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_uri, target_uri, edge_type, weight, created_at
             FROM relevance_edges
             WHERE source_uri = ?1
             ORDER BY weight DESC",
        )?;

        let rows = stmt.query_map(params![source_uri], |row| {
            Ok(RelevanceEdge {
                id: Some(row.get(0)?),
                source_uri: row.get(1)?,
                target_uri: row.get(2)?,
                edge_type: EdgeType::parse(&row.get::<_, String>(3)?).unwrap_or(EdgeType::Explicit),
                weight: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Get all edges pointing to a target URI
    pub fn get_incoming_edges(&self, target_uri: &str) -> Result<Vec<RelevanceEdge>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, source_uri, target_uri, edge_type, weight, created_at
             FROM relevance_edges
             WHERE target_uri = ?1
             ORDER BY weight DESC",
        )?;

        let rows = stmt.query_map(params![target_uri], |row| {
            Ok(RelevanceEdge {
                id: Some(row.get(0)?),
                source_uri: row.get(1)?,
                target_uri: row.get(2)?,
                edge_type: EdgeType::parse(&row.get::<_, String>(3)?).unwrap_or(EdgeType::Explicit),
                weight: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    /// Remove a relevance edge
    pub fn remove_relevance_edge(&self, source_uri: &str, target_uri: &str, edge_type: EdgeType) -> Result<bool, StoreError> {
        let rows = self.conn.execute(
            "DELETE FROM relevance_edges WHERE source_uri = ?1 AND target_uri = ?2 AND edge_type = ?3",
            params![source_uri, target_uri, edge_type.as_str()],
        )?;
        Ok(rows > 0)
    }

    /// Clear all edges of a specific type
    pub fn clear_edges_by_type(&self, edge_type: EdgeType) -> Result<usize, StoreError> {
        let rows = self.conn.execute(
            "DELETE FROM relevance_edges WHERE edge_type = ?1",
            params![edge_type.as_str()],
        )?;
        Ok(rows)
    }

    /// Count relevance edges
    pub fn count_relevance_edges(&self) -> Result<usize, StoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM relevance_edges",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
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

    // ==================== RFC 0022: Filesystem Authority Tests ====================

    #[test]
    fn test_title_to_slug() {
        assert_eq!(title_to_slug("Filesystem Authority"), "filesystem-authority");
        assert_eq!(title_to_slug("Plan File Authority"), "plan-file-authority");
        assert_eq!(title_to_slug("already-slug"), "already-slug");
        assert_eq!(title_to_slug("UPPER CASE"), "upper-case");
        assert_eq!(title_to_slug("single"), "single");
        assert_eq!(title_to_slug("  extra   spaces  "), "extra-spaces");
    }

    #[test]
    fn test_find_document_by_slug() {
        let store = DocumentStore::open_in_memory().unwrap();

        let doc = Document::new(DocType::Rfc, "Filesystem Authority", "draft");
        let id = store.add_document(&doc).unwrap();

        // Exact title match
        let found = store.find_document(DocType::Rfc, "Filesystem Authority").unwrap();
        assert_eq!(found.id, Some(id));

        // Slug match (RFC 0022)
        let found = store.find_document(DocType::Rfc, "filesystem-authority").unwrap();
        assert_eq!(found.id, Some(id));
        assert_eq!(found.title, "Filesystem Authority");
    }

    #[test]
    fn test_find_document_slug_with_multiple_words() {
        let store = DocumentStore::open_in_memory().unwrap();

        let doc = Document::new(DocType::Rfc, "Plan File Authority", "accepted");
        let id = store.add_document(&doc).unwrap();

        let found = store.find_document(DocType::Rfc, "plan-file-authority").unwrap();
        assert_eq!(found.id, Some(id));
    }

    #[test]
    fn test_next_number_with_fs_empty_dir() {
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        // No directory at all - should return 1
        let next = store.next_number_with_fs(DocType::Rfc, docs_path).unwrap();
        assert_eq!(next, 1);
    }

    #[test]
    fn test_next_number_with_fs_files_exist() {
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        // Create rfcs directory with files
        let rfcs_dir = docs_path.join("rfcs");
        std::fs::create_dir_all(&rfcs_dir).unwrap();
        std::fs::write(rfcs_dir.join("0001-first.md"), "# RFC 0001: First\n").unwrap();
        std::fs::write(rfcs_dir.join("0005-fifth.md"), "# RFC 0005: Fifth\n").unwrap();
        std::fs::write(rfcs_dir.join("0003-third.md"), "# RFC 0003: Third\n").unwrap();

        // DB is empty, filesystem has max 5 → next should be 6
        let next = store.next_number_with_fs(DocType::Rfc, docs_path).unwrap();
        assert_eq!(next, 6);
    }

    #[test]
    fn test_next_number_with_fs_takes_max_of_both() {
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        // Add document to DB with number 10
        let mut doc = Document::new(DocType::Rfc, "DB Document", "draft");
        doc.number = Some(10);
        store.add_document(&doc).unwrap();

        // Create rfcs directory with file numbered 7
        let rfcs_dir = docs_path.join("rfcs");
        std::fs::create_dir_all(&rfcs_dir).unwrap();
        std::fs::write(rfcs_dir.join("0007-seventh.md"), "# RFC\n").unwrap();

        // DB has 10, filesystem has 7 → next should be 11 (max + 1)
        let next = store.next_number_with_fs(DocType::Rfc, docs_path).unwrap();
        assert_eq!(next, 11);
    }

    #[test]
    fn test_next_number_with_fs_filesystem_wins() {
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        // Add document to DB with number 3
        let mut doc = Document::new(DocType::Rfc, "DB Document", "draft");
        doc.number = Some(3);
        store.add_document(&doc).unwrap();

        // Create rfcs directory with files up to 20
        let rfcs_dir = docs_path.join("rfcs");
        std::fs::create_dir_all(&rfcs_dir).unwrap();
        std::fs::write(rfcs_dir.join("0020-twentieth.md"), "# RFC\n").unwrap();

        // DB has 3, filesystem has 20 → next should be 21
        let next = store.next_number_with_fs(DocType::Rfc, docs_path).unwrap();
        assert_eq!(next, 21);
    }

    #[test]
    fn test_scan_filesystem_max_skips_plan_files() {
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        let rfcs_dir = docs_path.join("rfcs");
        std::fs::create_dir_all(&rfcs_dir).unwrap();
        std::fs::write(rfcs_dir.join("0005-feature.md"), "# RFC\n").unwrap();
        std::fs::write(rfcs_dir.join("0005-feature.plan.md"), "# Plan\n").unwrap();
        std::fs::write(rfcs_dir.join("0010-big.plan.md"), "# Plan\n").unwrap();

        // Should see 5 from the .md file, ignore .plan.md files
        let max = store.scan_filesystem_max(DocType::Rfc, docs_path).unwrap();
        assert_eq!(max, 5);
    }

    #[test]
    fn test_next_number_regression_numbering_collision() {
        // Regression test for the exact bug that caused RFC 0022:
        // Files 0018 and 0019 existed on disk but not in DB.
        // next_number() returned 0018, causing a collision.
        let store = DocumentStore::open_in_memory().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let docs_path = tmp.path();

        // Simulate: DB has RFCs 1-17
        for i in 1..=17 {
            let mut doc = Document::new(DocType::Rfc, &format!("rfc-{}", i), "draft");
            doc.number = Some(i);
            store.add_document(&doc).unwrap();
        }

        // Filesystem has 1-19 (18 and 19 are untracked)
        let rfcs_dir = docs_path.join("rfcs");
        std::fs::create_dir_all(&rfcs_dir).unwrap();
        for i in 1..=19 {
            std::fs::write(
                rfcs_dir.join(format!("{:04}-rfc-{}.md", i, i)),
                format!("# RFC {:04}\n", i),
            ).unwrap();
        }

        // Old behavior: next_number() returns 18 (collision!)
        let old_next = store.next_number(DocType::Rfc).unwrap();
        assert_eq!(old_next, 18); // Bug: would collide with existing file

        // New behavior: next_number_with_fs() returns 20 (safe!)
        let new_next = store.next_number_with_fs(DocType::Rfc, docs_path).unwrap();
        assert_eq!(new_next, 20); // Correct: max(17, 19) + 1
    }

    // ==================== RFC 0031: Document Lifecycle Filename Tests ====================

    #[test]
    fn test_utc_timestamp_format() {
        let ts = crate::documents::utc_timestamp();
        let re = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{4}Z$").unwrap();
        assert!(re.is_match(&ts), "timestamp '{}' doesn't match expected format", ts);
    }

    #[test]
    fn test_status_suffix_all_types() {
        // Spike
        assert_eq!(status_suffix(DocType::Spike, "in-progress"), Some("wip"));
        assert_eq!(status_suffix(DocType::Spike, "complete"), Some("done"));
        assert_eq!(status_suffix(DocType::Spike, "resolved"), Some("resolved"));

        // RFC
        assert_eq!(status_suffix(DocType::Rfc, "draft"), Some("draft"));
        assert_eq!(status_suffix(DocType::Rfc, "accepted"), Some("accepted"));
        assert_eq!(status_suffix(DocType::Rfc, "in-progress"), Some("wip"));
        assert_eq!(status_suffix(DocType::Rfc, "implemented"), Some("impl"));
        assert_eq!(status_suffix(DocType::Rfc, "superseded"), Some("super"));

        // ADR
        assert_eq!(status_suffix(DocType::Adr, "accepted"), Some("accepted"));
        assert_eq!(status_suffix(DocType::Adr, "superseded"), Some("super"));

        // Decision
        assert_eq!(status_suffix(DocType::Decision, "recorded"), Some("recorded"));

        // PRD
        assert_eq!(status_suffix(DocType::Prd, "draft"), Some("draft"));
        assert_eq!(status_suffix(DocType::Prd, "approved"), Some("approved"));
        assert_eq!(status_suffix(DocType::Prd, "implemented"), Some("impl"));

        // Postmortem
        assert_eq!(status_suffix(DocType::Postmortem, "open"), Some("open"));
        assert_eq!(status_suffix(DocType::Postmortem, "closed"), Some("closed"));

        // Runbook
        assert_eq!(status_suffix(DocType::Runbook, "active"), Some("active"));
        assert_eq!(status_suffix(DocType::Runbook, "published"), Some("pub"));
        assert_eq!(status_suffix(DocType::Runbook, "archived"), Some("archived"));

        // Dialogue
        assert_eq!(status_suffix(DocType::Dialogue, "recorded"), Some("recorded"));
        assert_eq!(status_suffix(DocType::Dialogue, "published"), Some("pub"));

        // Audit
        assert_eq!(status_suffix(DocType::Audit, "in-progress"), Some("wip"));
        assert_eq!(status_suffix(DocType::Audit, "complete"), Some("done"));

        // Unknown status → None
        assert_eq!(status_suffix(DocType::Rfc, "unknown-status"), None);
    }

    #[test]
    fn test_rebuild_filename_simple() {
        let result = rebuild_filename(
            "spikes/2026-01-26T0856Z-my-spike.md",
            DocType::Spike,
            "complete",
        );
        assert_eq!(result, "spikes/2026-01-26T0856Z-my-spike.done.md");
    }

    #[test]
    fn test_rebuild_filename_dialogue() {
        let result = rebuild_filename(
            "dialogues/2026-01-26T0856Z-my-dialogue.dialogue.md",
            DocType::Dialogue,
            "published",
        );
        assert_eq!(result, "dialogues/2026-01-26T0856Z-my-dialogue.dialogue.pub.md");
    }

    #[test]
    fn test_rebuild_filename_strip_old() {
        // Already has a suffix — strip it and add the new one
        let result = rebuild_filename(
            "rfcs/0001-my-rfc.accepted.md",
            DocType::Rfc,
            "implemented",
        );
        assert_eq!(result, "rfcs/0001-my-rfc.impl.md");
    }

    #[test]
    fn test_rebuild_filename_strip_dialogue_old() {
        let result = rebuild_filename(
            "dialogues/2026-01-26T0856Z-slug.dialogue.pub.md",
            DocType::Dialogue,
            "recorded",
        );
        // recorded now gets .recorded suffix
        assert_eq!(result, "dialogues/2026-01-26T0856Z-slug.dialogue.recorded.md");
    }

    #[test]
    fn test_rebuild_filename_noop() {
        // draft now gets .draft suffix
        let result = rebuild_filename(
            "rfcs/0001-my-rfc.draft.md",
            DocType::Rfc,
            "draft",
        );
        assert_eq!(result, "rfcs/0001-my-rfc.draft.md");
    }

    #[test]
    fn test_rebuild_filename_remove_suffix() {
        // in-progress now gets .wip suffix
        let result = rebuild_filename(
            "spikes/2026-01-26T0856Z-spike.done.md",
            DocType::Spike,
            "in-progress",
        );
        assert_eq!(result, "spikes/2026-01-26T0856Z-spike.wip.md");
    }

    #[test]
    fn test_update_document_file_path() {
        let store = DocumentStore::open_in_memory().unwrap();
        let mut doc = Document::new(DocType::Spike, "test-spike", "in-progress");
        doc.file_path = Some("spikes/2026-01-26T0856Z-test-spike.wip.md".to_string());
        store.add_document(&doc).unwrap();

        store.update_document_file_path(
            DocType::Spike,
            "test-spike",
            "spikes/2026-01-26T0856Z-test-spike.done.md",
        ).unwrap();

        let updated = store.find_document(DocType::Spike, "test-spike").unwrap();
        assert_eq!(
            updated.file_path.as_deref(),
            Some("spikes/2026-01-26T0856Z-test-spike.done.md")
        );
    }
}
