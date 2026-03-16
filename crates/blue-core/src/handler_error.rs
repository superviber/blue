//! Handler error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HandlerError {
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Invalid params")]
    InvalidParams,

    #[error("{0}")]
    BlueNotDetected(String),

    #[error("State load failed: {0}")]
    StateLoadFailed(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Workflow error: {0}")]
    Workflow(String),
}
