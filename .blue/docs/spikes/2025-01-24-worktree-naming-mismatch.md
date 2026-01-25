# Spike: Worktree Naming Mismatch Investigation

**Date:** 2025-01-24
**Time-box:** 30 minutes
**Status:** Complete

## Problem

User reported seeing:
```
Done. Implementation complete in worktree ../fungal-image-analysis-rfc0069
(branch rfc0069-job-status-namespace)
```

The worktree/branch weren't created with the right names.

## Investigation

### Blue's Actual Worktree Naming Patterns

**Single-repo worktrees** (`crates/blue-mcp/src/handlers/worktree.rs`):
- Path: `~/.blue/worktrees/<stripped-name>` (e.g., `~/.blue/worktrees/job-status-namespace`)
- Branch: `<stripped-name>` (RFC number prefix stripped per RFC 0007)
- Example: RFC `0069-job-status-namespace` → branch `job-status-namespace`

**Realm multi-repo worktrees** (`crates/blue-mcp/src/handlers/realm.rs`):
- Path: `~/.blue/worktrees/<realm>/<rfc>/<repo>`
- Branch: `rfc/<rfc>` (prefixed with `rfc/`)
- Example: RFC `0069-job-status-namespace` → branch `rfc/0069-job-status-namespace`

### What the User Saw

- Path: `../fungal-image-analysis-rfc0069`
- Branch: `rfc0069-job-status-namespace`

### Key Finding: Not From Blue

The observed output doesn't match either Blue pattern:

| Aspect | Blue Single-Repo | Blue Realm | User Saw |
|--------|------------------|------------|----------|
| Path style | Absolute (`~/.blue/...`) | Absolute (`~/.blue/...`) | Relative (`../`) |
| Path components | `worktrees/<name>` | `worktrees/<realm>/<rfc>/<repo>` | `<repo>-<rfc>` |
| Branch format | `<stripped-name>` | `rfc/<rfc>` | `rfc<number>-<name>` (no slash) |

There's no code in Blue that:
1. Produces relative paths like `../`
2. Uses the squashed `rfc0069` format (no leading zeros, no separator)
3. Combines repo name with RFC number in a single path segment

## Root Cause

The message "Implementation complete in worktree" is **not from Blue** - it's from the LLM agent (Claude) that was implementing the feature.

The LLM agent likely:
1. **Constructed its own worktree path** instead of using `blue_worktree_create` or `blue_realm_worktree_create`
2. **Used `git worktree add` directly** via Bash, inventing its own naming convention
3. **Hallucinated the path/branch names** in its completion message

## Evidence

1. Searched Blue codebase for `../` pattern - not found
2. Searched for `rfc\d+` squashed format - not found (only `rfc3339` date formats)
3. Blue's worktree success messages don't include "Implementation complete"
4. Blue always uses absolute paths via `state.home.worktrees_path` or `DaemonPaths`

## Additional Issues Found

While investigating, identified these potential problems in Blue's worktree code:

1. **Inconsistent naming between single/realm**: Single-repo strips RFC number, realm preserves it with `rfc/` prefix
2. **Store failures silently ignored**: `let _ = state.store.add_worktree()` at worktree.rs:87
3. **Realm worktrees not persisted**: `realm.rs:551-691` never calls `add_worktree()`

## Recommendations

1. **Guide LLM agents**: Add clear instructions in `blue_guide` about always using Blue's worktree tools, never raw git worktree commands
2. **Validate in status**: `blue_status` could check for orphan worktrees (git worktrees not in Blue's store)
3. **Consider RFC for guidance**: Write an RFC for "LLM Agent Worktree Best Practices" if this recurs

## Outcome

The worktree naming wasn't wrong in Blue - the LLM agent bypassed Blue's tools entirely and invented its own naming. The fix is behavioral (prompt engineering/guidance) rather than code.
