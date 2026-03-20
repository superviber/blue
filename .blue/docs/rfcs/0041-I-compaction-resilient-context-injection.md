# RFC 0041: Compaction-Resilient Context Injection

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-30 |
| **Related** | RFC 0016 (Context Injection Architecture), RFC 0038 (SDLC Workflow Discipline) |
| **Problem** | SDLC discipline drift after conversation compaction |

---

## Summary

Establish a compaction-resilient context injection architecture by: (1) consolidating dual-source hook configuration into `~/.claude/settings.json`, (2) using `PreCompact` hooks to inject survival context before compaction so it enters the summary, and (3) using `SessionStart` with `compact` matcher to re-run targeted injection scripts (not CLAUDE.md files).

## Problem

After conversation compaction, Claude exhibits "SDLC drift"—forgetting workflow discipline, knowledge injection, and project-specific behavior patterns. Investigation reveals three root causes:

### 1. Dual-Source Hook Configuration Conflict

| File | Written By | Contents | Status |
|------|------------|----------|--------|
| `~/.claude/hooks.json` | `install.sh` | Blue's `hooks/session-start` | **Ignored** |
| `~/.claude/settings.json` | Manual | Compact-matcher SessionStart | **Active** |

The `install.sh` script writes hooks to `hooks.json`, but Claude Code reads from `settings.json`. Blue's session-start hook never fires.

### 2. No PostCompact Hook Exists

Claude Code provides:
- `PreCompact` — fires BEFORE compaction
- `SessionStart` with `matcher: "compact"` — fires when session RESUMES after compaction

But there is no `PostCompact` hook. When compaction occurs mid-session, the context injected at SessionStart is lost and only restored if the session is paused and resumed.

### 3. Knowledge Files Not Injected

Blue has five knowledge files that should shape Claude's behavior:
- `knowledge/alignment-measure.md` — ALIGNMENT scoring framework
- `knowledge/blue-adrs.md` — Architecture decision records digest
- `knowledge/expert-pools.md` — Expert persona definitions
- `knowledge/task-sync.md` — Claude Code task integration
- `knowledge/workflow-creation.md` — Workflow file guidance

Due to the hook conflict, these files never reach Claude's context.

### Observable Symptoms

- Claude forgets to use `blue_*` MCP tools mid-conversation
- SDLC discipline (worktree enforcement, RFC-driven development) degrades
- Task sync patterns stop working
- ADR adherence drops

## Goals

1. Single authoritative source for hook configuration
2. Critical context survives conversation compaction
3. Knowledge files reliably reach Claude's context
4. Graceful degradation when hooks fail
5. Audit trail for context injection events

## Non-Goals

- Changing Claude Code's hook architecture
- Eliminating compaction (it's necessary for long conversations)
- Real-time context refresh during conversation (MCP Resources handle this)

## Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    ~/.claude/settings.json                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ SessionStart (always)     → hooks/session-start (full)      ││
│  │ SessionStart (compact)    → hooks/context-restore (targeted)││
│  │ PreCompact                → hooks/pre-compact (survival)    ││
│  │ PreToolUse (blue_*)       → blue session-heartbeat          ││
│  │ SessionEnd                → blue session-end                ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    knowledge/*.md (source of truth)              │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ alignment-measure.md  → ALIGNMENT scoring framework         ││
│  │ blue-adrs.md          → Architecture decision digest        ││
│  │ expert-pools.md       → Expert persona definitions          ││
│  │ task-sync.md          → Claude Code task integration        ││
│  │ workflow-creation.md  → Workflow file guidance              ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### Hook Configuration (Consolidated)

All hooks consolidated into `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/blue/hooks/session-start"
          }
        ]
      },
      {
        "matcher": "compact",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/blue/hooks/context-restore"
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/blue/hooks/pre-compact"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "blue_*",
        "hooks": [
          {
            "type": "command",
            "command": "blue session-heartbeat"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "blue session-end"
          }
        ]
      }
    ]
  }
}
```

**Note**: Paths are written as absolute paths during `install.sh` execution.

### PreCompact Hook Strategy

The `PreCompact` hook fires BEFORE compaction, injecting context that becomes part of the compacted summary:

```bash
#!/bin/bash
# hooks/pre-compact
# Inject survival context before compaction

cat << 'EOF'
<compaction-survival-context>
## Critical Context for Post-Compaction

This project uses Blue MCP tools. After compaction, remember:

1. **MCP Tools Available**: blue_status, blue_next, blue_rfc_*, blue_worktree_*, blue_pr_*
2. **SDLC Discipline**: Use worktrees for code changes, RFCs for planning
3. **Task Sync**: Tasks with blue_rfc metadata sync to .plan.md files
4. **Active State**: Run `blue_status` to see current RFC/worktree

If you notice workflow drift, run `blue_status` to restore context.
</compaction-survival-context>
EOF
```

This context becomes part of the compaction summary, ensuring Claude retains awareness of Blue's existence.

### Context-Restore Hook (Post-Compaction)

A targeted script that re-injects critical context after compaction. Lighter than full session-start:

```bash
#!/bin/bash
# hooks/context-restore
# Targeted context restoration after compaction
# Lighter than session-start - only critical awareness

BLUE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

cat << 'EOF'
<blue-context-restore>
## Blue MCP Tools Available

This project uses Blue for SDLC workflow management. Key tools:
- `blue_status` - Current RFC, worktree, and session state
- `blue_next` - Recommended next action
- `blue_rfc_*` - RFC lifecycle management
- `blue_worktree_*` - Isolated development environments

## Workflow Discipline

1. Code changes require active worktrees (RFC 0038)
2. RFCs drive planning; worktrees isolate implementation
3. Tasks with `blue_rfc` metadata sync to .plan.md files

Run `blue_status` to see current project state.
</blue-context-restore>
EOF

# Optionally inject project-specific workflow if it exists
if [ -f ".blue/workflow.md" ]; then
    echo "<blue-project-workflow>"
    cat ".blue/workflow.md"
    echo "</blue-project-workflow>"
fi
```

This hook is deliberately minimal (~150 tokens) to avoid bloating post-compaction context while ensuring Claude remembers Blue exists.

### Three-Layer Injection Model

| Layer | Hook | Matcher | Script | Tokens | Survives Compaction? |
|-------|------|---------|--------|--------|---------------------|
| **Bootstrap** | SessionStart | (none) | `session-start` | ~800 | No (restored on resume) |
| **Restoration** | SessionStart | compact | `context-restore` | ~150 | Yes (re-injected) |
| **Survival** | PreCompact | (none) | `pre-compact` | ~200 | Yes (enters summary) |

**Why three layers?**

- **Bootstrap**: Full knowledge injection when session starts fresh. Expensive but comprehensive.
- **Survival**: Injected BEFORE compaction so critical awareness enters the compacted summary. Claude "remembers" Blue exists.
- **Restoration**: Targeted re-injection when session resumes after compaction. Lighter than bootstrap.

### Installation Script Updates

Update `install.sh` to configure hooks in `settings.json` (not `hooks.json`):

```bash
# install.sh changes

SETTINGS_FILE="$HOME/.claude/settings.json"
BLUE_ROOT="$(cd "$(dirname "$0")" && pwd)"

# Ensure settings.json exists with hooks structure
if [ ! -f "$SETTINGS_FILE" ]; then
    echo '{"hooks":{}}' > "$SETTINGS_FILE"
fi

# Merge blue hooks into settings.json
if command -v jq &> /dev/null; then
    jq --arg blue_root "$BLUE_ROOT" '
    .hooks.SessionStart = [
        {
            "matcher": "",
            "hooks": [{"type": "command", "command": ($blue_root + "/hooks/session-start")}]
        },
        {
            "matcher": "compact",
            "hooks": [{"type": "command", "command": ($blue_root + "/hooks/context-restore")}]
        }
    ] |
    .hooks.PreCompact = [
        {
            "matcher": "",
            "hooks": [{"type": "command", "command": ($blue_root + "/hooks/pre-compact")}]
        }
    ] |
    .hooks.PreToolUse = (.hooks.PreToolUse // []) + [
        {
            "matcher": "blue_*",
            "hooks": [{"type": "command", "command": "blue session-heartbeat"}]
        }
    ] |
    .hooks.SessionEnd = [
        {
            "matcher": "",
            "hooks": [{"type": "command", "command": "blue session-end"}]
        }
    ]
    ' "$SETTINGS_FILE" > "$SETTINGS_FILE.tmp" && mv "$SETTINGS_FILE.tmp" "$SETTINGS_FILE"
fi
```

### Migration from hooks.json

For users with existing `~/.claude/hooks.json`:

```bash
# Detect and migrate hooks.json → settings.json
if [ -f "$HOME/.claude/hooks.json" ] && [ -f "$HOME/.claude/settings.json" ]; then
    echo "Migrating hooks.json to settings.json..."
    jq -s '.[0] * .[1]' "$HOME/.claude/settings.json" "$HOME/.claude/hooks.json" > "$HOME/.claude/settings.json.tmp"
    mv "$HOME/.claude/settings.json.tmp" "$HOME/.claude/settings.json"
    mv "$HOME/.claude/hooks.json" "$HOME/.claude/hooks.json.migrated"
    echo "Migration complete. Old file preserved as hooks.json.migrated"
fi
```

### Audit Trail

Add injection logging to `blue.db`:

```sql
CREATE TABLE IF NOT EXISTS context_injections (
    id INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    hook_type TEXT NOT NULL,  -- SessionStart, PreCompact, etc.
    matcher TEXT,             -- compact, blue_*, etc.
    content_hash TEXT,        -- SHA-256 of injected content
    token_estimate INTEGER,   -- Approximate token count
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
```

## Implementation Plan

### Phase 1: Hook Scripts & Configuration
1. Create `hooks/pre-compact` script with survival context
2. Create `hooks/context-restore` script for post-compaction restoration
3. Update `install.sh` to write to `settings.json` instead of `hooks.json`
4. Add migration logic: detect `hooks.json` and merge into `settings.json`
5. Test hook firing with `blue context test` command

### Phase 2: Audit & Observability
6. Add `context_injections` table to schema
7. Update all hooks to log injections via `blue context log`
8. Add `blue context audit` command to view injection history
9. Add `blue context test` command to verify hooks fire correctly

### Phase 3: Documentation & Refinement
10. Document hook customization for per-project needs
11. Add `.blue/context-restore.md` override support (optional per-project restoration)
12. Tune token budgets based on production usage

## Test Plan

### Hook Configuration
- [ ] `install.sh` writes to `settings.json`, not `hooks.json`
- [ ] Existing `settings.json` hooks preserved during install
- [ ] `hooks.json` contents migrated to `settings.json`
- [ ] SessionStart hook fires on new session
- [ ] SessionStart (compact) hook fires after compaction
- [ ] PreCompact hook fires before compaction
- [ ] PreToolUse hook fires only for blue_* tools

### Context Injection
- [ ] Knowledge files injected at SessionStart via `session-start` hook
- [ ] `context-restore` hook fires after compaction with targeted content
- [ ] `pre-compact` survival context appears in compaction summary
- [ ] Claude retains Blue awareness after compaction (manual verification)

### Audit Trail
- [ ] Injections logged to context_injections table
- [ ] `blue context audit` shows injection history
- [ ] `blue context test` verifies hook configuration

### Per-Project Override
- [ ] `.blue/context-restore.md` content appended if present
- [ ] Missing override file handled gracefully

## Alternatives Considered

### A. Request PostCompact Hook from Anthropic

**Deferred**: Would be ideal but requires upstream changes. PreCompact + SessionStart(compact) provides equivalent functionality today.

### B. MCP Resource for Context Refresh

**Complementary**: RFC 0016's MCP Resources (`blue://context/*`) provide on-demand refresh. This RFC addresses the gap where Claude forgets MCP tools exist.

### C. Periodic Context Re-injection via UserPromptSubmit

**Rejected**: Would inject on every user message, wasting tokens. PreCompact is more efficient—fires only when needed.

### D. Use CLAUDE.md Files

**Rejected**: Adds file management overhead. Direct hook scripts are simpler—no generated artifacts to maintain, no sync issues between knowledge files and CLAUDE.md.

### E. Re-run Full session-start on Compact

**Rejected**: Too heavy (~800 tokens). The `context-restore` hook is deliberately minimal (~150 tokens) to avoid bloating post-compaction context.

## Consequences

### Positive
- SDLC discipline survives conversation compaction
- Single authoritative hook configuration in `settings.json`
- Audit trail for debugging injection issues
- No generated files to maintain (no CLAUDE.md sync issues)
- Targeted restoration (~150 tokens) vs full re-injection (~800 tokens)

### Negative
- Requires manual migration for existing users
- PreCompact adds ~200 tokens before each compaction
- Three hooks to maintain instead of one

### Neutral
- Shifts from hooks.json to settings.json (Claude Code's actual config)
- Knowledge files remain source of truth; hooks read them directly

## References

- [RFC 0016: Context Injection Architecture](0016-context-injection-architecture.draft.md) — Three-tier model
- [RFC 0038: SDLC Workflow Discipline](0038-sdlc-workflow-discipline.draft.md) — What needs to survive
- [Claude Code Hooks Documentation](https://docs.anthropic.com/en/docs/claude-code/hooks) — Available hook events

---

*"Context flows through explicit boundaries; survival requires deliberate architecture."*

— Blue
