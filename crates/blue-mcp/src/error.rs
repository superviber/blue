//! Server error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Parse error: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid params")]
    InvalidParams,

    #[error("Blue not detected in this directory")]
    BlueNotDetected,

    #[error("State load failed: {0}")]
    StateLoadFailed(String),

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

impl ServerError {
    /// Get JSON-RPC error code
    pub fn code(&self) -> i32 {
        match self {
            ServerError::Parse(_) => -32700,
            ServerError::MethodNotFound(_) => -32601,
            ServerError::InvalidParams => -32602,
            ServerError::ToolNotFound(_) => -32601,
            ServerError::BlueNotDetected => -32000,
            ServerError::StateLoadFailed(_) => -32001,
            ServerError::CommandFailed(_) => -32002,
            ServerError::NotFound(_) => -32003,
        }
    }
}
