//! MCP Server implementation
//!
//! Handles JSON-RPC requests and routes to appropriate tool handlers.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use blue_core::{detect_blue, DocType, Document, ProjectState, Rfc, RfcStatus, title_to_slug, validate_rfc_transition};

use crate::error::ServerError;

/// Blue MCP Server state
pub struct BlueServer {
    /// Current working directory (set explicitly via tool args)
    cwd: Option<PathBuf>,
    /// MCP root from initialize handshake (RFC 0020)
    mcp_root: Option<PathBuf>,
    /// Cached project state
    state: Option<ProjectState>,
    /// Raw initialize params (for diagnostics)
    init_params: Option<Value>,
}

impl BlueServer {
    pub fn new() -> Self {
        Self {
            cwd: None,
            mcp_root: None,
            state: None,
            init_params: None,
        }
    }

    /// Walk up directory tree to find Blue project root
    fn find_blue_root(&self) -> Option<PathBuf> {
        Self::find_blue_root_static()
    }

    /// Static version for use in contexts without &self
    fn find_blue_root_static() -> Option<PathBuf> {
        let mut dir = std::env::current_dir().ok()?;
        for _ in 0..20 {
            if dir.join(".blue").exists() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
        None
    }

    /// Build RFC 0020 "not found" error with attempted paths and guidance
    fn not_found_error(&self) -> ServerError {
        let process_cwd = std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

        let mut msg = format!("Blue project not found. Process cwd: {process_cwd}");

        if let Some(ref root) = self.mcp_root {
            msg.push_str(&format!(", mcp_root: {}", root.display()));
        }

        msg.push_str(". Run 'blue init' or pass 'cwd' parameter.");
        ServerError::BlueNotDetected(msg)
    }

    /// Try to load project state for the current directory
    ///
    /// RFC 0020 fallback chain: cwd → mcp_root → walk tree → fail with guidance
    fn ensure_state(&mut self) -> Result<&ProjectState, ServerError> {
        if self.state.is_none() {
            // RFC 0020: explicit cwd → MCP roots → walk tree → fail with guidance
            let cwd = self.cwd.clone()
                .or_else(|| self.mcp_root.clone())
                .or_else(|| self.find_blue_root())
                .ok_or_else(|| self.not_found_error())?;
            let home = detect_blue(&cwd).map_err(|_| {
                ServerError::BlueNotDetected(format!(
                    "Blue not detected in: {}. Expected .blue/ directory. Run 'blue init' or pass 'cwd' parameter.",
                    cwd.display()
                ))
            })?;

            // Try to get project name from the current path
            let project = home.project_name.clone().unwrap_or_else(|| "default".to_string());

            let state = ProjectState::load(home, &project)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            self.state = Some(state);
        }

        self.state.as_ref().ok_or_else(|| self.not_found_error())
    }

    fn ensure_state_mut(&mut self) -> Result<&mut ProjectState, ServerError> {
        if self.state.is_none() {
            // RFC 0020: explicit cwd → MCP roots → walk tree → fail with guidance
            let cwd = self.cwd.clone()
                .or_else(|| self.mcp_root.clone())
                .or_else(|| self.find_blue_root())
                .ok_or_else(|| self.not_found_error())?;
            let home = detect_blue(&cwd).map_err(|_| {
                ServerError::BlueNotDetected(format!(
                    "Blue not detected in: {}. Expected .blue/ directory. Run 'blue init' or pass 'cwd' parameter.",
                    cwd.display()
                ))
            })?;

            // Try to get project name from the current path
            let project = home.project_name.clone().unwrap_or_else(|| "default".to_string());

            let state = ProjectState::load(home, &project)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            self.state = Some(state);
        }

        let err = self.not_found_error();
        self.state.as_mut().ok_or(err)
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
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(&req.params),
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
    fn handle_initialize(&mut self, params: &Option<Value>) -> Result<Value, ServerError> {
        info!("MCP initialize with params: {:?}", params);
        self.init_params = params.clone();

        // RFC 0020: Write diagnostics for debugging
        let diag = json!({
            "init_params": params,
            "process_cwd": std::env::current_dir().ok().map(|p| p.display().to_string()),
            "mcp_root": self.mcp_root.as_ref().map(|p| p.display().to_string()),
            "blue_found_via_walk": Self::find_blue_root_static().map(|p| p.display().to_string()),
        });
        let _ = std::fs::write("/tmp/blue-mcp-diag.json", serde_json::to_string_pretty(&diag).unwrap_or_default());

        // RFC 0020: Extract roots from client capabilities (MCP spec)
        if let Some(p) = params {
            // Check for roots in clientInfo or capabilities
            if let Some(roots) = p.get("roots").and_then(|r| r.as_array()) {
                if let Some(first_root) = roots.first() {
                    if let Some(uri) = first_root.get("uri").and_then(|u| u.as_str()) {
                        // Convert file:// URI to path
                        let path = uri.strip_prefix("file://").unwrap_or(uri);
                        info!("Setting mcp_root from roots: {}", path);
                        self.mcp_root = Some(PathBuf::from(path));
                    }
                }
            }
            // Also check workspaceFolders (some clients use this)
            if self.mcp_root.is_none() {
                if let Some(folders) = p.get("workspaceFolders").and_then(|f| f.as_array()) {
                    if let Some(first) = folders.first() {
                        if let Some(uri) = first.get("uri").and_then(|u| u.as_str()) {
                            let path = uri.strip_prefix("file://").unwrap_or(uri);
                            info!("Setting mcp_root from workspaceFolders: {}", path);
                            self.mcp_root = Some(PathBuf::from(path));
                        }
                    }
                }
            }
        }

        Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {
                    "listChanged": true
                },
                "roots": {
                    "listChanged": true
                }
            },
            "serverInfo": {
                "name": "blue",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": concat!(
                "You are working with Blue, a project management and workflow tool.\n\n",
                "HOW BLUE SPEAKS — follow these patterns when writing responses:\n",
                "Do: Keep it to 2 sentences before action. Put questions at the end. ",
                "Suggest what to do next when something goes wrong. Trust the user's competence.\n",
                "Don't: Use exclamation marks in errors. Apologize for system behavior. ",
                "Hedge with \"maybe\" or \"perhaps\" or \"I think\". Over-explain.\n\n",
                "THE 14 ADRs — beliefs this project is built on (in .blue/docs/adrs/):\n",
                "0. Never Give Up  1. Purpose  2. Presence  3. Home  ",
                "4. Evidence  5. Single Source  6. Relationships  7. Integrity  ",
                "8. Honor  9. Courage  10. No Dead Code  ",
                "11. Freedom Through Constraint  12. Faith  13. Overflow\n",
                "Arc: Ground (0) → Welcome (1-3) → Integrity (4-7) → Commitment (8-10) → Flourishing (11-13)\n\n",
                "All docs live in .blue/docs/ — use blue_status to see what's happening, blue_next for what's next."
            )
        }))
    }

    /// Handle tools/list request
    fn handle_tools_list(&self) -> Result<Value, ServerError> {
        Ok(json!({
            "tools": [
                {
                    "name": "blue_status",
                    "description": "Get project status. Returns active work, ready items, stalled items, and recommendations.",
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
                            },
                            "problem": {
                                "type": "string",
                                "description": "Problem statement or summary"
                            },
                            "source_spike": {
                                "type": "string",
                                "description": "Source spike title that led to this RFC"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_rfc_get",
                    "description": "Get an RFC by title or number.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title or number"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_rfc_update_status",
                    "description": "Update an RFC's status (draft -> accepted -> in-progress -> implemented).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            },
                            "status": {
                                "type": "string",
                                "description": "New status: accepted, in-progress, implemented, or superseded",
                                "enum": ["accepted", "in-progress", "implemented", "superseded"]
                            }
                        },
                        "required": ["title", "status"]
                    }
                },
                {
                    "name": "blue_rfc_plan",
                    "description": "Create or update an implementation plan with checkboxes for an RFC.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            },
                            "tasks": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "List of implementation tasks"
                            }
                        },
                        "required": ["title", "tasks"]
                    }
                },
                {
                    "name": "blue_rfc_task_complete",
                    "description": "Mark a task as complete in an RFC plan.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            },
                            "task": {
                                "type": "string",
                                "description": "Task index (1-based) or substring to match"
                            }
                        },
                        "required": ["title", "task"]
                    }
                },
                {
                    "name": "blue_rfc_validate",
                    "description": "Check RFC status and plan completion.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_search",
                    "description": "Search documents using full-text search.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "query": {
                                "type": "string",
                                "description": "Search query"
                            },
                            "doc_type": {
                                "type": "string",
                                "description": "Filter by document type",
                                "enum": ["rfc", "spike", "adr", "decision"]
                            },
                            "limit": {
                                "type": "number",
                                "description": "Maximum results to return (default: 10)"
                            }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "blue_spike_create",
                    "description": "Start a time-boxed investigation.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Investigation title"
                            },
                            "question": {
                                "type": "string",
                                "description": "What we're trying to learn"
                            },
                            "time_box": {
                                "type": "string",
                                "description": "Time limit (e.g., '2 hours')"
                            }
                        },
                        "required": ["title", "question"]
                    }
                },
                {
                    "name": "blue_spike_complete",
                    "description": "Complete an investigation with findings.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Investigation title"
                            },
                            "outcome": {
                                "type": "string",
                                "description": "Investigation outcome",
                                "enum": ["no-action", "decision-made", "recommends-implementation"]
                            },
                            "summary": {
                                "type": "string",
                                "description": "Summary of findings"
                            }
                        },
                        "required": ["title", "outcome"]
                    }
                },
                {
                    "name": "blue_adr_create",
                    "description": "Create an Architecture Decision Record.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "ADR title (kebab-case)"
                            },
                            "rfc": {
                                "type": "string",
                                "description": "RFC title this ADR documents (must be implemented)"
                            },
                            "context": {
                                "type": "string",
                                "description": "Decision context"
                            },
                            "decision": {
                                "type": "string",
                                "description": "The decision that was made"
                            },
                            "consequences": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Consequences of the decision"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_adr_list",
                    "description": "List all ADRs with summaries.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_adr_get",
                    "description": "Get full ADR content with referenced_by information.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "number": {
                                "type": "number",
                                "description": "ADR number to retrieve"
                            }
                        },
                        "required": ["number"]
                    }
                },
                {
                    "name": "blue_adr_relevant",
                    "description": "Find relevant ADRs based on context. Uses keyword matching (AI matching when LLM available).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "context": {
                                "type": "string",
                                "description": "Context to match against (e.g., 'testing strategy', 'deleting old code')"
                            }
                        },
                        "required": ["context"]
                    }
                },
                {
                    "name": "blue_adr_audit",
                    "description": "Scan for potential ADR violations. Only checks testable ADRs (Evidence, Single Source, No Dead Code).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_decision_create",
                    "description": "Create a lightweight Decision Note.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Decision title"
                            },
                            "decision": {
                                "type": "string",
                                "description": "The decision made"
                            },
                            "rationale": {
                                "type": "string",
                                "description": "Why this decision was made"
                            },
                            "alternatives": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Alternatives that were considered"
                            }
                        },
                        "required": ["title", "decision"]
                    }
                },
                {
                    "name": "blue_worktree_create",
                    "description": "Create an isolated git worktree for RFC implementation. Use after an RFC is accepted, before starting work. Creates a feature branch and isolated directory.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title to implement"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_worktree_list",
                    "description": "List active git worktrees.",
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
                    "name": "blue_worktree_remove",
                    "description": "Remove a worktree after PR merge.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            },
                            "force": {
                                "type": "boolean",
                                "description": "Remove even if branch not merged"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_pr_create",
                    "description": "Create a PR with enforced base branch (develop, not main). Use after implementation is complete and blue_rfc_complete succeeds. If rfc is provided, title is formatted as 'RFC NNNN: Title Case Name'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "rfc": {
                                "type": "string",
                                "description": "RFC title (e.g., '0007-consistent-branch-naming'). If provided, PR title is formatted as 'RFC 0007: Consistent Branch Naming'"
                            },
                            "title": {
                                "type": "string",
                                "description": "PR title (used if rfc not provided)"
                            },
                            "base": {
                                "type": "string",
                                "description": "Base branch (default: develop)"
                            },
                            "body": {
                                "type": "string",
                                "description": "PR body (markdown)"
                            },
                            "draft": {
                                "type": "boolean",
                                "description": "Create as draft PR"
                            }
                        }
                    }
                },
                {
                    "name": "blue_pr_verify",
                    "description": "Verify test plan checkboxes in a PR.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "pr_number": {
                                "type": "number",
                                "description": "PR number (auto-detect from branch if not provided)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_pr_check_item",
                    "description": "Mark a test plan item as verified in the PR.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "pr_number": {
                                "type": "number",
                                "description": "PR number"
                            },
                            "item": {
                                "type": "string",
                                "description": "Item index (1-based) or substring to match"
                            },
                            "verified_by": {
                                "type": "string",
                                "description": "How the item was verified"
                            }
                        },
                        "required": ["item"]
                    }
                },
                {
                    "name": "blue_pr_check_approvals",
                    "description": "Check if PR has been approved by reviewers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "pr_number": {
                                "type": "number",
                                "description": "PR number"
                            }
                        }
                    }
                },
                {
                    "name": "blue_pr_merge",
                    "description": "Squash-merge a PR after verification and approval.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "pr_number": {
                                "type": "number",
                                "description": "PR number"
                            },
                            "squash": {
                                "type": "boolean",
                                "description": "Use squash merge (default: true)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_release_create",
                    "description": "Create a release with semantic versioning.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "version": {
                                "type": "string",
                                "description": "Override suggested version (e.g., '2.1.0')"
                            }
                        }
                    }
                },
                {
                    "name": "blue_session_ping",
                    "description": "Register or update session activity for an RFC.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title being worked on"
                            },
                            "action": {
                                "type": "string",
                                "description": "Session action to perform",
                                "enum": ["start", "heartbeat", "end"]
                            },
                            "session_type": {
                                "type": "string",
                                "description": "Type of work being done (default: implementation)",
                                "enum": ["implementation", "review", "testing"]
                            }
                        },
                        "required": ["title", "action"]
                    }
                },
                {
                    "name": "blue_session_list",
                    "description": "List active sessions on RFCs.",
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
                    "name": "blue_reminder_create",
                    "description": "Create a gated reminder with optional time and condition triggers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Short reminder title"
                            },
                            "context": {
                                "type": "string",
                                "description": "Additional notes/context"
                            },
                            "gate": {
                                "type": "string",
                                "description": "Condition that must be met (optional)"
                            },
                            "due_date": {
                                "type": "string",
                                "description": "Target date YYYY-MM-DD (optional)"
                            },
                            "snooze_until": {
                                "type": "string",
                                "description": "Don't show until this date YYYY-MM-DD (optional)"
                            },
                            "link_to": {
                                "type": "string",
                                "description": "RFC/spike/decision title to link"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_reminder_list",
                    "description": "List reminders with optional filters.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "status": {
                                "type": "string",
                                "description": "Filter by status (default: pending)",
                                "enum": ["pending", "snoozed", "cleared", "all"]
                            },
                            "include_future": {
                                "type": "boolean",
                                "description": "Include snoozed items not yet due (default: false)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_reminder_snooze",
                    "description": "Snooze a reminder until a specific date.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "id": {
                                "type": "number",
                                "description": "Reminder ID"
                            },
                            "title": {
                                "type": "string",
                                "description": "Or match by title (partial match)"
                            },
                            "until": {
                                "type": "string",
                                "description": "New snooze date (YYYY-MM-DD)"
                            }
                        },
                        "required": ["until"]
                    }
                },
                {
                    "name": "blue_reminder_clear",
                    "description": "Clear a reminder (mark gate as resolved).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "id": {
                                "type": "number",
                                "description": "Reminder ID"
                            },
                            "title": {
                                "type": "string",
                                "description": "Or match by title (partial match)"
                            },
                            "resolution": {
                                "type": "string",
                                "description": "How the gate was resolved"
                            }
                        }
                    }
                },
                {
                    "name": "blue_staging_lock",
                    "description": "Acquire exclusive access to a staging resource.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "resource": {
                                "type": "string",
                                "description": "Resource to lock (e.g., 'migration', 'staging-db')"
                            },
                            "locked_by": {
                                "type": "string",
                                "description": "Identifier for lock holder (RFC title or PR number)"
                            },
                            "agent_id": {
                                "type": "string",
                                "description": "Blue agent ID (from .env.isolated)"
                            },
                            "duration_minutes": {
                                "type": "number",
                                "description": "Lock duration in minutes (default 30)"
                            }
                        },
                        "required": ["resource", "locked_by"]
                    }
                },
                {
                    "name": "blue_staging_unlock",
                    "description": "Release a staging lock.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "resource": {
                                "type": "string",
                                "description": "Resource to unlock"
                            },
                            "locked_by": {
                                "type": "string",
                                "description": "Identifier that acquired the lock"
                            }
                        },
                        "required": ["resource", "locked_by"]
                    }
                },
                {
                    "name": "blue_staging_status",
                    "description": "Check staging lock status.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "resource": {
                                "type": "string",
                                "description": "Specific resource to check (omit for all locks)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_staging_cleanup",
                    "description": "Clean up expired staging resources.",
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
                    "name": "blue_staging_deployments",
                    "description": "List staging environment deployments. Shows deployed, destroyed, or expired environments. Use check_expired=true to mark expired deployments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["deployed", "destroyed", "expired"],
                                "description": "Filter by deployment status"
                            },
                            "check_expired": {
                                "type": "boolean",
                                "description": "Check for and mark expired deployments (default: false)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_health_check",
                    "description": "Check project health and find issues. Returns stalled work, missing ADRs, overdue reminders, and recommendations.",
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
                    "name": "blue_audit_create",
                    "description": "Create a new audit document (repository, security, rfc-verification, adr-adherence, or custom).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Audit title in kebab-case"
                            },
                            "audit_type": {
                                "type": "string",
                                "description": "Type of audit",
                                "enum": ["repository", "security", "rfc-verification", "adr-adherence", "custom"]
                            },
                            "scope": {
                                "type": "string",
                                "description": "What is being audited"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_audit_list",
                    "description": "List all audit documents.",
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
                    "name": "blue_audit_get",
                    "description": "Get an audit document by title.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Audit title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_audit_complete",
                    "description": "Mark an audit as complete.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Audit title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_rfc_complete",
                    "description": "Mark RFC as implemented based on plan progress. Use after completing tasks in the worktree. Requires at least 70% completion. Follow with blue_pr_create.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_worktree_cleanup",
                    "description": "Clean up after PR merge. Removes worktree, deletes local branch, and provides commands to sync.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "RFC title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                // Phase 7: PRD tools
                {
                    "name": "blue_prd_create",
                    "description": "Create a Product Requirements Document (PRD). Use when: user-facing features, business requirements, stakeholder sign-off needed.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "Feature title"
                            },
                            "problem": {
                                "type": "string",
                                "description": "What problem are users experiencing?"
                            },
                            "users": {
                                "type": "string",
                                "description": "Who are the target users?"
                            },
                            "goals": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Business goals"
                            },
                            "non_goals": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "What this feature explicitly won't do"
                            },
                            "stakeholders": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Who requested this, who benefits"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_prd_get",
                    "description": "Get the content of a PRD.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "PRD title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_prd_approve",
                    "description": "Mark PRD as approved by stakeholders. Transitions: draft -> approved.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "PRD title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_prd_complete",
                    "description": "Mark PRD as implemented after verification. Checks acceptance criteria before allowing completion.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "PRD title"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_prd_list",
                    "description": "List PRDs by status.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "status": {
                                "type": "string",
                                "description": "Filter by status (draft, approved, implemented)"
                            }
                        }
                    }
                },
                // Phase 7: Lint tool
                {
                    "name": "blue_lint",
                    "description": "Run code quality checks. Detects project type (Rust, JS, Python, CDK) and runs appropriate linters.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "check": {
                                "type": "string",
                                "description": "Specific check to run (default: all)",
                                "enum": ["format", "lint", "all"]
                            },
                            "fix": {
                                "type": "boolean",
                                "description": "Auto-fix issues where possible (default: false)"
                            }
                        }
                    }
                },
                // RFC 0018: Document sync tool
                {
                    "name": "blue_sync",
                    "description": "Reconcile database with filesystem. Scans .blue/docs/ for documents not in database and vice versa.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "doc_type": {
                                "type": "string",
                                "description": "Limit to specific document type",
                                "enum": ["rfc", "spike", "adr", "decision", "dialogue", "audit", "runbook", "postmortem", "prd"]
                            },
                            "dry_run": {
                                "type": "boolean",
                                "description": "Report drift without fixing (default: false)"
                            }
                        }
                    }
                },
                // Phase 7: Environment isolation tools
                {
                    "name": "blue_env_detect",
                    "description": "Detect external dependencies in a project. Returns S3, database, Redis, IaC configs found.",
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
                    "name": "blue_env_mock",
                    "description": "Generate isolated environment configuration. Creates .env.isolated with agent ID and mock configs.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "worktree_path": {
                                "type": "string",
                                "description": "Path to worktree (defaults to cwd)"
                            },
                            "agent_id": {
                                "type": "string",
                                "description": "Custom agent ID (auto-generated if not provided)"
                            }
                        }
                    }
                },
                // Phase 7: Onboarding guide
                {
                    "name": "blue_guide",
                    "description": "Interactive onboarding guide for new Blue users.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "action": {
                                "type": "string",
                                "description": "Guide action to perform",
                                "enum": ["start", "resume", "next", "skip", "reset", "status"]
                            },
                            "choice": {
                                "type": "string",
                                "description": "User's choice from the previous prompt"
                            }
                        }
                    }
                },
                // Phase 7: Staging IaC tools
                {
                    "name": "blue_staging_create",
                    "description": "Prepare staging environment deployment from IaC. Detects CDK/Terraform/Pulumi and generates deploy commands.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "stack": {
                                "type": "string",
                                "description": "Specific stack to deploy (defaults to all)"
                            },
                            "ttl_hours": {
                                "type": "number",
                                "description": "TTL in hours for auto-cleanup (default: 24)"
                            },
                            "dry_run": {
                                "type": "boolean",
                                "description": "Just show command without running (default: true)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_staging_destroy",
                    "description": "Destroy a staging environment. Generates destroy command for detected IaC type.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "name": {
                                "type": "string",
                                "description": "Deployment name to destroy"
                            },
                            "dry_run": {
                                "type": "boolean",
                                "description": "Just show command without executing (default: true)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_staging_cost",
                    "description": "Estimate costs for staging environment. Uses Infracost for Terraform/CDK.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "duration_hours": {
                                "type": "number",
                                "description": "Duration in hours for cost calculation (default: 24)"
                            }
                        }
                    }
                },
                // Phase 8: Dialogue tools
                {
                    "name": "blue_dialogue_lint",
                    "description": "Validate dialogue documents against the blue-dialogue-pattern. Returns weighted consistency score.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "Path to the .dialogue.md file"
                            }
                        },
                        "required": ["file_path"]
                    }
                },
                {
                    "name": "blue_extract_dialogue",
                    "description": "Extract dialogue content from spawned agent JSONL outputs.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "string",
                                "description": "Task ID (e.g., 'a6dc70c') - resolves via symlink in /tmp/claude/.../tasks/"
                            },
                            "file_path": {
                                "type": "string",
                                "description": "Absolute path to JSONL file"
                            }
                        }
                    }
                },
                {
                    "name": "blue_dialogue_create",
                    "description": "Create a new dialogue document. Pass alignment: true for multi-agent alignment dialogues (ADR 0014). When alignment is enabled, the response message contains a JUDGE PROTOCOL section — you MUST follow those instructions exactly to orchestrate the dialogue. The protocol tells you how to spawn background agents, score them, and run convergence rounds.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Dialogue title"
                            },
                            "rfc_title": {
                                "type": "string",
                                "description": "RFC title to link this dialogue to"
                            },
                            "summary": {
                                "type": "string",
                                "description": "Brief summary of the dialogue"
                            },
                            "content": {
                                "type": "string",
                                "description": "Full dialogue content"
                            },
                            "alignment": {
                                "type": "boolean",
                                "description": "Enable alignment mode — returns a judge protocol with pastry-themed expert agents"
                            },
                            "agents": {
                                "type": "integer",
                                "description": "Number of cupcake agents (alignment mode only, default 3)"
                            },
                            "model": {
                                "type": "string",
                                "description": "Model for agents: sonnet, opus, or haiku (alignment mode only, default sonnet)"
                            },
                            "sources": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "File paths agents must Read for grounding (alignment mode only)"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_dialogue_get",
                    "description": "Get a dialogue document by title.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Dialogue title or number"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_dialogue_list",
                    "description": "List all dialogue documents, optionally filtered by RFC.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "rfc_title": {
                                "type": "string",
                                "description": "Filter dialogues by RFC title"
                            }
                        }
                    }
                },
                {
                    "name": "blue_dialogue_save",
                    "description": "Extract dialogue from JSONL and save as a dialogue document with metadata.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Dialogue title"
                            },
                            "task_id": {
                                "type": "string",
                                "description": "Task ID to extract dialogue from"
                            },
                            "file_path": {
                                "type": "string",
                                "description": "Path to JSONL file (alternative to task_id)"
                            },
                            "rfc_title": {
                                "type": "string",
                                "description": "RFC title to link this dialogue to"
                            },
                            "summary": {
                                "type": "string",
                                "description": "Brief summary of the dialogue"
                            }
                        },
                        "required": ["title"]
                    }
                },
                // Phase 8: Playwright verification
                {
                    "name": "blue_playwright_verify",
                    "description": "Generate a verification plan for browser-based testing using Playwright MCP.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "task": {
                                "type": "string",
                                "description": "Description of the verification task"
                            },
                            "base_url": {
                                "type": "string",
                                "description": "Base URL for the application (e.g., 'http://localhost:3000')"
                            },
                            "path": {
                                "type": "string",
                                "description": "Specific path to navigate to (e.g., '/login')"
                            },
                            "expected_outcomes": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Expected outcomes to verify"
                            },
                            "allow_staging": {
                                "type": "boolean",
                                "description": "Allow staging URLs (default: false, only localhost allowed)"
                            }
                        },
                        "required": ["task", "base_url"]
                    }
                },
                // Phase 9: Post-mortem tools
                {
                    "name": "blue_postmortem_create",
                    "description": "Create a post-mortem document for incident tracking.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Post-mortem title"
                            },
                            "severity": {
                                "type": "string",
                                "description": "Severity level (P1, P2, P3, P4)"
                            },
                            "summary": {
                                "type": "string",
                                "description": "Brief incident summary"
                            },
                            "root_cause": {
                                "type": "string",
                                "description": "Root cause of the incident"
                            },
                            "duration": {
                                "type": "string",
                                "description": "Incident duration"
                            },
                            "impact": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Impact items"
                            }
                        },
                        "required": ["title", "severity"]
                    }
                },
                {
                    "name": "blue_postmortem_action_to_rfc",
                    "description": "Convert a post-mortem action item into an RFC with bidirectional linking.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "postmortem_title": {
                                "type": "string",
                                "description": "Title of the post-mortem"
                            },
                            "action": {
                                "type": "string",
                                "description": "Action item index (1-based) or substring to match"
                            },
                            "rfc_title": {
                                "type": "string",
                                "description": "Optional RFC title (defaults to action item text)"
                            }
                        },
                        "required": ["postmortem_title", "action"]
                    }
                },
                // Phase 9: Runbook tools
                {
                    "name": "blue_runbook_create",
                    "description": "Create a runbook document for operational procedures.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Runbook title"
                            },
                            "source_rfc": {
                                "type": "string",
                                "description": "Source RFC title to link"
                            },
                            "service_name": {
                                "type": "string",
                                "description": "Service or feature name"
                            },
                            "owner": {
                                "type": "string",
                                "description": "Owner team or person"
                            },
                            "operations": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Initial operations to document"
                            },
                            "actions": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Action tags for lookup (e.g., ['docker build', 'build image'])"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_runbook_update",
                    "description": "Update an existing runbook with new operations or troubleshooting.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": {
                                "type": "string",
                                "description": "Runbook title"
                            },
                            "add_operation": {
                                "type": "string",
                                "description": "New operation to add"
                            },
                            "add_troubleshooting": {
                                "type": "string",
                                "description": "Troubleshooting section to add"
                            }
                        },
                        "required": ["title"]
                    }
                },
                {
                    "name": "blue_runbook_lookup",
                    "description": "Find a runbook by action query. Uses word-based matching to find the best runbook for a given action like 'docker build' or 'deploy staging'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "action": {
                                "type": "string",
                                "description": "Action to look up (e.g., 'docker build', 'deploy staging')"
                            }
                        },
                        "required": ["action"]
                    }
                },
                {
                    "name": "blue_runbook_actions",
                    "description": "List all registered actions across runbooks. Use this to discover what runbooks are available.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_realm_status",
                    "description": "Get realm overview including repos, domains, contracts, and bindings. Returns pending notifications.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                {
                    "name": "blue_realm_check",
                    "description": "Validate realm contracts and bindings. Returns errors and warnings including schema-without-version changes.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "realm": {
                                "type": "string",
                                "description": "Specific realm to check (defaults to current repo's realm)"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                {
                    "name": "blue_contract_get",
                    "description": "Get contract details including schema, value, version, owner, and bindings.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "domain": {
                                "type": "string",
                                "description": "Domain name containing the contract"
                            },
                            "contract": {
                                "type": "string",
                                "description": "Contract name"
                            }
                        },
                        "required": ["cwd", "domain", "contract"]
                    }
                },
                // Phase 2: Session tools (RFC 0002)
                {
                    "name": "blue_session_start",
                    "description": "Begin a work session. Tracks active realm, repo, domains, and contracts being modified or watched. Returns session ID and context.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "active_rfc": {
                                "type": "string",
                                "description": "Optional RFC title being worked on"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                {
                    "name": "blue_session_stop",
                    "description": "End the current work session. Returns summary including duration, domains touched, and contracts modified.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                // Phase 3: Workflow tools (RFC 0002)
                {
                    "name": "blue_realm_worktree_create",
                    "description": "Create git worktrees for coordinated multi-repo development. Auto-selects domain peers (repos sharing domains) by default.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "rfc": {
                                "type": "string",
                                "description": "RFC name for branch naming (creates rfc/<name> branches)"
                            },
                            "repos": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Specific repos to create worktrees for (defaults to domain peers)"
                            }
                        },
                        "required": ["cwd", "rfc"]
                    }
                },
                {
                    "name": "blue_realm_pr_status",
                    "description": "Get PR readiness across realm repos. Shows uncommitted changes, commits ahead, and PR status for coordinated releases.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "rfc": {
                                "type": "string",
                                "description": "RFC name to check specific branches (rfc/<name>)"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                // Phase 4: Notifications (RFC 0002)
                {
                    "name": "blue_notifications_list",
                    "description": "List notifications with state filters. States: pending (unseen), seen (acknowledged), expired (7+ days old). Auto-cleans expired notifications.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory (must be in a realm repo)"
                            },
                            "state": {
                                "type": "string",
                                "enum": ["pending", "seen", "expired", "all"],
                                "description": "Filter by notification state (default: all)"
                            }
                        },
                        "required": ["cwd"]
                    }
                },
                // RFC 0005: Local LLM Integration
                {
                    "name": "blue_llm_start",
                    "description": "Start the Ollama LLM server. Manages an embedded Ollama instance or uses an external one.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "port": {
                                "type": "number",
                                "description": "Port to run on (default: 11434)"
                            },
                            "model": {
                                "type": "string",
                                "description": "Default model to use (default: qwen2.5:7b)"
                            },
                            "backend": {
                                "type": "string",
                                "enum": ["auto", "cuda", "mps", "cpu"],
                                "description": "Backend to use (default: auto)"
                            },
                            "use_external": {
                                "type": "boolean",
                                "description": "Use external Ollama instead of embedded (default: false)"
                            }
                        }
                    }
                },
                {
                    "name": "blue_llm_stop",
                    "description": "Stop the managed Ollama LLM server.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_llm_status",
                    "description": "Check LLM server status. Returns running state, version, and GPU info.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_llm_providers",
                    "description": "Show LLM provider fallback chain status. Returns availability of: Ollama (local) → API (Anthropic/OpenAI) → Keywords (always available).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_model_list",
                    "description": "List available models in the Ollama instance.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "blue_model_pull",
                    "description": "Pull a model from the Ollama registry.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Model name (e.g., 'qwen2.5:7b', 'llama3.2:3b')"
                            }
                        },
                        "required": ["name"]
                    }
                },
                {
                    "name": "blue_model_remove",
                    "description": "Remove a model from the Ollama instance.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Model name to remove"
                            }
                        },
                        "required": ["name"]
                    }
                },
                {
                    "name": "blue_model_warmup",
                    "description": "Warm up a model by loading it into memory.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Model name to warm up"
                            }
                        },
                        "required": ["name"]
                    }
                },
                // RFC 0006: Delete tools
                {
                    "name": "blue_delete",
                    "description": "Delete a document (RFC, spike, decision, etc.) with safety checks. Supports dry_run, force, and permanent options. Default is soft-delete with 7-day retention.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "doc_type": {
                                "type": "string",
                                "description": "Document type",
                                "enum": ["rfc", "spike", "adr", "decision", "prd", "postmortem", "runbook"]
                            },
                            "title": {
                                "type": "string",
                                "description": "Document title or number"
                            },
                            "dry_run": {
                                "type": "boolean",
                                "description": "Preview what would be deleted without making changes"
                            },
                            "force": {
                                "type": "boolean",
                                "description": "Skip confirmation for non-draft documents or active sessions"
                            },
                            "permanent": {
                                "type": "boolean",
                                "description": "Permanently delete (skip soft-delete retention)"
                            }
                        },
                        "required": ["doc_type", "title"]
                    }
                },
                {
                    "name": "blue_restore",
                    "description": "Restore a soft-deleted document within the 7-day retention period.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "doc_type": {
                                "type": "string",
                                "description": "Document type",
                                "enum": ["rfc", "spike", "adr", "decision", "prd", "postmortem", "runbook"]
                            },
                            "title": {
                                "type": "string",
                                "description": "Document title to restore"
                            }
                        },
                        "required": ["doc_type", "title"]
                    }
                },
                {
                    "name": "blue_deleted_list",
                    "description": "List soft-deleted documents that can be restored.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "doc_type": {
                                "type": "string",
                                "description": "Filter by document type (optional)",
                                "enum": ["rfc", "spike", "adr", "decision", "prd", "postmortem", "runbook"]
                            }
                        }
                    }
                },
                {
                    "name": "blue_purge_deleted",
                    "description": "Permanently remove soft-deleted documents older than specified days.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "days": {
                                "type": "number",
                                "description": "Documents deleted more than this many days ago will be purged (default: 7)"
                            }
                        }
                    }
                },
                // RFC 0010: Semantic Index Tools
                {
                    "name": "blue_index_status",
                    "description": "Get semantic index status. Shows indexed file count, symbol count, and prompt version.",
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
                    "name": "blue_index_search",
                    "description": "Search the semantic index. Returns files or symbols matching the query.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "query": {
                                "type": "string",
                                "description": "Search query"
                            },
                            "symbols_only": {
                                "type": "boolean",
                                "description": "Search symbols only (default: false, searches files)"
                            },
                            "limit": {
                                "type": "number",
                                "description": "Maximum results to return (default: 10)"
                            }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "blue_index_impact",
                    "description": "Analyze impact of changing a file. Shows what depends on it and its relationships.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "file": {
                                "type": "string",
                                "description": "File path to analyze"
                            }
                        },
                        "required": ["file"]
                    }
                },
                {
                    "name": "blue_index_file",
                    "description": "Index a single file with AI-generated summary, relationships, and symbols.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "file_path": {
                                "type": "string",
                                "description": "File path to index"
                            },
                            "file_hash": {
                                "type": "string",
                                "description": "Hash of file contents for staleness detection"
                            },
                            "summary": {
                                "type": "string",
                                "description": "One-sentence summary of what the file does"
                            },
                            "relationships": {
                                "type": "string",
                                "description": "Description of relationships to other files"
                            },
                            "symbols": {
                                "type": "array",
                                "description": "List of symbols in the file",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string" },
                                        "kind": { "type": "string" },
                                        "start_line": { "type": "number" },
                                        "end_line": { "type": "number" },
                                        "description": { "type": "string" }
                                    },
                                    "required": ["name", "kind"]
                                }
                            }
                        },
                        "required": ["file_path", "file_hash"]
                    }
                },
                {
                    "name": "blue_index_realm",
                    "description": "List all indexed files in the current realm.",
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
                // RFC 0017: Context Activation tools
                {
                    "name": "blue_context_status",
                    "description": "Get context injection status: session ID, active injections, staleness, and relevance graph summary.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            }
                        }
                    }
                }
            ]
        }))
    }

    // ==================== Resources Handlers (RFC 0016) ====================

    /// Handle resources/list request
    fn handle_resources_list(&mut self) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::resources::handle_resources_list(state)
    }

    /// Handle resources/read request
    fn handle_resources_read(&mut self, params: &Option<Value>) -> Result<Value, ServerError> {
        let params = params.as_ref().ok_or(ServerError::InvalidParams)?;
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;
        crate::handlers::resources::handle_resources_read(state, uri)
    }

    /// Handle tools/call request
    fn handle_tool_call(&mut self, params: &Option<Value>) -> Result<Value, ServerError> {
        let params = params.as_ref().ok_or(ServerError::InvalidParams)?;
        let call: ToolCallParams = serde_json::from_value(params.clone())?;

        // Extract cwd from arguments if present
        if let Some(ref args) = call.arguments {
            if let Some(cwd) = args.get("cwd").and_then(|v| v.as_str()) {
                self.cwd = Some(PathBuf::from(cwd));
                // Reset state when cwd changes
                self.state = None;
            }
        }

        let result = match call.name.as_str() {
            "blue_status" => self.handle_status(&call.arguments),
            "blue_next" => self.handle_next(&call.arguments),
            "blue_rfc_create" => self.handle_rfc_create(&call.arguments),
            "blue_rfc_get" => self.handle_rfc_get(&call.arguments),
            "blue_rfc_update_status" => self.handle_rfc_update_status(&call.arguments),
            "blue_rfc_plan" => self.handle_rfc_plan(&call.arguments),
            "blue_rfc_task_complete" => self.handle_rfc_task_complete(&call.arguments),
            "blue_rfc_validate" => self.handle_rfc_validate(&call.arguments),
            "blue_search" => self.handle_search(&call.arguments),
            // Phase 2: Workflow handlers
            "blue_spike_create" => self.handle_spike_create(&call.arguments),
            "blue_spike_complete" => self.handle_spike_complete(&call.arguments),
            "blue_adr_create" => self.handle_adr_create(&call.arguments),
            "blue_adr_list" => self.handle_adr_list(),
            "blue_adr_get" => self.handle_adr_get(&call.arguments),
            "blue_adr_relevant" => self.handle_adr_relevant(&call.arguments),
            "blue_adr_audit" => self.handle_adr_audit(),
            "blue_decision_create" => self.handle_decision_create(&call.arguments),
            "blue_worktree_create" => self.handle_worktree_create(&call.arguments),
            "blue_worktree_list" => self.handle_worktree_list(&call.arguments),
            "blue_worktree_remove" => self.handle_worktree_remove(&call.arguments),
            // Phase 3: PR and Release handlers
            "blue_pr_create" => self.handle_pr_create(&call.arguments),
            "blue_pr_verify" => self.handle_pr_verify(&call.arguments),
            "blue_pr_check_item" => self.handle_pr_check_item(&call.arguments),
            "blue_pr_check_approvals" => self.handle_pr_check_approvals(&call.arguments),
            "blue_pr_merge" => self.handle_pr_merge(&call.arguments),
            "blue_release_create" => self.handle_release_create(&call.arguments),
            // Phase 4: Session and Reminder handlers
            "blue_session_ping" => self.handle_session_ping(&call.arguments),
            "blue_session_list" => self.handle_session_list(&call.arguments),
            "blue_reminder_create" => self.handle_reminder_create(&call.arguments),
            "blue_reminder_list" => self.handle_reminder_list(&call.arguments),
            "blue_reminder_snooze" => self.handle_reminder_snooze(&call.arguments),
            "blue_reminder_clear" => self.handle_reminder_clear(&call.arguments),
            // Phase 5: Staging handlers
            "blue_staging_lock" => self.handle_staging_lock(&call.arguments),
            "blue_staging_unlock" => self.handle_staging_unlock(&call.arguments),
            "blue_staging_status" => self.handle_staging_status(&call.arguments),
            "blue_staging_cleanup" => self.handle_staging_cleanup(&call.arguments),
            "blue_staging_deployments" => self.handle_staging_deployments(&call.arguments),
            // Phase 6: Health check, audit documents, and completion handlers
            "blue_health_check" => self.handle_health_check(&call.arguments),
            "blue_audit_create" => self.handle_audit_create(&call.arguments),
            "blue_audit_list" => self.handle_audit_list(&call.arguments),
            "blue_audit_get" => self.handle_audit_get(&call.arguments),
            "blue_audit_complete" => self.handle_audit_complete(&call.arguments),
            "blue_rfc_complete" => self.handle_rfc_complete(&call.arguments),
            "blue_worktree_cleanup" => self.handle_worktree_cleanup(&call.arguments),
            // Phase 7: PRD handlers
            "blue_prd_create" => self.handle_prd_create(&call.arguments),
            "blue_prd_get" => self.handle_prd_get(&call.arguments),
            "blue_prd_approve" => self.handle_prd_approve(&call.arguments),
            "blue_prd_complete" => self.handle_prd_complete(&call.arguments),
            "blue_prd_list" => self.handle_prd_list(&call.arguments),
            // Phase 7: Lint handler
            "blue_lint" => self.handle_lint(&call.arguments),
            // RFC 0018: Document sync handler
            "blue_sync" => self.handle_sync(&call.arguments),
            // Phase 7: Environment handlers
            "blue_env_detect" => self.handle_env_detect(&call.arguments),
            "blue_env_mock" => self.handle_env_mock(&call.arguments),
            // Phase 7: Guide handler
            "blue_guide" => self.handle_guide(&call.arguments),
            // Phase 7: Staging IaC handlers
            "blue_staging_create" => self.handle_staging_create(&call.arguments),
            "blue_staging_destroy" => self.handle_staging_destroy(&call.arguments),
            "blue_staging_cost" => self.handle_staging_cost(&call.arguments),
            // Phase 8: Dialogue handlers
            "blue_dialogue_lint" => self.handle_dialogue_lint(&call.arguments),
            "blue_extract_dialogue" => self.handle_extract_dialogue(&call.arguments),
            "blue_dialogue_create" => self.handle_dialogue_create(&call.arguments),
            "blue_dialogue_get" => self.handle_dialogue_get(&call.arguments),
            "blue_dialogue_list" => self.handle_dialogue_list(&call.arguments),
            "blue_dialogue_save" => self.handle_dialogue_save(&call.arguments),
            // Phase 8: Playwright handler
            "blue_playwright_verify" => self.handle_playwright_verify(&call.arguments),
            // Phase 9: Post-mortem handlers
            "blue_postmortem_create" => self.handle_postmortem_create(&call.arguments),
            "blue_postmortem_action_to_rfc" => self.handle_postmortem_action_to_rfc(&call.arguments),
            // Phase 9: Runbook handlers
            "blue_runbook_create" => self.handle_runbook_create(&call.arguments),
            "blue_runbook_update" => self.handle_runbook_update(&call.arguments),
            "blue_runbook_lookup" => self.handle_runbook_lookup(&call.arguments),
            "blue_runbook_actions" => self.handle_runbook_actions(),
            // Phase 10: Realm tools (RFC 0002)
            "blue_realm_status" => self.handle_realm_status(&call.arguments),
            "blue_realm_check" => self.handle_realm_check(&call.arguments),
            "blue_contract_get" => self.handle_contract_get(&call.arguments),
            "blue_session_start" => self.handle_session_start(&call.arguments),
            "blue_session_stop" => self.handle_session_stop(&call.arguments),
            "blue_realm_worktree_create" => self.handle_realm_worktree_create(&call.arguments),
            "blue_realm_pr_status" => self.handle_realm_pr_status(&call.arguments),
            "blue_notifications_list" => self.handle_notifications_list(&call.arguments),
            // RFC 0005: LLM tools
            "blue_llm_start" => crate::handlers::llm::handle_start(&call.arguments.unwrap_or_default()),
            "blue_llm_stop" => crate::handlers::llm::handle_stop(),
            "blue_llm_status" => crate::handlers::llm::handle_status(),
            "blue_llm_providers" => crate::handlers::llm::handle_providers(),
            "blue_model_list" => crate::handlers::llm::handle_model_list(),
            "blue_model_pull" => crate::handlers::llm::handle_model_pull(&call.arguments.unwrap_or_default()),
            "blue_model_remove" => crate::handlers::llm::handle_model_remove(&call.arguments.unwrap_or_default()),
            "blue_model_warmup" => crate::handlers::llm::handle_model_warmup(&call.arguments.unwrap_or_default()),
            // RFC 0006: Delete tools
            "blue_delete" => self.handle_delete(&call.arguments),
            "blue_restore" => self.handle_restore(&call.arguments),
            "blue_deleted_list" => self.handle_deleted_list(&call.arguments),
            "blue_purge_deleted" => self.handle_purge_deleted(&call.arguments),
            // RFC 0010: Semantic Index tools
            "blue_index_status" => self.handle_index_status(),
            "blue_index_search" => self.handle_index_search(&call.arguments),
            "blue_index_impact" => self.handle_index_impact(&call.arguments),
            "blue_index_file" => self.handle_index_file(&call.arguments),
            "blue_index_realm" => self.handle_index_realm(&call.arguments),
            // RFC 0017: Context Activation tools
            "blue_context_status" => self.handle_context_status(&call.arguments),
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

    fn handle_status(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        match self.ensure_state() {
            Ok(state) => {
                let summary = state.status_summary();

                // Check for index drift across all doc types
                let mut total_drift = 0;
                let mut drift_details = serde_json::Map::new();

                for doc_type in &[DocType::Rfc, DocType::Spike, DocType::Adr, DocType::Decision] {
                    if let Ok(result) = state.store.reconcile(&state.home.docs_path, Some(*doc_type), true) {
                        if result.has_drift() {
                            total_drift += result.drift_count();
                            drift_details.insert(
                                format!("{:?}", doc_type).to_lowercase(),
                                json!({
                                    "unindexed": result.unindexed.len(),
                                    "orphaned": result.orphaned.len(),
                                    "stale": result.stale.len()
                                })
                            );
                        }
                    }
                }

                let mut response = json!({
                    "active": summary.active,
                    "ready": summary.ready,
                    "stalled": summary.stalled,
                    "drafts": summary.drafts,
                    "hint": summary.hint
                });

                if total_drift > 0 {
                    response["index_drift"] = json!({
                        "total": total_drift,
                        "by_type": drift_details,
                        "hint": "Run blue_sync to reconcile."
                    });
                }

                Ok(response)
            }
            Err(_) => {
                // Fall back to a simple message if not in a Blue project
                Ok(json!({
                    "message": blue_core::voice::error(
                        "Can't find Blue here",
                        "Run 'blue init' to set up this project"
                    ),
                    "active": [],
                    "ready": [],
                    "stalled": [],
                    "drafts": []
                }))
            }
        }
    }

    fn handle_next(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        match self.ensure_state() {
            Ok(state) => {
                let summary = state.status_summary();

                let recommendations = if !summary.stalled.is_empty() {
                    vec![format!(
                        "'{}' might be stalled. Check if work is still in progress.",
                        summary.stalled[0].title
                    )]
                } else if !summary.ready.is_empty() {
                    vec![format!(
                        "'{}' is ready to implement. Use blue_worktree_create with title='{}' to start.",
                        summary.ready[0].title, summary.ready[0].title
                    )]
                } else if !summary.drafts.is_empty() {
                    vec![format!(
                        "'{}' is in draft. Review and accept it when ready.",
                        summary.drafts[0].title
                    )]
                } else if !summary.active.is_empty() {
                    vec![format!(
                        "{} item(s) in progress. Keep at it.",
                        summary.active.len()
                    )]
                } else {
                    vec!["Nothing pressing. Good time to plan something new.".to_string()]
                };

                Ok(json!({
                    "recommendations": recommendations,
                    "hint": summary.hint
                }))
            }
            Err(_) => {
                Ok(json!({
                    "recommendations": [
                        "Run 'blue init' to set up this project first."
                    ],
                    "hint": "Can't find Blue here."
                }))
            }
        }
    }

    fn handle_rfc_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let problem = args.get("problem").and_then(|v| v.as_str());
        let source_spike = args.get("source_spike").and_then(|v| v.as_str());

        match self.ensure_state() {
            Ok(state) => {
                // Get next RFC number
                let number = state.store.next_number_with_fs(DocType::Rfc, &state.home.docs_path)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                // Generate markdown
                let mut rfc = Rfc::new(title);
                if let Some(p) = problem {
                    rfc.problem = Some(p.to_string());
                }
                if let Some(s) = source_spike {
                    // Resolve spike file path for markdown link
                    let link = if let Ok(spike_doc) = state.store.find_document(DocType::Spike, s) {
                        if let Some(ref file_path) = spike_doc.file_path {
                            format!("[{}](../{})", s, file_path)
                        } else {
                            s.to_string()
                        }
                    } else {
                        s.to_string()
                    };
                    rfc.source_spike = Some(link);
                }

                let markdown = rfc.to_markdown(number as u32);

                // Generate filename and write file
                let filename = format!("rfcs/{:04}-{}.draft.md", number, title_to_slug(title));
                let docs_path = state.home.docs_path.clone();
                let rfc_path = docs_path.join(&filename);
                if let Some(parent) = rfc_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
                }
                fs::write(&rfc_path, &markdown)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                // Create document in store with file path
                let mut doc = Document::new(DocType::Rfc, title, "draft");
                doc.number = Some(number);
                doc.file_path = Some(filename.clone());

                let id = state.store.add_document(&doc)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                Ok(json!({
                    "status": "success",
                    "id": id,
                    "number": number,
                    "title": title,
                    "file": rfc_path.display().to_string(),
                    "markdown": markdown,
                    "message": blue_core::voice::success(
                        &format!("Created RFC {:04}: '{}'", number, title),
                        Some("Want me to help fill in the details?")
                    )
                }))
            }
            Err(_) => {
                // Create RFC without persistence (just generate markdown)
                let rfc = Rfc::new(title);
                let markdown = rfc.to_markdown(1);

                Ok(json!({
                    "status": "success",
                    "number": 1,
                    "title": title,
                    "markdown": markdown,
                    "message": blue_core::voice::success(
                        &format!("Created RFC '{}'", title),
                        Some("Note: Not persisted - run 'blue init' to enable storage.")
                    )
                }))
            }
        }
    }

    fn handle_rfc_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;

        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let doc_id = doc.id;
        let rfc_number = doc.number.unwrap_or(0);

        // RFC 0017: Check if plan file exists and cache is stale - rebuild if needed
        let plan_path = blue_core::plan_file_path(&state.home.docs_path, title, rfc_number);
        let mut cache_rebuilt = false;

        if let Some(id) = doc_id {
            if plan_path.exists() {
                let cache_mtime = state.store.get_plan_cache_mtime(id)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                if blue_core::is_cache_stale(&plan_path, cache_mtime.as_deref()) {
                    // Rebuild cache from plan file
                    let plan = blue_core::read_plan_file(&plan_path)
                        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                    state.store.rebuild_tasks_from_plan(id, &plan.tasks)
                        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                    // Update cache mtime
                    let mtime = chrono::Utc::now().to_rfc3339();
                    state.store.update_plan_cache_mtime(id, &mtime)
                        .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                    cache_rebuilt = true;
                }
            }
        }

        // Get tasks if any
        let tasks = if let Some(id) = doc_id {
            state.store.get_tasks(id).unwrap_or_default()
        } else {
            vec![]
        };

        let progress = if let Some(id) = doc_id {
            state.store.get_task_progress(id).ok()
        } else {
            None
        };

        let mut response = json!({
            "id": doc.id,
            "number": doc.number,
            "title": doc.title,
            "status": doc.status,
            "file_path": doc.file_path,
            "created_at": doc.created_at,
            "updated_at": doc.updated_at,
            "tasks": tasks.iter().map(|t| json!({
                "index": t.task_index,
                "description": t.description,
                "completed": t.completed
            })).collect::<Vec<_>>(),
            "progress": progress.map(|p| json!({
                "completed": p.completed,
                "total": p.total,
                "percentage": p.percentage
            }))
        });

        // Add plan file info if it exists
        if plan_path.exists() {
            response["plan_file"] = json!(plan_path.display().to_string());
            response["_plan_uri"] = json!(format!("blue://docs/rfcs/{}/plan", rfc_number));
            response["cache_rebuilt"] = json!(cache_rebuilt);

            // RFC 0019: Include Claude Code task format for auto-creation
            let incomplete_tasks: Vec<_> = tasks.iter()
                .filter(|t| !t.completed)
                .map(|t| json!({
                    "subject": format!("💙 {}", t.description),
                    "description": format!("RFC: {}\nTask {} of {}", doc.title, t.task_index + 1, tasks.len()),
                    "activeForm": format!("Working on: {}", t.description),
                    "metadata": {
                        "blue_rfc": doc.title,
                        "blue_rfc_number": rfc_number,
                        "blue_task_index": t.task_index
                    }
                }))
                .collect();

            if !incomplete_tasks.is_empty() {
                response["claude_code_tasks"] = json!(incomplete_tasks);
            }
        }

        Ok(response)
    }

    fn handle_rfc_update_status(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let status_str = args
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;

        // Find the document to get its file path and current status
        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Parse statuses and validate transition (RFC 0014)
        let current_status = RfcStatus::parse(&doc.status)
            .map_err(|e| ServerError::Workflow(e.to_string()))?;
        let target_status = RfcStatus::parse(status_str)
            .map_err(|e| ServerError::Workflow(e.to_string()))?;

        // Validate the transition
        validate_rfc_transition(current_status, target_status)
            .map_err(|e| ServerError::Workflow(e.to_string()))?;

        // Check for worktree if going to in-progress (RFC 0011)
        let has_worktree = state.has_worktree(title);
        let worktree_warning = if status_str == "in-progress" && !has_worktree {
            Some("No worktree exists for this RFC. Consider using blue_worktree_create for isolated development.")
        } else {
            None
        };

        // Update database
        state.store.update_document_status(DocType::Rfc, title, status_str)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Rename file for new status (RFC 0031)
        let final_path = blue_core::rename_for_status(&state.home.docs_path, &state.store, &doc, status_str)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Update markdown file (RFC 0008) at effective path
        let effective_path = final_path.as_deref().or(doc.file_path.as_deref());
        let file_updated = if let Some(p) = effective_path {
            let full_path = state.home.docs_path.join(p);
            blue_core::update_markdown_status(&full_path, status_str).unwrap_or(false)
        } else {
            false
        };

        // Conversational hints guide Claude to next action (RFC 0014)
        let hint = match target_status {
            RfcStatus::Accepted => Some(
                "RFC accepted. Ask the user: 'Ready to begin implementation? \
                 I'll create a worktree and set up the environment.'"
            ),
            RfcStatus::InProgress => Some(
                "Implementation started. Work in the worktree, mark plan tasks \
                 as you complete them."
            ),
            RfcStatus::Implemented => Some(
                "Implementation complete. Ask the user: 'Ready to create a PR?'"
            ),
            RfcStatus::Superseded => Some(
                "RFC superseded. The newer RFC takes precedence."
            ),
            RfcStatus::Draft => None,
        };

        // Build next_action for accepted status (RFC 0011)
        let next_action = if status_str == "accepted" {
            Some(json!({
                "tool": "blue_worktree_create",
                "args": { "title": title },
                "hint": "Create a worktree to start implementation"
            }))
        } else {
            None
        };

        let mut response = json!({
            "status": "success",
            "title": title,
            "new_status": status_str,
            "file_updated": file_updated,
            "message": blue_core::voice::success(
                &format!("Updated '{}' to {}", title, status_str),
                hint
            )
        });

        // Add optional fields
        if let Some(action) = next_action {
            response["next_action"] = action;
        }
        if let Some(warning) = worktree_warning {
            response["warning"] = json!(warning);
        }

        Ok(response)
    }

    fn handle_rfc_plan(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let tasks: Vec<String> = args
            .get("tasks")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let state = self.ensure_state()?;

        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

        // RFC 0017: Status gating - only allow planning for accepted or in-progress RFCs
        let status_lower = doc.status.to_lowercase();
        if status_lower != "accepted" && status_lower != "in-progress" {
            return Err(ServerError::Workflow(format!(
                "RFC must be 'accepted' or 'in-progress' to create a plan (current: {})",
                doc.status
            )));
        }

        // RFC 0017: Write .plan.md file as authoritative source
        let plan_tasks: Vec<blue_core::PlanTask> = tasks
            .iter()
            .map(|desc| blue_core::PlanTask {
                description: desc.clone(),
                completed: false,
            })
            .collect();

        let plan = blue_core::PlanFile {
            rfc_title: title.to_string(),
            status: blue_core::PlanStatus::InProgress,
            updated_at: chrono::Utc::now().to_rfc3339(),
            tasks: plan_tasks.clone(),
        };

        let rfc_number = doc.number.unwrap_or(0);
        let plan_path = blue_core::plan_file_path(&state.home.docs_path, title, rfc_number);

        // Ensure parent directory exists
        if let Some(parent) = plan_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ServerError::StateLoadFailed(format!("Failed to create directory: {}", e)))?;
        }

        blue_core::write_plan_file(&plan_path, &plan)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Update SQLite cache
        state.store.set_tasks(doc_id, &tasks)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Update cache mtime
        let mtime = chrono::Utc::now().to_rfc3339();
        state.store.update_plan_cache_mtime(doc_id, &mtime)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "title": title,
            "task_count": tasks.len(),
            "plan_file": plan_path.display().to_string(),
            "message": blue_core::voice::success(
                &format!("Set {} tasks for '{}'. Plan file created.", tasks.len(), title),
                Some("Mark them complete as you go with blue_rfc_task_complete.")
            )
        }))
    }

    fn handle_rfc_task_complete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;

        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;
        let rfc_number = doc.number.unwrap_or(0);

        // RFC 0017: Check if .plan.md exists and use it as authority
        let plan_path = blue_core::plan_file_path(&state.home.docs_path, title, rfc_number);

        // Parse task index or find by substring
        let task_index = if let Ok(idx) = task.parse::<i32>() {
            idx
        } else {
            // Find task by substring - check plan file first if it exists
            if plan_path.exists() {
                let plan = blue_core::read_plan_file(&plan_path)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                plan.tasks
                    .iter()
                    .position(|t| t.description.to_lowercase().contains(&task.to_lowercase()))
                    .map(|idx| idx as i32)
                    .ok_or(ServerError::InvalidParams)?
            } else {
                // Fall back to SQLite
                let tasks = state.store.get_tasks(doc_id)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                tasks.iter()
                    .find(|t| t.description.to_lowercase().contains(&task.to_lowercase()))
                    .map(|t| t.task_index)
                    .ok_or(ServerError::InvalidParams)?
            }
        };

        // RFC 0017: Update .plan.md if it exists
        let plan_updated = if plan_path.exists() {
            let updated_plan = blue_core::update_task_in_plan(&plan_path, task_index as usize, true)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            // Rebuild SQLite cache from plan
            state.store.rebuild_tasks_from_plan(doc_id, &updated_plan.tasks)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            // Update cache mtime
            let mtime = chrono::Utc::now().to_rfc3339();
            state.store.update_plan_cache_mtime(doc_id, &mtime)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            true
        } else {
            // No plan file - update SQLite directly (legacy behavior)
            state.store.complete_task(doc_id, task_index)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;
            false
        };

        let progress = state.store.get_task_progress(doc_id)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "title": title,
            "task_index": task_index,
            "plan_updated": plan_updated,
            "progress": {
                "completed": progress.completed,
                "total": progress.total,
                "percentage": progress.percentage
            },
            "message": blue_core::voice::success(
                &format!("Task {} complete. {} of {} done ({}%)",
                    task_index, progress.completed, progress.total, progress.percentage),
                None
            )
        }))
    }

    fn handle_rfc_validate(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;

        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let doc_id = doc.id.ok_or(ServerError::InvalidParams)?;

        let progress = state.store.get_task_progress(doc_id)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let message = if progress.total == 0 {
            "No plan defined yet. Use blue_rfc_plan to add tasks.".to_string()
        } else if progress.percentage == 100 {
            format!("All {} tasks complete. Ready to mark as implemented.", progress.total)
        } else if progress.percentage >= 70 {
            format!("{}% done ({}/{}). Getting close.", progress.percentage, progress.completed, progress.total)
        } else {
            format!("{}% done ({}/{}). Keep going.", progress.percentage, progress.completed, progress.total)
        };

        Ok(json!({
            "title": doc.title,
            "status": doc.status,
            "progress": {
                "completed": progress.completed,
                "total": progress.total,
                "percentage": progress.percentage
            },
            "message": message
        }))
    }

    fn handle_search(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let doc_type = args.get("doc_type").and_then(|v| v.as_str()).and_then(DocType::parse);
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let state = self.ensure_state()?;

        // Check for adr: prefix query (RFC 0004)
        if let Some(adr_num_str) = query.strip_prefix("adr:") {
            if let Ok(adr_num) = adr_num_str.trim().parse::<i32>() {
                // Find documents that cite this ADR
                return Self::search_adr_citations(state, adr_num, limit);
            }
        }

        let results = state.store.search_documents(query, doc_type, limit)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "query": query,
            "count": results.len(),
            "results": results.iter().map(|r| json!({
                "title": r.document.title,
                "type": r.document.doc_type.as_str(),
                "status": r.document.status,
                "score": r.score
            })).collect::<Vec<_>>()
        }))
    }

    /// Search for documents citing a specific ADR (RFC 0004)
    fn search_adr_citations(state: &ProjectState, adr_num: i32, limit: usize) -> Result<Value, ServerError> {
        // Find the ADR document first
        let adrs = state.store.list_documents(DocType::Adr)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let adr_doc = adrs.into_iter().find(|d| d.number == Some(adr_num));

        let Some(adr) = adr_doc else {
            return Ok(json!({
                "query": format!("adr:{}", adr_num),
                "count": 0,
                "results": [],
                "message": format!("ADR {} not found", adr_num)
            }));
        };

        let Some(adr_id) = adr.id else {
            return Ok(json!({
                "query": format!("adr:{}", adr_num),
                "count": 0,
                "results": []
            }));
        };

        // Find documents that link to this ADR
        let query = "SELECT d.id, d.doc_type, d.title, d.status
                     FROM documents d
                     JOIN document_links l ON l.source_id = d.id
                     WHERE l.target_id = ?1
                     LIMIT ?2";

        let conn = state.store.conn();
        let mut stmt = conn.prepare(query)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let rows = stmt.query_map(rusqlite::params![adr_id, limit], |row| {
            Ok((
                row.get::<_, String>(1)?, // doc_type
                row.get::<_, String>(2)?, // title
                row.get::<_, String>(3)?, // status
            ))
        }).map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let mut results = Vec::new();
        for row in rows.flatten() {
            let (doc_type, title, status) = row;
            results.push(json!({
                "title": title,
                "type": doc_type,
                "status": status,
                "score": 1.0
            }));
        }

        Ok(json!({
            "query": format!("adr:{}", adr_num),
            "adr_title": adr.title,
            "count": results.len(),
            "results": results
        }))
    }

    // Phase 2: Workflow handlers

    fn handle_spike_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::spike::handle_create(state, args)
    }

    fn handle_spike_complete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::spike::handle_complete(state, args)
    }

    fn handle_adr_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::adr::handle_create(state, args)
    }

    fn handle_adr_list(&mut self) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::adr::handle_list(state)
    }

    fn handle_adr_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::adr::handle_get(state, args)
    }

    fn handle_adr_relevant(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::adr::handle_relevant(state, args)
    }

    fn handle_adr_audit(&mut self) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::adr::handle_audit(state)
    }

    fn handle_decision_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::decision::handle_create(state, args)
    }

    fn handle_worktree_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::worktree::handle_create(state, args)
    }

    fn handle_worktree_list(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::worktree::handle_list(state)
    }

    fn handle_worktree_remove(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::worktree::handle_remove(state, args)
    }

    // Phase 3: PR and Release handlers

    fn handle_pr_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::pr::handle_create(state, args)
    }

    fn handle_pr_verify(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::pr::handle_verify(state, args)
    }

    fn handle_pr_check_item(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::pr::handle_check_item(state, args)
    }

    fn handle_pr_check_approvals(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::pr::handle_check_approvals(state, args)
    }

    fn handle_pr_merge(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::pr::handle_merge(state, args)
    }

    fn handle_release_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::release::handle_create(state, args)
    }

    // Phase 4: Session and Reminder handlers

    fn handle_session_ping(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::session::handle_ping(state, args)
    }

    fn handle_session_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::session::handle_list(state, args)
    }

    fn handle_reminder_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::reminder::handle_create(state, args)
    }

    fn handle_reminder_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::reminder::handle_list(state, args)
    }

    fn handle_reminder_snooze(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::reminder::handle_snooze(state, args)
    }

    fn handle_reminder_clear(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::reminder::handle_clear(state, args)
    }

    // Phase 5: Staging handlers

    fn handle_staging_lock(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_lock(state, args)
    }

    fn handle_staging_unlock(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_unlock(state, args)
    }

    fn handle_staging_status(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_status(state, args)
    }

    fn handle_staging_cleanup(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_cleanup(state, args)
    }

    fn handle_staging_deployments(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_deployments(state, args)
    }

    // Phase 6: Health check, audit documents, and completion handlers

    fn handle_health_check(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::audit::handle_audit(state)
    }

    fn handle_audit_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::audit_doc::handle_create(state, args)
    }

    fn handle_audit_list(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::audit_doc::handle_list(state)
    }

    fn handle_audit_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::audit_doc::handle_get(state, args)
    }

    fn handle_audit_complete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::audit_doc::handle_complete(state, args)
    }

    fn handle_rfc_complete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::rfc::handle_complete(state, args)
    }

    fn handle_worktree_cleanup(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::worktree::handle_cleanup(state, args)
    }

    // Phase 7: PRD handlers

    fn handle_prd_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::prd::handle_create(state, args)
    }

    fn handle_prd_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::prd::handle_get(state, args)
    }

    fn handle_prd_approve(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::prd::handle_approve(state, args)
    }

    fn handle_prd_complete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::prd::handle_complete(state, args)
    }

    fn handle_prd_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::prd::handle_list(state, args)
    }

    // Phase 7: Lint handler

    fn handle_lint(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::lint::handle_lint(args, &state.home.root)
    }

    // RFC 0018: Document sync handler
    fn handle_sync(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);

        let doc_type = args.get("doc_type")
            .and_then(|v| v.as_str())
            .and_then(DocType::parse);

        let dry_run = args.get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let state = self.ensure_state()?;

        let result = state.store.reconcile(&state.home.docs_path, doc_type, dry_run)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let message = if dry_run {
            if result.has_drift() {
                blue_core::voice::info(
                    &format!(
                        "Found {} issues: {} unindexed, {} orphaned, {} stale",
                        result.drift_count(),
                        result.unindexed.len(),
                        result.orphaned.len(),
                        result.stale.len()
                    ),
                    Some("Run without --dry-run to fix.")
                )
            } else {
                blue_core::voice::success("No drift detected. Database and filesystem in sync.", None)
            }
        } else if result.added > 0 || result.updated > 0 || result.soft_deleted > 0 {
            blue_core::voice::success(
                &format!(
                    "Synced: {} added, {} updated, {} soft-deleted",
                    result.added, result.updated, result.soft_deleted
                ),
                None
            )
        } else {
            blue_core::voice::success("Already in sync.", None)
        };

        Ok(json!({
            "status": "success",
            "message": message,
            "dry_run": dry_run,
            "unindexed": result.unindexed,
            "orphaned": result.orphaned,
            "stale": result.stale,
            "added": result.added,
            "updated": result.updated,
            "soft_deleted": result.soft_deleted,
            "has_drift": result.has_drift()
        }))
    }

    // Phase 7: Environment handlers

    fn handle_env_detect(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::env::handle_detect(args, &state.home.root)
    }

    fn handle_env_mock(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::env::handle_mock(args, &state.home.root)
    }

    // Phase 7: Guide handler

    fn handle_guide(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::guide::handle_guide(args, &state.home.blue_dir)
    }

    // Phase 7: Staging IaC handlers

    fn handle_staging_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_create(args, &state.home.root)
    }

    fn handle_staging_destroy(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_destroy(args, &state.home.root)
    }

    fn handle_staging_cost(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::staging::handle_cost(args, &state.home.root)
    }

    // Phase 8: Dialogue and Playwright handlers

    fn handle_dialogue_lint(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        crate::handlers::dialogue_lint::handle_dialogue_lint(args)
    }

    fn handle_extract_dialogue(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        crate::handlers::dialogue::handle_extract_dialogue(args)
    }

    fn handle_dialogue_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::dialogue::handle_create(state, args)
    }

    fn handle_dialogue_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::dialogue::handle_get(state, args)
    }

    fn handle_dialogue_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let empty = json!({});
        let args = args.as_ref().unwrap_or(&empty);
        let state = self.ensure_state()?;
        crate::handlers::dialogue::handle_list(state, args)
    }

    fn handle_dialogue_save(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::dialogue::handle_save(state, args)
    }

    fn handle_playwright_verify(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        crate::handlers::playwright::handle_verify(args)
    }

    // Phase 9: Post-mortem handlers

    fn handle_postmortem_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::postmortem::handle_create(state, args)
    }

    fn handle_postmortem_action_to_rfc(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::postmortem::handle_action_to_rfc(state, args)
    }

    // Phase 9: Runbook handlers

    fn handle_runbook_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::runbook::handle_create(state, args)
    }

    fn handle_runbook_update(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state_mut()?;
        crate::handlers::runbook::handle_update(state, args)
    }

    fn handle_runbook_lookup(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::runbook::handle_lookup(state, args)
    }

    fn handle_runbook_actions(&mut self) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::runbook::handle_actions(state)
    }

    // Phase 10: Realm handlers (RFC 0002)

    fn handle_realm_status(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        crate::handlers::realm::handle_status(self.cwd.as_deref())
    }

    fn handle_realm_check(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let realm = args
            .as_ref()
            .and_then(|a| a.get("realm"))
            .and_then(|v| v.as_str());
        crate::handlers::realm::handle_check(self.cwd.as_deref(), realm)
    }

    fn handle_contract_get(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let domain = args
            .get("domain")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;
        let contract = args
            .get("contract")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;
        crate::handlers::realm::handle_contract_get(self.cwd.as_deref(), domain, contract)
    }

    // Phase 2: Session handlers (RFC 0002)

    fn handle_session_start(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let active_rfc = args
            .as_ref()
            .and_then(|a| a.get("active_rfc"))
            .and_then(|v| v.as_str());
        crate::handlers::realm::handle_session_start(self.cwd.as_deref(), active_rfc)
    }

    fn handle_session_stop(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        crate::handlers::realm::handle_session_stop(self.cwd.as_deref())
    }

    // Phase 3: Workflow handlers (RFC 0002)

    fn handle_realm_worktree_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let rfc = args
            .get("rfc")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;
        let repos: Option<Vec<&str>> = args
            .get("repos")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());
        crate::handlers::realm::handle_worktree_create(self.cwd.as_deref(), rfc, repos)
    }

    fn handle_realm_pr_status(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let rfc = args
            .as_ref()
            .and_then(|a| a.get("rfc"))
            .and_then(|v| v.as_str());
        crate::handlers::realm::handle_pr_status(self.cwd.as_deref(), rfc)
    }

    // Phase 4: Notifications handler (RFC 0002)

    fn handle_notifications_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let state = args
            .as_ref()
            .and_then(|a| a.get("state"))
            .and_then(|v| v.as_str());
        crate::handlers::realm::handle_notifications_list(self.cwd.as_deref(), state)
    }

    // RFC 0006: Delete handlers

    fn handle_delete(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let doc_type_str = args
            .get("doc_type")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;
        let doc_type = DocType::parse(doc_type_str)
            .ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let dry_run = args
            .get("dry_run")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let force = args
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let permanent = args
            .get("permanent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if dry_run {
            let state = self.ensure_state()?;
            crate::handlers::delete::handle_delete_dry_run(state, doc_type, title)
        } else {
            let state = self.ensure_state_mut()?;
            crate::handlers::delete::handle_delete(state, doc_type, title, force, permanent)
        }
    }

    fn handle_restore(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let doc_type_str = args
            .get("doc_type")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;
        let doc_type = DocType::parse(doc_type_str)
            .ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state_mut()?;
        crate::handlers::delete::handle_restore(state, doc_type, title)
    }

    fn handle_deleted_list(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let doc_type = args
            .as_ref()
            .and_then(|a| a.get("doc_type"))
            .and_then(|v| v.as_str())
            .and_then(DocType::parse);

        let state = self.ensure_state()?;
        crate::handlers::delete::handle_list_deleted(state, doc_type)
    }

    fn handle_purge_deleted(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let days = args
            .as_ref()
            .and_then(|a| a.get("days"))
            .and_then(|v| v.as_i64())
            .unwrap_or(7);

        let state = self.ensure_state_mut()?;
        crate::handlers::delete::handle_purge_deleted(state, days)
    }

    // RFC 0010: Semantic Index handlers

    fn handle_index_status(&mut self) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::index::handle_status(state)
    }

    fn handle_index_search(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::index::handle_search(state, args)
    }

    fn handle_index_impact(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::index::handle_impact(state, args)
    }

    fn handle_index_file(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        let state = self.ensure_state()?;
        crate::handlers::index::handle_index_file(state, args)
    }

    fn handle_index_realm(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let default_args = serde_json::json!({});
        let args = args.as_ref().unwrap_or(&default_args);
        let state = self.ensure_state()?;
        crate::handlers::index::handle_index_realm(state, args)
    }

    // RFC 0017: Context Activation handlers
    fn handle_context_status(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::resources::handle_context_status(state)
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- BlueServer construction ---

    #[test]
    fn test_new_server_fields_are_none() {
        let server = BlueServer::new();
        assert!(server.cwd.is_none());
        assert!(server.mcp_root.is_none());
        assert!(server.state.is_none());
        assert!(server.init_params.is_none());
    }

    // --- handle_initialize: roots extraction ---

    #[test]
    fn test_initialize_extracts_roots_uri() {
        let mut server = BlueServer::new();
        let params = Some(json!({
            "roots": [{"uri": "file:///home/user/project"}]
        }));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.mcp_root, Some(PathBuf::from("/home/user/project")));
        assert!(server.cwd.is_none(), "cwd must not be set from initialize");
    }

    #[test]
    fn test_initialize_extracts_workspace_folders() {
        let mut server = BlueServer::new();
        let params = Some(json!({
            "workspaceFolders": [{"uri": "file:///home/user/workspace"}]
        }));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.mcp_root, Some(PathBuf::from("/home/user/workspace")));
        assert!(server.cwd.is_none());
    }

    #[test]
    fn test_initialize_roots_takes_precedence_over_workspace_folders() {
        let mut server = BlueServer::new();
        let params = Some(json!({
            "roots": [{"uri": "file:///from/roots"}],
            "workspaceFolders": [{"uri": "file:///from/workspace"}]
        }));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.mcp_root, Some(PathBuf::from("/from/roots")));
    }

    #[test]
    fn test_initialize_strips_file_prefix() {
        let mut server = BlueServer::new();
        let params = Some(json!({
            "roots": [{"uri": "file:///some/path"}]
        }));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.mcp_root, Some(PathBuf::from("/some/path")));
    }

    #[test]
    fn test_initialize_handles_uri_without_file_prefix() {
        let mut server = BlueServer::new();
        let params = Some(json!({
            "roots": [{"uri": "/direct/path"}]
        }));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.mcp_root, Some(PathBuf::from("/direct/path")));
    }

    #[test]
    fn test_initialize_empty_roots_leaves_mcp_root_none() {
        let mut server = BlueServer::new();
        let params = Some(json!({ "roots": [] }));
        let _ = server.handle_initialize(&params);
        assert!(server.mcp_root.is_none());
    }

    #[test]
    fn test_initialize_no_roots_leaves_mcp_root_none() {
        let mut server = BlueServer::new();
        let params = Some(json!({ "clientInfo": {"name": "test"} }));
        let _ = server.handle_initialize(&params);
        assert!(server.mcp_root.is_none());
    }

    #[test]
    fn test_initialize_none_params_leaves_mcp_root_none() {
        let mut server = BlueServer::new();
        let _ = server.handle_initialize(&None);
        assert!(server.mcp_root.is_none());
    }

    #[test]
    fn test_initialize_stores_raw_params() {
        let mut server = BlueServer::new();
        let params = Some(json!({"test": "value"}));
        let _ = server.handle_initialize(&params);
        assert_eq!(server.init_params.unwrap()["test"], "value");
    }

    // --- Field isolation: cwd vs mcp_root ---

    #[test]
    fn test_cwd_and_mcp_root_are_independent() {
        let mut server = BlueServer::new();

        // Set mcp_root via initialize
        let params = Some(json!({
            "roots": [{"uri": "file:///mcp/root"}]
        }));
        let _ = server.handle_initialize(&params);

        // Set cwd as tool args would
        server.cwd = Some(PathBuf::from("/explicit/cwd"));

        // Both should exist independently
        assert_eq!(server.cwd, Some(PathBuf::from("/explicit/cwd")));
        assert_eq!(server.mcp_root, Some(PathBuf::from("/mcp/root")));
    }

    // --- ensure_state fallback chain ---

    #[test]
    fn test_ensure_state_uses_cwd_first() {
        let mut server = BlueServer::new();
        server.cwd = Some(PathBuf::from("/nonexistent/cwd"));
        server.mcp_root = Some(PathBuf::from("/nonexistent/mcp"));

        let result = server.ensure_state();
        // Should fail, but error references cwd path (first in chain)
        match result {
            Err(ServerError::BlueNotDetected(msg)) => {
                assert!(
                    msg.contains("/nonexistent/cwd"),
                    "Expected cwd path in error, got: {msg}"
                );
            }
            other => panic!("Expected BlueNotDetected with cwd path, got: {other:?}"),
        }
    }

    #[test]
    fn test_ensure_state_falls_back_to_mcp_root() {
        let mut server = BlueServer::new();
        // No cwd set, only mcp_root
        server.mcp_root = Some(PathBuf::from("/nonexistent/mcp"));

        let result = server.ensure_state();
        match result {
            Err(ServerError::BlueNotDetected(msg)) => {
                assert!(
                    msg.contains("/nonexistent/mcp"),
                    "Expected mcp_root path in error, got: {msg}"
                );
            }
            other => panic!("Expected BlueNotDetected with mcp_root path, got: {other:?}"),
        }
    }

    #[test]
    fn test_ensure_state_no_paths_falls_through_to_walk() {
        let mut server = BlueServer::new();
        // No cwd, no mcp_root — will try find_blue_root (walk-up)
        // Since tests run from within the blue project, walk-up should find .blue/
        // and ensure_state should succeed
        let result = server.ensure_state();
        // If running from within blue project, this succeeds.
        // If not, it fails with BlueNotDetected. Either is valid.
        match result {
            Ok(state) => {
                // Walk-up found the project
                assert!(!state.home.root.as_os_str().is_empty());
            }
            Err(ServerError::BlueNotDetected(_)) => {
                // Not running from within a blue project — walk-up returned None
            }
            Err(other) => panic!("Unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn test_not_found_error_includes_process_cwd() {
        let server = BlueServer::new();
        let err = server.not_found_error();
        let msg = err.to_string();
        assert!(msg.contains("Blue project not found"), "Missing lead: {msg}");
        assert!(msg.contains("Process cwd:"), "Missing process cwd: {msg}");
        assert!(msg.contains("blue init"), "Missing fix suggestion: {msg}");
    }

    #[test]
    fn test_not_found_error_includes_mcp_root_when_set() {
        let mut server = BlueServer::new();
        server.mcp_root = Some(PathBuf::from("/some/mcp/root"));
        let msg = server.not_found_error().to_string();
        assert!(msg.contains("/some/mcp/root"), "Missing mcp_root in error: {msg}");
    }

    #[test]
    fn test_detect_blue_failure_shows_path_and_guidance() {
        let mut server = BlueServer::new();
        server.cwd = Some(PathBuf::from("/nonexistent/no-blue-here"));

        let result = server.ensure_state();
        match result {
            Err(ServerError::BlueNotDetected(msg)) => {
                assert!(msg.contains("/nonexistent/no-blue-here"), "Missing attempted path: {msg}");
                assert!(msg.contains(".blue/"), "Missing expected dir: {msg}");
                assert!(msg.contains("blue init"), "Missing fix suggestion: {msg}");
            }
            other => panic!("Expected BlueNotDetected, got: {other:?}"),
        }
    }

    // --- find_blue_root_static ---

    #[test]
    fn test_find_blue_root_static_returns_dir_with_blue() {
        // When running from within the blue project, should find .blue/
        if let Some(root) = BlueServer::find_blue_root_static() {
            assert!(
                root.join(".blue").exists(),
                "Found root {} but .blue/ doesn't exist there",
                root.display()
            );
        }
        // If not in a blue project, None is fine — no assertion needed
    }

    // --- Full request/response integration ---

    #[test]
    fn test_initialize_request_returns_capabilities() {
        let mut server = BlueServer::new();
        let request = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "roots": [{"uri": "file:///test/project"}]
            },
            "id": 1
        });

        let response_str = server.handle_request(&serde_json::to_string(&request).unwrap());
        let response: Value = serde_json::from_str(&response_str).unwrap();

        assert_eq!(response["result"]["protocolVersion"], "2024-11-05");
        assert!(response["result"]["capabilities"]["tools"].is_object());
        assert_eq!(response["result"]["serverInfo"]["name"], "blue");
        assert_eq!(server.mcp_root, Some(PathBuf::from("/test/project")));
    }

    #[test]
    fn test_tool_call_sets_cwd_not_mcp_root() {
        let mut server = BlueServer::new();

        // Initialize with roots first
        let init = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": { "roots": [{"uri": "file:///mcp/root"}] },
            "id": 1
        });
        server.handle_request(&serde_json::to_string(&init).unwrap());

        // Tool call with cwd arg
        let call = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "blue_status",
                "arguments": {"cwd": "/tool/cwd"}
            },
            "id": 2
        });
        server.handle_request(&serde_json::to_string(&call).unwrap());

        // cwd set from tool arg, mcp_root preserved from initialize
        assert_eq!(server.cwd, Some(PathBuf::from("/tool/cwd")));
        assert_eq!(server.mcp_root, Some(PathBuf::from("/mcp/root")));
    }
}
