# RFC 0011: MCP Workflow Guidance

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-25 |
| **Source Spike** | Inconsistent Worktree Creation in Claude MCP |

---

## Problem

Claude doesn't consistently create worktrees and feature branches when implementing RFCs via MCP. Investigation found five root causes:

1. **Tool descriptions lack workflow context** - `blue_worktree_create` says what it does but not *when* to use it
2. **CLI syntax in MCP responses** - `blue_next` says "Run 'blue worktree create'" instead of "Use blue_worktree_create"
3. **Hints don't name tools** - `generate_hint()` says "Want to start?" but doesn't say how
4. **No next_action on status change** - When RFC becomes "accepted", response has no guidance
5. **No warning on premature in-progress** - RFC can go in-progress without worktree, only detected later as "stalled"

The result: Claude skips worktree creation ~50% of the time, working directly on main branch.

## Goals

1. Claude creates worktrees consistently when implementing accepted RFCs
2. MCP responses guide Claude to the next workflow step
3. Tool descriptions explain their place in the workflow
4. Warnings surface when workflow is violated

## Non-Goals

- Enforcing worktree creation (just guiding)
- Changing the workflow itself
- CLI behavior changes (MCP only)

## Proposal

### 1. Add `next_action` field to status-changing responses

When `blue_rfc_update_status` changes status to "accepted":

```json
{
  "status": "success",
  "title": "my-feature",
  "new_status": "accepted",
  "next_action": {
    "tool": "blue_worktree_create",
    "args": { "title": "my-feature" },
    "hint": "Create a worktree to start implementation"
  }
}
```

### 2. Fix `blue_next` to use MCP tool syntax

Change:
```rust
"'{}' is ready to implement. Run 'blue worktree create {}' to start."
```

To:
```rust
"'{}' is ready to implement. Use blue_worktree_create with title='{}' to start."
```

### 3. Update `generate_hint()` to name tools

Change:
```rust
"'{}' is ready to implement. Want to start?"
```

To:
```rust
"'{}' is ready to implement. Use blue_worktree_create to begin."
```

### 4. Enhance tool descriptions with workflow context

Current:
```
"Create an isolated git worktree for RFC implementation."
```

Proposed:
```
"Create an isolated git worktree for RFC implementation. Use after an RFC is accepted, before starting work. Creates a feature branch and switches to isolated directory."
```

### 5. Add warning when RFC goes in-progress without worktree

In `handle_rfc_update_status`, when status becomes "in-progress":

```json
{
  "status": "success",
  "warning": "No worktree exists for this RFC. Consider creating one with blue_worktree_create.",
  ...
}
```

## Alternatives Considered

### A. Require worktree before in-progress
Rejected: Too restrictive. Some quick fixes don't need worktrees.

### B. Auto-create worktree on accept
Rejected: Side effects without explicit user action violate principle of least surprise.

### C. Add workflow documentation to MCP server description
Partial: Good idea but doesn't solve the in-context guidance problem.

## Implementation Plan

1. Add `next_action` struct and field to response types
2. Update `handle_rfc_update_status` to include next_action
3. Update `handle_next` to use MCP tool syntax
4. Update `generate_hint()` to name tools
5. Enhance tool descriptions in `handle_tools_list`
6. Add worktree warning in status transitions

## Test Plan

- [x] `blue_rfc_update_status` to "accepted" includes next_action with blue_worktree_create
- [x] `blue_next` output uses MCP tool syntax, not CLI syntax
- [x] `blue_status` hint mentions tool names
- [x] Tool description for blue_worktree_create includes workflow context
- [x] `blue_rfc_update_status` to "in-progress" warns if no worktree exists
- [ ] Manual test: Claude creates worktree after accepting RFC

---

*"Show, don't tell. But also tell, when showing isn't enough."*

— Blue
