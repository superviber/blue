//! MCP Server implementation
//!
//! Handles JSON-RPC requests and routes to appropriate tool handlers.

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, info};

use blue_core::{detect_blue, DocType, Document, ProjectState, Rfc};

use crate::error::ServerError;

/// Blue MCP Server state
pub struct BlueServer {
    /// Current working directory
    cwd: Option<PathBuf>,
    /// Cached project state
    state: Option<ProjectState>,
}

impl BlueServer {
    pub fn new() -> Self {
        Self {
            cwd: None,
            state: None,
        }
    }

    /// Try to load project state for the current directory
    fn ensure_state(&mut self) -> Result<&ProjectState, ServerError> {
        if self.state.is_none() {
            let cwd = self.cwd.as_ref().ok_or(ServerError::BlueNotDetected)?;
            let home = detect_blue(cwd).map_err(|_| ServerError::BlueNotDetected)?;

            // Try to get project name from the current path
            let project = home.project_name.clone().unwrap_or_else(|| "default".to_string());

            let state = ProjectState::load(home, &project)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            self.state = Some(state);
        }

        self.state.as_ref().ok_or(ServerError::BlueNotDetected)
    }

    fn ensure_state_mut(&mut self) -> Result<&mut ProjectState, ServerError> {
        if self.state.is_none() {
            let cwd = self.cwd.as_ref().ok_or(ServerError::BlueNotDetected)?;
            let home = detect_blue(cwd).map_err(|_| ServerError::BlueNotDetected)?;

            // Try to get project name from the current path
            let project = home.project_name.clone().unwrap_or_else(|| "default".to_string());

            let state = ProjectState::load(home, &project)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            self.state = Some(state);
        }

        self.state.as_mut().ok_or(ServerError::BlueNotDetected)
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
                    "description": "Create a new dialogue document with SQLite metadata. Dialogues capture agent conversations and can be linked to RFCs.",
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
                Ok(json!({
                    "active": summary.active,
                    "ready": summary.ready,
                    "stalled": summary.stalled,
                    "drafts": summary.drafts,
                    "hint": summary.hint
                }))
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
                let number = state.store.next_number(DocType::Rfc)
                    .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

                // Generate markdown
                let mut rfc = Rfc::new(title);
                if let Some(p) = problem {
                    rfc.problem = Some(p.to_string());
                }
                if let Some(s) = source_spike {
                    rfc.source_spike = Some(s.to_string());
                }

                let markdown = rfc.to_markdown(number as u32);

                // Generate filename and write file
                let filename = format!("rfcs/{:04}-{}.md", number, title);
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

        // Get tasks if any
        let tasks = if let Some(id) = doc.id {
            state.store.get_tasks(id).unwrap_or_default()
        } else {
            vec![]
        };

        let progress = if let Some(id) = doc.id {
            state.store.get_task_progress(id).ok()
        } else {
            None
        };

        Ok(json!({
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
        }))
    }

    fn handle_rfc_update_status(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;

        let title = args
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let status = args
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or(ServerError::InvalidParams)?;

        let state = self.ensure_state()?;

        // Find the document to get its file path
        let doc = state.store.find_document(DocType::Rfc, title)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Check for worktree if going to in-progress (RFC 0011)
        let has_worktree = state.has_worktree(title);
        let worktree_warning = if status == "in-progress" && !has_worktree {
            Some("No worktree exists for this RFC. Consider using blue_worktree_create for isolated development.")
        } else {
            None
        };

        // Update database
        state.store.update_document_status(DocType::Rfc, title, status)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        // Update markdown file (RFC 0008)
        let file_updated = if let Some(ref file_path) = doc.file_path {
            let full_path = state.home.docs_path.join(file_path);
            blue_core::update_markdown_status(&full_path, status).unwrap_or(false)
        } else {
            false
        };

        // Build next_action for accepted status (RFC 0011)
        let next_action = if status == "accepted" {
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
            "new_status": status,
            "file_updated": file_updated,
            "message": blue_core::voice::success(
                &format!("Updated '{}' to {}", title, status),
                None
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

        state.store.set_tasks(doc_id, &tasks)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "title": title,
            "task_count": tasks.len(),
            "message": blue_core::voice::success(
                &format!("Set {} tasks for '{}'", tasks.len(), title),
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

        // Parse task index or find by substring
        let task_index = if let Ok(idx) = task.parse::<i32>() {
            idx
        } else {
            // Find task by substring
            let tasks = state.store.get_tasks(doc_id)
                .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

            tasks.iter()
                .find(|t| t.description.to_lowercase().contains(&task.to_lowercase()))
                .map(|t| t.task_index)
                .ok_or(ServerError::InvalidParams)?
        };

        state.store.complete_task(doc_id, task_index)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        let progress = state.store.get_task_progress(doc_id)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "title": title,
            "task_index": task_index,
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

        let doc_type = args.get("doc_type").and_then(|v| v.as_str()).and_then(DocType::from_str);
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
        let doc_type = DocType::from_str(doc_type_str)
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
        let doc_type = DocType::from_str(doc_type_str)
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
            .and_then(DocType::from_str);

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
