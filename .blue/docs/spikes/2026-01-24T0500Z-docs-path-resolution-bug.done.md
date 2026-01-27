# Spike: Docs Path Resolution Bug

| | |
|---|---|
| **Status** | Completed |
| **Date** | 2026-01-24 |
| **Outcome** | Answered |

---

## Question

Why does blue_rfc_create write to .blue/repos/blue/docs/rfcs/ instead of .blue/docs/rfcs/?

## Root Cause

The bug was caused by coexistence of OLD and NEW directory structures:
- OLD: `.blue/repos/blue/docs/`, `.blue/data/blue/blue.db`
- NEW: `.blue/docs/`, `.blue/blue.db`

When `detect_blue()` runs:
1. It sees `.blue/repos/` or `.blue/data/` exist
2. Calls `migrate_to_new_structure()`
3. Migration is a NO-OP because new paths already exist
4. Returns `BlueHome::new(root)` which sets correct paths

**However**, the MCP server caches `ProjectState` in `self.state`. If the server was started when old structure was the only structure, the cached state has old paths. The state only resets when `cwd` changes.

## Evidence

1. RFC created at `.blue/repos/blue/docs/rfcs/` (wrong)
2. Spike created at `.blue/docs/spikes/` (correct)
3. `detect_blue()` now returns correct paths
4. Old DB (`.blue/data/blue/blue.db`) was modified at 16:28
5. New DB (`.blue/blue.db`) was modified at 16:01

The RFC was stored in the old database because the MCP server had cached the old state.

## Fix Applied

Removed old structure directories:
```bash
rm -rf .blue/repos .blue/data
```

This prevents the migration code path from triggering and ensures only new paths are used.

## Recommendations

1. Migration should DELETE old directories after migration completes, not leave them as orphans
2. Or: `detect_blue()` should always use new paths and ignore old structure once new structure exists
3. Consider adding a version marker file (`.blue/version`) to distinguish structure versions

---

*"The old paths were ghosts. We exorcised them."*

— Blue
