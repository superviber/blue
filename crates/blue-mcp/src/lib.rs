//! Blue MCP - Blue's voice through MCP
//!
//! Model Context Protocol server implementation.
//! Implements JSON-RPC 2.0 over stdio.

#![recursion_limit = "512"]

mod error;
pub mod handlers;
mod server;

pub use error::ServerError;
pub use server::BlueServer;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::info;

/// Run the MCP server
pub async fn run() -> anyhow::Result<()> {
    let server = std::sync::Arc::new(std::sync::Mutex::new(BlueServer::new()));

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);

    info!("Blue MCP server started");

    let mut line = String::new();
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            break; // EOF
        }

        // Run blocking handlers in spawn_blocking to avoid tokio runtime conflicts
        let request = line.trim().to_string();
        let server_clone = server.clone();
        let response = tokio::task::spawn_blocking(move || {
            let mut server = server_clone.lock().unwrap();
            server.handle_request(&request)
        })
        .await?;

        stdout.write_all(response.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}
