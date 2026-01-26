# RFC 0019: Claude Code Task Integration

| | |
|---|---|
| **Status** | Draft |
| **Created** | 2026-01-25 |
| **Source** | Spike: Claude Code Task Integration |
| **Dialogue** | 12-expert alignment, 97% convergence (4 rounds) |

---

## Problem

Blue's RFC task tracking (via `.plan.md` files per RFC 0017) and Claude Code's built-in task management operate independently. Users cannot see Blue tasks in Claude Code's UI without manual effort.

## Proposal

Integrate Blue plan files with Claude Code through **automatic injection and sync** - no skills, no explicit commands.

### Design Principles

1. **File Authority**: `.plan.md` remains the single source of truth
2. **Automatic Injection**: Plan context appears when RFC is accessed
3. **Automatic Sync**: Task completion writes back without explicit command

### Architecture

```
┌─────────────────┐                    ┌─────────────────┐
│   .plan.md      │◄───── MCP ────────│  blue_rfc_get   │
│   (authority)   │     Resource       │                 │
└────────┬────────┘                    └────────┬────────┘
         │                                      │
         │ auto-inject                          │ auto-creates
         │ as context                           │ Claude Code tasks
         ▼                                      ▼
┌─────────────────┐                    ┌─────────────────┐
│  Claude Code    │◄───────────────────│   TaskCreate    │
│    Context      │                    │   (automatic)   │
└────────┬────────┘                    └─────────────────┘
         │
         │ on task complete (hook)
         ▼
┌─────────────────┐
│ blue_rfc_task   │────────► Updates .plan.md
│    _complete    │          (automatic)
└─────────────────┘
```

## Implementation

### 1. MCP Resource: Plan Files

Expose `.plan.md` files as MCP resources for context injection:

```
URI: blue://docs/rfcs/{number}/plan
Type: text/markdown
```

When Claude Code accesses an RFC, the plan resource is automatically available for context injection.

### 2. Tool Enhancement: Auto Task Creation

Modify `blue_rfc_get` to:
1. Return RFC content as normal
2. Include `_plan_uri` field pointing to plan resource
3. **Automatically call TaskCreate** for each plan task with metadata:

```json
{
  "subject": "Task description from plan",
  "activeForm": "Working on RFC task...",
  "metadata": {
    "blue_rfc": "plan-file-authority",
    "blue_rfc_number": 17,
    "blue_task_index": 0
  }
}
```

### 3. Injected Sync Instruction

Via Blue's `SessionStart` hook, inject knowledge that instructs Claude to sync:

```markdown
# knowledge/task-sync.md (injected at session start)

When you mark a task complete that has `blue_rfc` metadata,
call `blue_rfc_task_complete` with the RFC title and task index
to update the plan file automatically.
```

This works in any repo with Blue installed - no per-repo configuration needed.

### 4. Audit Trail

All syncs are logged:
- In `blue status` output
- As git-friendly comments in `.plan.md`:
  ```markdown
  <!-- sync: 2026-01-25T10:30:42Z task-0 completed -->
  ```

## What This Enables

| Before | After |
|--------|-------|
| User runs `/blue-plan` skill | Tasks appear automatically |
| User calls `blue_rfc_task_complete` | Completion syncs via hook |
| No visibility in Claude Code UI | Full task progress in UI |
| Manual context switching | Seamless flow |

## What We Don't Build

| Rejected | Reason |
|----------|--------|
| Skills | User preference: use injection |
| Explicit sync command | User preference: automatic |
| Bidirectional conflict resolution | First-time consent + hash validation sufficient |

## Security Considerations

| Risk | Mitigation |
|------|------------|
| Injection via metadata | Validate `blue_rfc` metadata exists in Blue |
| Hash conflicts | Content-hash validation before write |
| Audit gaps | All syncs logged with timestamps + git history |

## ADR Alignment

| ADR | How Honored |
|-----|-------------|
| ADR 5 (Single Source) | `.plan.md` is authoritative; Claude Code tasks are mirrors |
| ADR 8 (Honor) | Automatic sync is documented behavior; git provides audit |
| ADR 11 (Constraint) | Fully automatic flow removes all ceremony |

## Open Questions

1. ~~Should auto-created tasks be marked with a visual indicator?~~ **Resolved: Yes, use 💙**
2. ~~How to handle task additions mid-session?~~ **Resolved: Poll on access** - Re-read plan file on next `blue_rfc_get`, create missing tasks. Aligns with rebuild-on-read pattern from RFC 0017.

## References

- [RFC 0017: Plan File Authority](./0017-plan-file-authority.md)
- [RFC 0018: Document Import/Sync](./0018-document-import-sync.md)
- [Spike: Claude Code Task Integration](../spikes/2026-01-26-claude-code-task-integration.md)
- [Alignment Dialogue](../dialogues/2026-01-25-claude-code-task-integration.dialogue.md)
