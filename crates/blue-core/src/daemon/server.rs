//! Daemon HTTP server
//!
//! Runs on localhost:7865 and provides the API for CLI and GUI clients.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use super::db::{DaemonDb, Notification, Realm, Session};
use super::paths::DaemonPaths;
use super::DAEMON_PORT;

/// Shared daemon state
pub struct DaemonState {
    /// Database wrapped in Mutex (rusqlite::Connection is not Sync)
    pub db: Mutex<DaemonDb>,
    pub paths: DaemonPaths,
}

impl DaemonState {
    pub fn new(db: DaemonDb, paths: DaemonPaths) -> Self {
        Self {
            db: Mutex::new(db),
            paths,
        }
    }
}

type AppState = Arc<DaemonState>;

/// Run the daemon HTTP server
pub async fn run_daemon(state: DaemonState) -> anyhow::Result<()> {
    let state = Arc::new(state);
    let app = create_router(state);

    let addr = format!("127.0.0.1:{}", DAEMON_PORT);
    info!("Blue daemon starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn create_router(state: AppState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health))
        // Realms
        .route("/realms", get(list_realms))
        .route("/realms/{name}", get(get_realm))
        .route("/realms/{name}/sync", post(sync_realm))
        // Sessions
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", delete(remove_session))
        // Notifications
        .route("/notifications", get(list_notifications))
        .route("/notifications/{id}/ack", post(acknowledge_notification))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ─── Health ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: super::DAEMON_VERSION,
    })
}

// ─── Realms ─────────────────────────────────────────────────────────────────

async fn list_realms(State(state): State<AppState>) -> Result<Json<Vec<Realm>>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    let realms = db.list_realms()?;
    Ok(Json(realms))
}

async fn get_realm(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Realm>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    let realm = db.get_realm(&name)?.ok_or(AppError::NotFound)?;
    Ok(Json(realm))
}

#[derive(Deserialize, Default)]
struct SyncRealmRequest {
    #[serde(default)]
    force: bool,
}

#[derive(Serialize)]
struct SyncRealmResponse {
    status: &'static str,
    message: String,
}

async fn sync_realm(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: Option<Json<SyncRealmRequest>>,
) -> Result<Json<SyncRealmResponse>, AppError> {
    let req = body.map(|b| b.0).unwrap_or_default();

    let realm = {
        let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
        db.get_realm(&name)?.ok_or(AppError::NotFound)?
    };

    // TODO: Implement actual git sync via git2
    info!(
        realm = %name,
        force = req.force,
        "Sync requested for realm"
    );

    Ok(Json(SyncRealmResponse {
        status: "ok",
        message: format!("Sync initiated for realm '{}'", realm.name),
    }))
}

// ─── Sessions ───────────────────────────────────────────────────────────────

async fn list_sessions(State(state): State<AppState>) -> Result<Json<Vec<Session>>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    let sessions = db.list_sessions()?;
    Ok(Json(sessions))
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    id: String,
    repo: String,
    realm: String,
    client_id: Option<String>,
    active_rfc: Option<String>,
    #[serde(default)]
    active_domains: Vec<String>,
    #[serde(default)]
    exports_modified: Vec<String>,
    #[serde(default)]
    imports_watching: Vec<String>,
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Session>), AppError> {
    let now = chrono::Utc::now();
    let session = Session {
        id: req.id,
        repo: req.repo,
        realm: req.realm,
        client_id: req.client_id,
        started_at: now,
        last_activity: now,
        active_rfc: req.active_rfc,
        active_domains: req.active_domains,
        exports_modified: req.exports_modified,
        imports_watching: req.imports_watching,
    };

    {
        let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
        db.create_session(&session)?;
    }

    info!(session_id = %session.id, repo = %session.repo, "Session registered");
    Ok((StatusCode::CREATED, Json(session)))
}

async fn remove_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    db.remove_session(&id)?;
    info!(session_id = %id, "Session deregistered");
    Ok(StatusCode::NO_CONTENT)
}

// ─── Notifications ──────────────────────────────────────────────────────────

async fn list_notifications(
    State(state): State<AppState>,
) -> Result<Json<Vec<Notification>>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    let notifications = db.list_notifications()?;
    Ok(Json(notifications))
}

#[derive(Deserialize)]
struct AcknowledgeRequest {
    repo: String,
}

async fn acknowledge_notification(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<AcknowledgeRequest>,
) -> Result<StatusCode, AppError> {
    let db = state.db.lock().map_err(|_| AppError::LockPoisoned)?;
    db.acknowledge_notification(id, &req.repo)?;
    info!(notification_id = id, repo = %req.repo, "Notification acknowledged");
    Ok(StatusCode::OK)
}

// ─── Error Handling ─────────────────────────────────────────────────────────

#[derive(Debug)]
enum AppError {
    NotFound,
    Database(super::db::DaemonDbError),
    LockPoisoned,
}

impl From<super::db::DaemonDbError> for AppError {
    fn from(err: super::db::DaemonDbError) -> Self {
        AppError::Database(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            error: String,
        }

        let (status, message) = match self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "Not found".to_string()),
            AppError::Database(err) => {
                warn!("Database error: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
            }
            AppError::LockPoisoned => {
                warn!("Lock poisoned");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        let db = DaemonDb::open_memory().unwrap();
        let paths = DaemonPaths::new().unwrap();
        Arc::new(DaemonState::new(db, paths))
    }

    #[tokio::test]
    async fn test_health() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_realms_empty() {
        let app = create_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/realms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
