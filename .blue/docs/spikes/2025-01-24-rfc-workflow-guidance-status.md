# Spike: RFC Workflow Guidance Status

**Date:** 2025-01-24
**Time-box:** 30 minutes
**Status:** Complete

## Question

Does Blue have the RFC workflow baked in like coherence-mcp did?

## Short Answer

**No.** RFC 0011 was supposed to add this, but it's marked "Implemented" with all test items unchecked. The implementation is incomplete.

## Investigation

### What coherence-mcp Had (The Goal)

Baked-in workflow guidance that told Claude exactly what to do next:
- RFC accepted → "Use `blue_worktree_create` to start implementation"
- Worktree created → "Make your changes, then use `blue_pr_create`"
- PR merged → "Use `blue_worktree_cleanup` to finish"

Each response included `next_action` with the exact tool and args.

### What Blue Has Now

**Individual tools exist:**
- `blue_rfc_create` / `blue_rfc_update_status` / `blue_rfc_plan` / `blue_rfc_complete`
- `blue_worktree_create` / `blue_worktree_cleanup`
- `blue_pr_create` / `blue_pr_merge`
- `blue_status` / `blue_next`

**But no orchestration:**
- `blue_next` still uses CLI syntax: `"Run 'blue worktree create'"` (server.rs:2234)
- `blue_worktree_create` description lacks workflow context (server.rs:484)
- No `next_action` in RFC status changes
- No warning when RFC goes in-progress without worktree

### RFC 0011 Test Plan Status

| Test Item | Status |
|-----------|--------|
| `blue_rfc_update_status` to "accepted" includes next_action | ❌ Not done |
| `blue_next` uses MCP tool syntax | ❌ Not done |
| `blue_status` hint mentions tool names | ❌ Not done |
| `blue_worktree_create` description includes workflow context | ❌ Not done |
| `blue_rfc_update_status` warns if no worktree | ❌ Not done |
| Manual test: Claude creates worktree after accepting RFC | ❌ Not done |

**All 6 items unchecked, but RFC marked "Implemented".**

### Evidence from Code

```rust
// server.rs:2234 - Still uses CLI syntax
"'{}' is ready to implement. Run 'blue worktree create {}' to start."

// server.rs:484 - No workflow context
"description": "Create an isolated git worktree for RFC implementation."

// Only next_action in entire codebase (worktree.rs:256)
"next_action": "Execute the commands to sync with develop"
```

### What's Missing

1. **`next_action` struct** - Not added to response types
2. **MCP tool syntax in responses** - Still says "Run 'blue ...'" not "Use blue_..."
3. **Workflow context in descriptions** - Tools don't explain when to use them
4. **Worktree warnings** - No warning when RFC goes in-progress without worktree
5. **Generate hint improvements** - Hints don't name specific tools

## Root Cause

RFC 0011 was created and marked "Implemented" prematurely. There's even a worktree at `.blue/worktrees/mcp-workflow-guidance/` suggesting work started but wasn't completed.

## Recommendation

1. **Reopen RFC 0011** - Change status back to "in-progress"
2. **Implement the 6 test items** - They're well-specified already
3. **This will fix the worktree naming issue** - Claude will use Blue's tools instead of improvising

## Impact

Without this, Claude will continue to:
- Skip worktree creation ~50% of the time
- Invent its own worktree naming (like `../fungal-image-analysis-rfc0069`)
- Work directly on main branch
- Not follow the RFC workflow

## Outcome

RFC 0011 is the right solution but wasn't actually implemented. Completing it will give Blue the baked-in workflow guidance that coherence-mcp had.
