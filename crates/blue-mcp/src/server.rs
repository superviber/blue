//! MCP Server implementation
//!
//! Handles JSON-RPC requests and routes to appropriate tool handlers.

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::error::ServerError;

/// Blue MCP Server state
pub struct BlueServer {
    /// Current working directory
    cwd: Option<std::path::PathBuf>,
}

impl BlueServer {
    pub fn new() -> Self {
        Self { cwd: None }
    }

    /// Handle a JSON-RPC request
    pub fn handle_request(&mut self, request: &str) -> String {
        let result = self.handle_request_inner(request);
        match result {
            Ok(response) => response,
            Err(e) => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": e.code(),
                        "message": e.to_string()
                    },
                    "id": null
                });
                serde_json::to_string(&error_response).unwrap_or_default()
            }
        }
    }

    fn handle_request_inner(&mut self, request: &str) -> Result<String, ServerError> {
        let req: JsonRpcRequest = serde_json::from_str(request)?;

        debug!("Received request: {} (id: {:?})", req.method, req.id);

        let result = match req.method.as_str() {
            "initialize" => self.handle_initialize(&req.params),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tool_call(&req.params),
            _ => Err(ServerError::MethodNotFound(req.method.clone())),
        };

        let response = match result {
            Ok(value) => json!({
                "jsonrpc": "2.0",
                "result": value,
                "id": req.id
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": e.code(),
                    "message": e.to_string()
                },
                "id": req.id
            }),
        };

        Ok(serde_json::to_string(&response)?)
    }

    /// Handle initialize request
    fn handle_initialize(&mut self, _params: &Option<Value>) -> Result<Value, ServerError> {
        info!("MCP initialize");
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "blue",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    /// Handle tools/list request
    fn handle_tools_list(&self) -> Result<Value, ServerError> {
        Ok(json!({
            "tools": [
                {
                    "name": "blue_status",
                    "description": "Get project status. Returns active work, ready items, and recommendations.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            }
                        }
                    }
                },
                {
                    "name": "blue_next",
                    "description": "Get recommended next actions based on project state.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            }
                        }
                    }
                },
                {
                    "name": "blue_rfc_create",
                    "description": "Create a new RFC (design document) for a feature.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title in kebab-case"
                            }
                        },
                        "required": ["title"]
                    }
                }
            ]
        }))
    }

    /// Handle tools/call request
    fn handle_tool_call(&mut self, params: &Option<Value>) -> Result<Value, ServerError> {
        let params = params.as_ref().ok_or(ServerError::InvalidParams)?;
        let call: ToolCallParams = serde_json::from_value(params.clone())?;

        // Extract cwd from arguments if present
        if let Some(ref args) = call.arguments {
            if let Some(cwd) = args.get("cwd").and_then(|v| v.as_str()) {
                self.cwd = Some(std::path::PathBuf::from(cwd));
            }
        }

        let result = match call.name.as_str() {
            "blue_status" => self.handle_status(&call.arguments),
            "blue_next" => self.handle_next(&call.arguments),
            "blue_rfc_create" => self.handle_rfc_create(&call.arguments),
            _ => Err(ServerError::ToolNotFound(call.name)),
        }?;

        // Wrap result in MCP tool call response format
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }]
        }))
    }

    fn handle_status(&self, _args: &Option<Value>) -> Result<Value, ServerError> {
        Ok(json!({
            "status": "success",
            "message": blue_core::voice::speak("Checking status. Give me a moment.")
        }))
    }

    fn handle_next(&self, _args: &Option<Value>) -> Result<Value, ServerError> {
        Ok(json!({
            "status": "success",
            "message": blue_core::voice::speak("Looking at what's ready. One moment.")
        }))
    }

    fn handle_rfc_create(&self, args: &Option<Value>) -> Result<Value, ServerError> {
        let title = args
            .as_ref()
            .and_then(|a| a.get("title"))
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        Ok(json!({
            "status": "success",
            "message": blue_core::voice::success(
                &format!("Created RFC '{}'", title),
                Some("Want me to help fill in the details?"),
            )
        }))
    }
}

impl Default for BlueServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    arguments: Option<Value>,
}
