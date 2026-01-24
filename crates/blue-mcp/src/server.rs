//! MCP Server implementation
//!
//! Handles JSON-RPC requests and routes to appropriate tool handlers.

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
                    "description": "Create an isolated git worktree for RFC implementation.",
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
                    "description": "Create a PR with enforced base branch (develop, not main).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "cwd": {
                                "type": "string",
                                "description": "Current working directory"
                            },
                            "title": {
                                "type": "string",
                                "description": "PR title"
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
                        },
                        "required": ["title"]
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
                    "name": "blue_audit",
                    "description": "Check project health and find issues. Returns stalled work, missing ADRs, and recommendations.",
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
                    "name": "blue_rfc_complete",
                    "description": "Mark RFC as implemented based on plan progress. Requires at least 70% completion.",
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
            // Phase 6: Audit and completion handlers
            "blue_audit" => self.handle_audit(&call.arguments),
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
            // Phase 8: Playwright handler
            "blue_playwright_verify" => self.handle_playwright_verify(&call.arguments),
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
                        "'{}' is ready to implement. Run 'blue worktree create {}' to start.",
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

                // Create document in store
                let mut doc = Document::new(DocType::Rfc, title, "draft");
                doc.number = Some(number);

                let id = state.store.add_document(&doc)
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

                Ok(json!({
                    "status": "success",
                    "id": id,
                    "number": number,
                    "title": title,
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

        state.store.update_document_status(DocType::Rfc, title, status)
            .map_err(|e| ServerError::StateLoadFailed(e.to_string()))?;

        Ok(json!({
            "status": "success",
            "title": title,
            "new_status": status,
            "message": blue_core::voice::success(
                &format!("Updated '{}' to {}", title, status),
                None
            )
        }))
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

    // Phase 6: Audit and completion handlers

    fn handle_audit(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
        let state = self.ensure_state()?;
        crate::handlers::audit::handle_audit(state)
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
        crate::handlers::guide::handle_guide(args, &state.home.data_path)
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

    fn handle_playwright_verify(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
        let args = args.as_ref().ok_or(ServerError::InvalidParams)?;
        crate::handlers::playwright::handle_verify(args)
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
