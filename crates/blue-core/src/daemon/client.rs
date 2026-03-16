//! Daemon client for CLI and GUI
//!
//! Provides a typed interface to the daemon HTTP API with auto-start support.

use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, info};

use super::db::{Notification, Realm, Session};
use super::DAEMON_PORT;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Daemon not running and failed to start: {0}")]
    DaemonStartFailed(String),

    #[error("Daemon not reachable after {0} attempts")]
    DaemonUnreachable(u32),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
}

#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}

// ─── Request/Response Types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
struct SyncRequest {
    force: bool,
}

#[derive(Debug, Deserialize)]
pub struct SyncResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSessionRequest {
    pub id: String,
    pub repo: String,
    pub realm: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_rfc: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_domains: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports_modified: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports_watching: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AckRequest {
    repo: String,
}

// ─── Client ─────────────────────────────────────────────────────────────────

/// Client for communicating with the Blue daemon
#[derive(Clone)]
pub struct DaemonClient {
    client: Client,
    base_url: String,
}

impl DaemonClient {
    /// Create a new daemon client
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: format!("http://127.0.0.1:{}", DAEMON_PORT),
        }
    }

    /// Ensure daemon is running, starting it if necessary
    pub async fn ensure_running(&self) -> Result<(), ClientError> {
        // Check if daemon is already running
        if self.health().await.is_ok() {
            debug!("Daemon already running");
            return Ok(());
        }

        info!("Daemon not running, starting...");
        self.start_daemon()?;

        // Wait for daemon to become available
        let max_attempts = 10;
        for attempt in 1..=max_attempts {
            sleep(Duration::from_millis(200)).await;
            if self.health().await.is_ok() {
                info!("Daemon started successfully");
                return Ok(());
            }
            debug!("Waiting for daemon... attempt {}/{}", attempt, max_attempts);
        }

        Err(ClientError::DaemonUnreachable(max_attempts))
    }

    /// Start the daemon as a background process
    fn start_daemon(&self) -> Result<(), ClientError> {
        // Get the path to the blue binary (assumes it's in PATH or same location)
        let exe =
            std::env::current_exe().map_err(|e| ClientError::DaemonStartFailed(e.to_string()))?;

        // Start daemon in background
        let child = Command::new(&exe)
            .arg("daemon")
            .arg("start")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ClientError::DaemonStartFailed(e.to_string()))?;

        debug!("Spawned daemon process with PID {}", child.id());
        Ok(())
    }

    // ─── Health ─────────────────────────────────────────────────────────────

    /// Check daemon health
    pub async fn health(&self) -> Result<HealthResponse, ClientError> {
        self.get("/health").await
    }

    // ─── Realms ─────────────────────────────────────────────────────────────

    /// List all tracked realms
    pub async fn list_realms(&self) -> Result<Vec<Realm>, ClientError> {
        self.get("/realms").await
    }

    /// Get a specific realm
    pub async fn get_realm(&self, name: &str) -> Result<Realm, ClientError> {
        self.get(&format!("/realms/{}", name)).await
    }

    /// Trigger a sync for a realm
    pub async fn sync_realm(&self, name: &str, force: bool) -> Result<SyncResponse, ClientError> {
        self.post(&format!("/realms/{}/sync", name), &SyncRequest { force })
            .await
    }

    // ─── Sessions ───────────────────────────────────────────────────────────

    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<Session>, ClientError> {
        self.get("/sessions").await
    }

    /// Register a new session
    pub async fn create_session(&self, req: CreateSessionRequest) -> Result<Session, ClientError> {
        self.post("/sessions", &req).await
    }

    /// Deregister a session
    pub async fn remove_session(&self, id: &str) -> Result<(), ClientError> {
        self.delete(&format!("/sessions/{}", id)).await
    }

    // ─── Notifications ──────────────────────────────────────────────────────

    /// List pending notifications
    pub async fn list_notifications(&self) -> Result<Vec<Notification>, ClientError> {
        self.get("/notifications").await
    }

    /// Acknowledge a notification
    pub async fn acknowledge_notification(&self, id: i64, repo: &str) -> Result<(), ClientError> {
        self.post(
            &format!("/notifications/{}/ack", id),
            &AckRequest {
                repo: repo.to_string(),
            },
        )
        .await
    }

    // ─── HTTP Helpers ───────────────────────────────────────────────────────

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.client.get(&url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status().as_u16();
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                error: "Unknown error".to_string(),
            });
            Err(ClientError::Api {
                status,
                message: error.error,
            })
        }
    }

    async fn post<Req: Serialize, Res: DeserializeOwned>(
        &self,
        path: &str,
        body: &Req,
    ) -> Result<Res, ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.client.post(&url).json(body).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status().as_u16();
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                error: "Unknown error".to_string(),
            });
            Err(ClientError::Api {
                status,
                message: error.error,
            })
        }
    }

    async fn delete(&self, path: &str) -> Result<(), ClientError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.client.delete(&url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let error: ApiError = response.json().await.unwrap_or(ApiError {
                error: "Unknown error".to_string(),
            });
            Err(ClientError::Api {
                status,
                message: error.error,
            })
        }
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}
