# RFC 0014: Workflow Enforcement Parity

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-25 |
| **Reference** | coherence-mcp workflow patterns |

---

## Summary

Blue's RFC workflow lacks enforcement patterns that coherence-mcp implements effectively. When an RFC is accepted, the current implementation returns a `next_action` JSON field that tools may ignore. Coherence-mcp instead uses conversational hints and gating requirements that create a more reliable workflow.

## Problem

When RFC 0015 was accepted, no worktree was created. The response included:

```json
"next_action": {
    "tool": "blue_worktree_create",
    "args": { "title": "..." },
    "hint": "Create a worktree to start implementation"
}
```

But this metadata was not acted upon. The workflow broke silently.

### Root Causes

1. **No plan requirement** - Blue allows worktree creation for RFCs without plans
2. **Weak hints** - `next_action` is machine-readable but not conversational
3. **No state machine** - Status transitions aren't validated formally
4. **No gating** - Can skip steps (accept → implement without worktree)

## Proposal

Adopt coherence-mcp's workflow patterns:

### 1. Conversational Hints

Change `handle_rfc_update_status` to include conversational prompts:

```rust
let hint = match status {
    "accepted" => Some(
        "RFC accepted. Ask the user: 'Ready to begin implementation? \
         I'll create a worktree and set up the environment.'"
    ),
    "in-progress" => Some(
        "Implementation started. Work in the worktree, mark plan tasks \
         as you complete them."
    ),
    "implemented" => Some(
        "Implementation complete. Ask the user: 'Ready to create a PR?'"
    ),
    _ => None,
};
```

### 2. Plan Enforcement

Gate worktree creation on having a plan:

```rust
// In handle_worktree_create
let tasks = state.store.get_tasks(doc_id)?;
if tasks.is_empty() {
    return Ok(json!({
        "status": "error",
        "message": "RFC needs a plan before creating worktree",
        "next_action": {
            "tool": "blue_rfc_plan",
            "hint": "Create a plan first with implementation tasks"
        }
    }));
}
```

### 3. Status State Machine

Add formal validation in `blue_core::workflow`:

```rust
pub enum RfcStatus {
    Draft,
    Accepted,
    InProgress,
    Implemented,
    Superseded,
}

impl RfcStatus {
    pub fn can_transition_to(&self, to: RfcStatus) -> Result<(), WorkflowError> {
        let valid = matches!(
            (self, to),
            (Self::Draft, Self::Accepted)
                | (Self::Draft, Self::Superseded)
                | (Self::Accepted, Self::InProgress)
                | (Self::Accepted, Self::Superseded)
                | (Self::InProgress, Self::Implemented)
                | (Self::InProgress, Self::Superseded)
                | (Self::Implemented, Self::Superseded)
        );
        if valid { Ok(()) } else { Err(WorkflowError::InvalidTransition) }
    }
}
```

### 4. Workflow Gating

| Action | Requires |
|--------|----------|
| `worktree_create` | RFC accepted + has plan |
| `rfc_complete` | RFC in-progress + 70% tasks done |
| `pr_create` | RFC implemented + worktree exists |

## Implementation Tasks

1. Add `RfcStatus` enum and state machine to `blue-core`
2. Update `handle_rfc_update_status` with conversational hints
3. Add plan check to `handle_worktree_create`
4. Add gating validation to PR creation
5. Update tests for new validation

## Files Changed

- `crates/blue-core/src/workflow.rs` (new)
- `crates/blue-mcp/src/server.rs` (update status handler)
- `crates/blue-mcp/src/handlers/worktree.rs` (add plan check)
- `crates/blue-mcp/src/handlers/pr.rs` (add worktree check)

## Test Plan

- [ ] State machine rejects invalid transitions (draft → implemented)
- [ ] Worktree creation fails without plan
- [ ] Acceptance returns conversational hint
- [ ] PR creation fails without worktree

---

*"Right then. Let's get to it."*

— Blue
