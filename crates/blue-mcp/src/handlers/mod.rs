//! Tool handlers for Blue MCP
//!
//! Each module handles a specific document type or workflow.

pub mod adr;
// alignment module removed per RFC 0015 - Claude orchestrates via Task tool, not MCP
pub mod audit; // Health check (blue_health_check)
pub mod audit_doc; // Audit documents (blue_audit_create, etc.)
pub mod decision;
pub mod index; // Semantic index (RFC 0010)
pub mod delete;
pub mod dialogue;
pub mod dialogue_lint;
pub mod env;
pub mod guide;
pub mod lint;
pub mod llm;
pub mod playwright;
pub mod postmortem;
pub mod pr;
pub mod prd;
pub mod realm;
pub mod release;
pub mod reminder;
pub mod resources; // MCP Resources (RFC 0016)
pub mod rfc;
pub mod runbook;
pub mod session;
pub mod spike;
pub mod staging;
pub mod worktree;
