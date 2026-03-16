//! Daemon SQLite database
//!
//! Stores realm state, sessions, and notifications in ~/.blue/daemon.db

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaemonDbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Realm not found: {0}")]
    RealmNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),
}

/// A realm tracked by the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Realm {
    pub name: String,
    pub forgejo_url: String,
    pub local_path: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub status: RealmStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RealmStatus {
    Active,
    Syncing,
    Error,
}

impl RealmStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Syncing => "syncing",
            Self::Error => "error",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "syncing" => Self::Syncing,
            "error" => Self::Error,
            _ => Self::Active,
        }
    }
}

/// An active session (CLI or GUI instance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub repo: String,
    pub realm: String,
    pub client_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub active_rfc: Option<String>,
    pub active_domains: Vec<String>,
    pub exports_modified: Vec<String>,
    pub imports_watching: Vec<String>,
}

/// A notification for cross-repo coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub realm: String,
    pub domain: String,
    pub contract: String,
    pub from_repo: String,
    pub change_type: ChangeType,
    pub changes: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub acknowledged_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Updated,
    Breaking,
    New,
}

impl ChangeType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Breaking => "breaking",
            Self::New => "new",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "breaking" => Self::Breaking,
            "new" => Self::New,
            _ => Self::Updated,
        }
    }
}

/// Daemon database handle
///
/// Note: rusqlite::Connection is not Sync, so this must be wrapped
/// in a std::sync::Mutex (not tokio::sync::RwLock) for async contexts.
pub struct DaemonDb {
    conn: Connection,
}

// Safety: We ensure exclusive access via external synchronization (Mutex)
unsafe impl Send for DaemonDb {}

impl DaemonDb {
    /// Open or create the daemon database
    pub fn open(path: &Path) -> Result<Self, DaemonDbError> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    #[cfg(test)]
    pub fn open_memory() -> Result<Self, DaemonDbError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<(), DaemonDbError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS realms (
                name TEXT PRIMARY KEY,
                forgejo_url TEXT NOT NULL,
                local_path TEXT NOT NULL,
                last_sync TEXT,
                status TEXT DEFAULT 'active'
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                repo TEXT NOT NULL,
                realm TEXT NOT NULL,
                client_id TEXT,
                started_at TEXT NOT NULL,
                last_activity TEXT NOT NULL,
                active_rfc TEXT,
                active_domains TEXT DEFAULT '[]',
                exports_modified TEXT DEFAULT '[]',
                imports_watching TEXT DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS notifications (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                realm TEXT NOT NULL,
                domain TEXT NOT NULL,
                contract TEXT NOT NULL,
                from_repo TEXT NOT NULL,
                change_type TEXT NOT NULL,
                changes TEXT,
                created_at TEXT NOT NULL,
                acknowledged_by TEXT DEFAULT '[]'
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_realm ON sessions(realm);
            CREATE INDEX IF NOT EXISTS idx_notifications_realm ON notifications(realm);
            CREATE INDEX IF NOT EXISTS idx_notifications_created ON notifications(created_at);
            "#,
        )?;
        Ok(())
    }

    // ─── Realm Operations ───────────────────────────────────────────────────

    /// List all tracked realms
    pub fn list_realms(&self) -> Result<Vec<Realm>, DaemonDbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, forgejo_url, local_path, last_sync, status FROM realms")?;

        let realms = stmt
            .query_map([], |row| {
                Ok(Realm {
                    name: row.get(0)?,
                    forgejo_url: row.get(1)?,
                    local_path: row.get(2)?,
                    last_sync: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    status: RealmStatus::from_str(&row.get::<_, String>(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(realms)
    }

    /// Get a specific realm
    pub fn get_realm(&self, name: &str) -> Result<Option<Realm>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, forgejo_url, local_path, last_sync, status FROM realms WHERE name = ?",
        )?;

        let realm = stmt
            .query_row([name], |row| {
                Ok(Realm {
                    name: row.get(0)?,
                    forgejo_url: row.get(1)?,
                    local_path: row.get(2)?,
                    last_sync: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    status: RealmStatus::from_str(&row.get::<_, String>(4)?),
                })
            })
            .optional()?;

        Ok(realm)
    }

    /// Add or update a realm
    pub fn upsert_realm(&self, realm: &Realm) -> Result<(), DaemonDbError> {
        self.conn.execute(
            r#"
            INSERT INTO realms (name, forgejo_url, local_path, last_sync, status)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(name) DO UPDATE SET
                forgejo_url = excluded.forgejo_url,
                local_path = excluded.local_path,
                last_sync = excluded.last_sync,
                status = excluded.status
            "#,
            params![
                &realm.name,
                &realm.forgejo_url,
                &realm.local_path,
                realm.last_sync.map(|dt| dt.to_rfc3339()),
                realm.status.as_str(),
            ],
        )?;
        Ok(())
    }

    /// Remove a realm
    pub fn remove_realm(&self, name: &str) -> Result<(), DaemonDbError> {
        self.conn
            .execute("DELETE FROM realms WHERE name = ?", [name])?;
        Ok(())
    }

    // ─── Session Operations ─────────────────────────────────────────────────

    /// List all active sessions
    pub fn list_sessions(&self) -> Result<Vec<Session>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, repo, realm, client_id, started_at, last_activity,
                   active_rfc, active_domains, exports_modified, imports_watching
            FROM sessions
            "#,
        )?;

        let sessions = stmt
            .query_map([], Self::row_to_session)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// List sessions for a specific realm
    pub fn list_sessions_for_realm(&self, realm: &str) -> Result<Vec<Session>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, repo, realm, client_id, started_at, last_activity,
                   active_rfc, active_domains, exports_modified, imports_watching
            FROM sessions WHERE realm = ?
            "#,
        )?;

        let sessions = stmt
            .query_map([realm], Self::row_to_session)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    /// Get a specific session
    pub fn get_session(&self, id: &str) -> Result<Option<Session>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, repo, realm, client_id, started_at, last_activity,
                   active_rfc, active_domains, exports_modified, imports_watching
            FROM sessions WHERE id = ?
            "#,
        )?;

        let session = stmt.query_row([id], Self::row_to_session).optional()?;

        Ok(session)
    }

    /// Register a new session
    pub fn create_session(&self, session: &Session) -> Result<(), DaemonDbError> {
        self.conn.execute(
            r#"
            INSERT INTO sessions (id, repo, realm, client_id, started_at, last_activity,
                                  active_rfc, active_domains, exports_modified, imports_watching)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                &session.id,
                &session.repo,
                &session.realm,
                &session.client_id,
                session.started_at.to_rfc3339(),
                session.last_activity.to_rfc3339(),
                &session.active_rfc,
                serde_json::to_string(&session.active_domains).unwrap_or_default(),
                serde_json::to_string(&session.exports_modified).unwrap_or_default(),
                serde_json::to_string(&session.imports_watching).unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// Update session activity timestamp
    pub fn touch_session(&self, id: &str) -> Result<(), DaemonDbError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE sessions SET last_activity = ? WHERE id = ?",
            params![now, id],
        )?;
        Ok(())
    }

    /// Remove a session
    pub fn remove_session(&self, id: &str) -> Result<(), DaemonDbError> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?", [id])?;
        Ok(())
    }

    fn row_to_session(row: &rusqlite::Row) -> Result<Session, rusqlite::Error> {
        Ok(Session {
            id: row.get(0)?,
            repo: row.get(1)?,
            realm: row.get(2)?,
            client_id: row.get(3)?,
            started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            active_rfc: row.get(6)?,
            active_domains: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
            exports_modified: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
            imports_watching: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
        })
    }

    // ─── Notification Operations ────────────────────────────────────────────

    /// List pending notifications (not fully acknowledged)
    pub fn list_notifications(&self) -> Result<Vec<Notification>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, realm, domain, contract, from_repo, change_type,
                   changes, created_at, acknowledged_by
            FROM notifications
            ORDER BY created_at DESC
            "#,
        )?;

        let notifications = stmt
            .query_map([], Self::row_to_notification)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(notifications)
    }

    /// List notifications for a specific realm
    pub fn list_notifications_for_realm(
        &self,
        realm: &str,
    ) -> Result<Vec<Notification>, DaemonDbError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, realm, domain, contract, from_repo, change_type,
                   changes, created_at, acknowledged_by
            FROM notifications WHERE realm = ?
            ORDER BY created_at DESC
            "#,
        )?;

        let notifications = stmt
            .query_map([realm], Self::row_to_notification)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(notifications)
    }

    /// Create a notification
    pub fn create_notification(
        &self,
        realm: &str,
        domain: &str,
        contract: &str,
        from_repo: &str,
        change_type: ChangeType,
        changes: Option<serde_json::Value>,
    ) -> Result<i64, DaemonDbError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO notifications (realm, domain, contract, from_repo, change_type, changes, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                realm,
                domain,
                contract,
                from_repo,
                change_type.as_str(),
                changes.map(|v| v.to_string()),
                now,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Acknowledge a notification from a repo
    pub fn acknowledge_notification(&self, id: i64, repo: &str) -> Result<(), DaemonDbError> {
        // Get current acknowledged_by list
        let mut stmt = self
            .conn
            .prepare("SELECT acknowledged_by FROM notifications WHERE id = ?")?;
        let ack_json: String = stmt.query_row([id], |row| row.get(0))?;
        let mut ack_list: Vec<String> = serde_json::from_str(&ack_json).unwrap_or_default();

        // Add repo if not already acknowledged
        if !ack_list.contains(&repo.to_string()) {
            ack_list.push(repo.to_string());
            self.conn.execute(
                "UPDATE notifications SET acknowledged_by = ? WHERE id = ?",
                params![serde_json::to_string(&ack_list).unwrap_or_default(), id],
            )?;
        }

        Ok(())
    }

    /// Delete notifications older than the specified number of days
    pub fn cleanup_expired_notifications(&self, days: i64) -> Result<usize, DaemonDbError> {
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let deleted = self.conn.execute(
            "DELETE FROM notifications WHERE created_at < ?",
            params![cutoff],
        )?;
        Ok(deleted)
    }

    /// List notifications for a realm filtered by state
    /// State is determined by: pending (not acknowledged by current repo),
    /// seen (acknowledged), expired (older than 7 days)
    pub fn list_notifications_with_state(
        &self,
        realm: &str,
        current_repo: &str,
        state_filter: Option<&str>,
    ) -> Result<Vec<(Notification, String)>, DaemonDbError> {
        let notifications = self.list_notifications_for_realm(realm)?;
        let now = Utc::now();
        let expiry_days = 7;

        let with_state: Vec<(Notification, String)> = notifications
            .into_iter()
            .map(|n| {
                let age_days = (now - n.created_at).num_days();
                let state = if age_days >= expiry_days {
                    "expired"
                } else if n.acknowledged_by.contains(&current_repo.to_string()) {
                    "seen"
                } else {
                    "pending"
                };
                (n, state.to_string())
            })
            .filter(|(_, state)| match state_filter {
                Some("pending") => state == "pending",
                Some("seen") => state == "seen",
                Some("expired") => state == "expired",
                Some("all") | None => true,
                _ => true,
            })
            .collect();

        Ok(with_state)
    }

    fn row_to_notification(row: &rusqlite::Row) -> Result<Notification, rusqlite::Error> {
        Ok(Notification {
            id: row.get(0)?,
            realm: row.get(1)?,
            domain: row.get(2)?,
            contract: row.get(3)?,
            from_repo: row.get(4)?,
            change_type: ChangeType::from_str(&row.get::<_, String>(5)?),
            changes: row
                .get::<_, Option<String>>(6)?
                .and_then(|s| serde_json::from_str(&s).ok()),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            acknowledged_by: serde_json::from_str(&row.get::<_, String>(8)?).unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realm_crud() {
        let db = DaemonDb::open_memory().unwrap();

        let realm = Realm {
            name: "test-realm".to_string(),
            forgejo_url: "https://git.example.com/realms/test".to_string(),
            local_path: "/home/user/.blue/realms/test-realm".to_string(),
            last_sync: None,
            status: RealmStatus::Active,
        };

        db.upsert_realm(&realm).unwrap();

        let realms = db.list_realms().unwrap();
        assert_eq!(realms.len(), 1);
        assert_eq!(realms[0].name, "test-realm");

        let fetched = db.get_realm("test-realm").unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().forgejo_url, realm.forgejo_url);

        db.remove_realm("test-realm").unwrap();
        let realms = db.list_realms().unwrap();
        assert!(realms.is_empty());
    }

    #[test]
    fn test_session_crud() {
        let db = DaemonDb::open_memory().unwrap();

        let session = Session {
            id: "sess-123".to_string(),
            repo: "aperture".to_string(),
            realm: "letemcook".to_string(),
            client_id: Some("cli-456".to_string()),
            started_at: Utc::now(),
            last_activity: Utc::now(),
            active_rfc: Some("training-metrics".to_string()),
            active_domains: vec!["s3-access".to_string()],
            exports_modified: vec![],
            imports_watching: vec!["s3-permissions".to_string()],
        };

        db.create_session(&session).unwrap();

        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].repo, "aperture");

        db.touch_session("sess-123").unwrap();
        db.remove_session("sess-123").unwrap();

        let sessions = db.list_sessions().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_notification_crud() {
        let db = DaemonDb::open_memory().unwrap();

        let id = db
            .create_notification(
                "letemcook",
                "s3-access",
                "s3-permissions",
                "aperture",
                ChangeType::Updated,
                Some(serde_json::json!({"added": ["training-metrics/*"]})),
            )
            .unwrap();

        let notifications = db.list_notifications().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].contract, "s3-permissions");

        db.acknowledge_notification(id, "fungal").unwrap();

        let notifications = db.list_notifications().unwrap();
        assert!(notifications[0]
            .acknowledged_by
            .contains(&"fungal".to_string()));
    }
}
