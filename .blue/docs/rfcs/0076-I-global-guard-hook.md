# RFC 0076: Global Guard Hook

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-03-24 |
| **Depends On** | RFC 0049 (Synchronous Guard), RFC 0052 (CLI Hook Management) |

---

## Summary

The guard-write hook is currently installed per-project by `blue install`, duplicating the same shell script into every repo's `.claude/hooks/guard-write.sh` and registering it in every repo's `.claude/settings.json`. This creates maintenance burden: when the hook logic changes (e.g., adding RFC status edit interception), every repo must be updated individually.

Claude Code supports global hooks in `~/.claude/settings.json`. Blue already uses this for `SessionStart`, `PreCompact`, `SessionEnd`, and `PreToolUse` (heartbeat). The guard-write hook should join them — a single global `PreToolUse` entry that runs `blue guard` for all `Write|Edit|MultiEdit` tool calls, eliminating per-project hook files entirely.

## Problem

1. **Duplication** — the-move-social has 10 copies of `guard-write.sh` (one per sub-repo). When we added RFC status edit interception, all 10 had to be updated manually.
2. **Drift** — any repo that skips `blue install --force` runs stale hook logic.
3. **Onboarding friction** — new repos require `blue install` before guards are active. Forgetting this means no protection.
4. **Inconsistency** — some blue hooks are global (session-start, heartbeat) while guard-write is per-project. No principled reason for the split.

## Proposal

### Move guard-write to `~/.claude/settings.json`

Add a global `PreToolUse` hook entry:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "command": "blue guard --stdin"
          }
        ]
      }
    ]
  }
}
```

### New `--stdin` flag for `blue guard`

Today `blue guard --path=<file>` receives only the file path. The shell hook extracts it from stdin JSON. With `--stdin`, blue reads the full PreToolUse JSON directly:

```json
{
  "tool_name": "Edit",
  "tool_input": {
    "file_path": "...",
    "old_string": "...",
    "new_string": "..."
  }
}
```

This lets blue guard perform content-aware checks (like RFC status edit interception) in Rust instead of fragile bash.

### What `blue guard --stdin` does

1. **Parse stdin** — extract `tool_name`, `file_path`, `old_string`, `new_string`, `content` from JSON.
2. **RFC status interception** (RFC 0031) — if the file is in `.blue/docs/rfcs/` and the edit changes the `| **Status** |` field, block with hint to use `blue rfc status`.
3. **Existing guard logic** — all current checks from `run_guard_sync()`: worktree scope, RFC approval, source code protection, allowlist.
4. **Exit codes** — 0 = allow, non-zero = block (with stderr message).

### Remove per-project hook files

`blue install` stops writing `.claude/hooks/guard-write.sh` and the corresponding `settings.json` PreToolUse entry. Instead, `blue install` (the global portion in blue-core) registers the global PreToolUse hook.

### Migration

- `blue install` in any repo detects and removes the old per-project `guard-write.sh` (already has this logic for `blue uninstall`).
- `blue install` adds the global hook to `~/.claude/settings.json` if not already present.
- `blue doctor` checks for stale per-project hooks and warns.

## Per-project `session-start.sh`

The `session-start.sh` hook (PATH injection) is already handled globally by `blue hook session-start` in `~/.claude/settings.json`. The per-project copy is redundant and should also be removed.

After this RFC, `blue install` should not create `.claude/hooks/` at all. The `.claude/settings.json` per-project file becomes unnecessary for hooks (it may still be needed for other project-level settings).

## Backward Compatibility

- Per-project hooks continue to work alongside global hooks — Claude Code runs both. There is no conflict, just redundancy.
- `blue guard --path=<file>` continues to work for any remaining per-project hooks.
- `blue uninstall` cleans up both global and per-project hooks.

## Phases

### Phase 1: `blue guard --stdin`
- [ ] Add `--stdin` flag to synchronous guard path in `main.rs`
- [ ] Parse full tool input JSON (file_path, old_string, new_string, content)
- [ ] Implement RFC status edit interception in Rust
- [ ] Keep `--path` working for backward compat

### Phase 2: Global hook registration
- [ ] Update `blue_core::install::configure_hooks()` to add PreToolUse guard entry
- [ ] Dedup logic: strip existing blue guard hooks before adding
- [ ] `blue install` in project removes per-project guard-write.sh and session-start.sh
- [ ] `blue install` removes per-project `.claude/settings.json` hook entries (preserves non-hook settings)

### Phase 3: Cleanup
- [ ] `blue doctor` warns about stale per-project hooks
- [ ] Remove `GUARD_WRITE_HOOK` and `SESSION_START_HOOK` constants from main.rs
- [ ] Remove `install_hooks()` function from CLI
- [ ] Update RFC 0052 as superseded or amended

## Resolved Questions

- [x] **Synchronous execution**: `blue guard --stdin` runs synchronously (pre-tokio), same as `--path`. Stdin read + JSON parse + filesystem checks are all fast enough. No async needed.
- [x] **PATH resolution**: The global hook uses `blue guard --stdin` (PATH-resolved). The `SessionStart` hook always runs first and injects blue onto PATH, so this is reliable.
