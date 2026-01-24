//! Blue MCP - Blue's voice through MCP
//!
//! Model Context Protocol server implementation.
//! Implements JSON-RPC 2.0 over stdio.

#![recursion_limit = "512"]

mod error;
mod handlers;
mod server;

pub use error::ServerError;
pub use server::BlueServer;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::info;

/// Run the MCP server
pub async fn run() -> anyhow::Result<()> {
    let mut server = BlueServer::new();

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

        let response = server.handle_request(line.trim());
        stdout.write_all(response.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}
