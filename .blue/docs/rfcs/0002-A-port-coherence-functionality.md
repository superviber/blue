# RFC 0002: Port Coherence Functionality

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-24 |
| **Source Spike** | blue-feature-gap-analysis |

---

## Summary

Port the essential functionality from coherence-mcp to blue, maintaining Blue's voice and philosophy while gaining the workflow capabilities that make coherence useful.

## Problem

Blue currently has ~2% of coherence-mcp's functionality:
- 3 MCP tools (vs 35)
- 0 handler modules (vs 28)
- ~50 LOC core (vs 8,796)
- 0 database tables (vs 14)

Without these capabilities, blue can't manage RFCs, track work, or provide meaningful project status.

## Proposal

Port coherence-mcp in 4 phases, adapting to Blue's voice throughout.

### Phase 1: Foundation

**Goal:** Basic document management and persistence.

| Component | Source | Target | Notes |
|-----------|--------|--------|-------|
| store.rs | alignment-core | blue-core | Rename tables, Blue voice in errors |
| documents.rs | alignment-core | blue-core | Keep existing Blue structs, add methods |
| state.rs | alignment-core | blue-core | ProjectState → BlueState |
| repo.rs | alignment-core | blue-core | detect_alignment → detect_blue |
| workflow.rs | alignment-core | blue-core | Status transitions |

**Database schema:**
- documents, document_links, tasks, worktrees, metadata
- FTS5 for search
- Schema version 1 (fresh start, not migrating from coherence)

**MCP tools (Phase 1):**
- blue_status
- blue_next
- blue_rfc_create
- blue_rfc_get
- blue_rfc_update_status
- blue_rfc_plan
- blue_rfc_validate
- blue_rfc_complete
- blue_rfc_task_complete

### Phase 2: Workflow

**Goal:** Full RFC/Spike/ADR lifecycle + PR workflow.

| Component | Source | Target |
|-----------|--------|--------|
| rfc.rs | handlers | blue-mcp/handlers |
| spike.rs | handlers | blue-mcp/handlers |
| adr.rs | handlers | blue-mcp/handlers |
| decision.rs | handlers | blue-mcp/handlers |
| worktree.rs | handlers | blue-mcp/handlers |
| pr.rs | handlers | blue-mcp/handlers |
| release.rs | handlers | blue-mcp/handlers |
| search.rs | handlers | blue-mcp/handlers |

**MCP tools (Phase 2):**
- blue_spike_create, blue_spike_complete
- blue_adr_create, blue_decision_create
- blue_worktree_create, blue_worktree_list, blue_worktree_remove
- blue_pr_create, blue_pr_verify, blue_pr_check_item, blue_pr_merge
- blue_release_create
- blue_search

### Phase 3: Advanced

**Goal:** Multi-agent coordination and reminders.

| Component | Source | Target |
|-----------|--------|--------|
| staging.rs | handlers | blue-mcp/handlers |
| reminder.rs | handlers | blue-mcp/handlers |
| session.rs | handlers | blue-mcp/handlers |
| env.rs | handlers | blue-mcp/handlers |

**Database additions:**
- staging_locks, staging_lock_queue, staging_deployments
- active_sessions
- reminders

### Phase 4: Specialized

**Goal:** Code intelligence and quality tools.

| Component | Source | Target |
|-----------|--------|--------|
| code_store.rs | alignment-core | blue-core |
| symbol_extractor.rs | alignment-core | blue-core |
| lint.rs | handlers | blue-mcp/handlers |
| audit.rs | handlers | blue-mcp/handlers |
| guide.rs | handlers | blue-mcp/handlers |

## What NOT to Port

- **Parked/Gated items** - Half-implemented in coherence, skip for now
- **Post-mortems/Runbooks** - Low usage, add later if needed
- **Dialogue tools** - Specialized, port only if needed
- **Infrastructure indexing** - Complex, defer to Phase 5

## Blue's Voice Adaptation

All ported code must speak as Blue:

```rust
// Coherence style
return Err(ServerError::AlignmentNotDetected);

// Blue style
return Err(ServerError::NotHome("Can't find Blue here. Run 'blue init' first?"));
```

```rust
// Coherence message
"RFC created successfully"

// Blue message
"Created RFC '{}'. Want me to help fill in the details?"
```

## Directory Structure After Port

```
blue/
├── crates/
│   └── blue-core/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── documents.rs    # Document types
│       │   ├── store.rs        # SQLite persistence
│       │   ├── state.rs        # Project state
│       │   ├── repo.rs         # Git operations
│       │   ├── workflow.rs     # Status transitions
│       │   ├── voice.rs        # Blue's tone (existing)
│       │   └── search.rs       # FTS5 search
│       └── Cargo.toml
│   └── blue-mcp/
│       ├── src/
│       │   ├── lib.rs
│       │   ├── server.rs       # MCP server
│       │   ├── error.rs        # Error types
│       │   ├── tools.rs        # Tool definitions
│       │   └── handlers/
│       │       ├── mod.rs
│       │       ├── rfc.rs
│       │       ├── spike.rs
│       │       ├── adr.rs
│       │       ├── worktree.rs
│       │       ├── pr.rs
│       │       ├── search.rs
│       │       └── ...
│       └── Cargo.toml
└── apps/
    └── blue-cli/
```

## Goals

1. Feature parity with coherence-mcp core workflow
2. Blue's voice and philosophy throughout
3. Fresh schema (no migration baggage)
4. Cleaner code structure from lessons learned

## Non-Goals

1. 100% feature parity (skip rarely-used features)
2. Backward compatibility with coherence databases
3. Supporting both alignment_ and blue_ tool names

## Implementation Progress

### Phase 1: Foundation - COMPLETE

- [x] store.rs - SQLite persistence with schema v1, WAL mode, FTS5 search
- [x] documents.rs - Rfc, Spike, Adr, Decision with markdown generation
- [x] state.rs - ProjectState with active/ready/stalled/draft items
- [x] repo.rs - detect_blue(), worktree operations
- [x] workflow.rs - RfcStatus, SpikeOutcome, transitions
- [x] 9 MCP tools: blue_status, blue_next, blue_rfc_create, blue_rfc_get,
      blue_rfc_update_status, blue_rfc_plan, blue_rfc_validate,
      blue_rfc_task_complete, blue_search
- [x] 14 unit tests passing
- [x] Blue's voice in all error messages

### Phase 2: Workflow - COMPLETE

- [x] handlers/spike.rs - Spike create/complete with RFC enforcement
- [x] handlers/adr.rs - ADR creation with RFC linking
- [x] handlers/decision.rs - Lightweight Decision Notes
- [x] handlers/worktree.rs - Git worktree operations
- [x] 7 new MCP tools: blue_spike_create, blue_spike_complete,
      blue_adr_create, blue_decision_create, blue_worktree_create,
      blue_worktree_list, blue_worktree_remove
- [x] Total: 16 MCP tools, 842 new lines of code
- [x] Blue's voice in all error messages

### Phase 3: PR and Release - COMPLETE

- [x] handlers/pr.rs - PR create, verify, check_item, check_approvals, merge
- [x] handlers/release.rs - Semantic versioning release creation
- [x] 6 new MCP tools: blue_pr_create, blue_pr_verify, blue_pr_check_item,
      blue_pr_check_approvals, blue_pr_merge, blue_release_create
- [x] Total: 22 MCP tools
- [x] Blue's voice in all error messages
- [x] 16 tests passing

### Phase 4: Session and Reminders - COMPLETE

- [x] store.rs - Added session and reminder tables, schema v2
- [x] handlers/session.rs - Session ping (start/heartbeat/end) + list
- [x] handlers/reminder.rs - Reminder CRUD with gates, snoozing, clearing
- [x] voice.rs - Added info() function for informational messages
- [x] 6 new MCP tools: blue_session_ping, blue_session_list,
      blue_reminder_create, blue_reminder_list, blue_reminder_snooze,
      blue_reminder_clear
- [x] Total: 28 MCP tools
- [x] Blue's voice in all error messages
- [x] 21 tests passing

### Phase 5: Staging Locks - COMPLETE

- [x] store.rs - Added staging_locks and staging_lock_queue tables
- [x] handlers/staging.rs - Lock/unlock/status/cleanup for multi-agent coordination
- [x] 4 new MCP tools: blue_staging_lock, blue_staging_unlock,
      blue_staging_status, blue_staging_cleanup
- [x] Total: 32 MCP tools
- [x] Blue's voice in all error messages
- [x] 24 tests passing

### Phase 6: Audit and Completion - COMPLETE

- [x] handlers/audit.rs - Project health check with issues and recommendations
- [x] handlers/rfc.rs - RFC completion with progress validation
- [x] handlers/worktree.rs - Added cleanup handler for post-merge workflow
- [x] 3 new MCP tools: blue_audit, blue_rfc_complete, blue_worktree_cleanup
- [x] Total: 35 MCP tools
- [x] Blue's voice in all error messages
- [x] 28 tests passing

### Phase 7: Pending (Future)

Remaining tools to port (if needed):
- Code search/indexing (requires tree-sitter)
- IaC detection and staging deployment tracking
- PRD tools (5): create, get, approve, complete, list

## Test Plan

- [ ] blue init creates .blue/ directory structure
- [x] blue rfc create persists to SQLite
- [x] blue status shows active/ready/stalled items
- [x] blue search finds documents by keyword
- [x] Blue's voice in all error messages
- [ ] Worktree operations work with git

---

*"Right then. Quite a bit to port. But we'll take it step by step."*

— Blue
