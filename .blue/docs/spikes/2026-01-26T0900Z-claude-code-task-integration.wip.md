# Spike: Claude Code Task Integration

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 2 hours |

---

## Question

How can Blue integrate with Claude Code's built-in task management (TaskCreate/TaskUpdate/TaskList) to provide bidirectional sync between Blue's RFC tasks and Claude Code's UI?

---

## Investigation

### What Claude Code Provides

Claude Code has built-in task management tools:
- `TaskCreate` - Create tasks with subject, description, activeForm
- `TaskUpdate` - Update status (pending → in_progress → completed)
- `TaskList` - List all tasks with status
- `TaskGet` - Get full task details

These render in Claude Code's UI with a progress tracker showing task status.

### Integration Approaches

#### Approach 1: MCP Server Push (Not Viable)
MCP servers can only respond to requests - they cannot initiate calls to Claude Code's task system.

#### Approach 2: Claude Code Skill (Recommended)
Create a `/blue-plan` skill that orchestrates both systems:

```
User: /blue-plan rfc-17
Skill:
  1. Call blue_rfc_get to fetch RFC tasks
  2. For each task, call TaskCreate with:
     - subject: task description
     - metadata: { blue_rfc: 17, blue_task_index: 0 }
  3. As work progresses, TaskUpdate syncs status
  4. On completion, call blue_rfc_task_complete
```

**Pros**: Clean separation, skill handles orchestration
**Cons**: Manual invocation required

#### Approach 3: .plan.md as Shared Interface
The `.plan.md` file (RFC 0017) becomes the bridge:
- Blue writes/reads plan files
- A watcher process syncs changes to Claude Code tasks
- Task checkbox state is the single source of truth

**Pros**: File-based, works offline
**Cons**: Requires external sync process

#### Approach 4: Hybrid - Skill + Plan File
1. `/blue-plan` skill reads `.plan.md` and creates Claude Code tasks
2. User works, marking tasks complete in Claude Code
3. On session end, skill writes back to `.plan.md`
4. Blue picks up changes on next read (rebuild-on-read from RFC 0017)

### Recommended Path

**Phase 1**: Implement RFC 0017 (plan file authority) - gives us the file format
**Phase 2**: Create `/blue-plan` skill that syncs plan → Claude Code tasks
**Phase 3**: Add completion writeback to skill

### Key Insight

The `.plan.md` format is already compatible with Claude Code's task model:
- `- [ ] Task` maps to TaskCreate with status=pending
- `- [x] Task` maps to status=completed

The skill just needs to translate between formats.

## Conclusion

Integration is viable via a Claude Code skill that reads `.plan.md` files and creates corresponding Claude Code tasks. This preserves Blue's file-first philosophy while enabling Claude Code's task UI.

**Next**: Create RFC for skill implementation after RFC 0017 is complete.
