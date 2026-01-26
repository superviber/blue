# Alignment Dialogue: Claude Code Task Integration

| | |
|---|---|
| **Topic** | RFC for integrating Blue plan files with Claude Code's task management |
| **Source** | Spike: Claude Code Task Integration (2026-01-26) |
| **Experts** | 12 |
| **Target** | 95% convergence |
| **Status** | In Progress |

---

## Round 1: Initial Perspectives

### Scoreboard

| Expert | Role | Wisdom | Consistency | Truth | Relationships | Total |
|--------|------|--------|-------------|-------|---------------|-------|
| 1 | Systems Architect | 9 | 9 | 9 | 8 | 35 |
| 2 | Developer Experience | 8 | 9 | 8 | 9 | 34 |
| 3 | File System Philosopher | 10 | 10 | 10 | 7 | 37 |
| 4 | MCP Protocol Expert | 9 | 10 | 9 | 8 | 36 |
| 5 | Workflow Automation | 8 | 8 | 8 | 8 | 32 |
| 6 | Data Sync Expert | 8 | 9 | 9 | 7 | 33 |
| 7 | UI Designer | 9 | 8 | 9 | 9 | 35 |
| 8 | Reliability Engineer | 9 | 9 | 9 | 7 | 34 |
| 9 | Simplicity Advocate | 10 | 9 | 10 | 8 | 37 |
| 10 | Security Analyst | 8 | 9 | 9 | 7 | 33 |
| 11 | Integration Architect | 9 | 8 | 8 | 8 | 33 |
| 12 | Blue ADR Guardian | 9 | 10 | 10 | 9 | 38 |

### Convergence Points (100%)

1. **File Authority**: `.plan.md` files are the single source of truth (ADR 5)
2. **Ephemeral Tasks**: Claude Code tasks are session-local mirrors, not persistent state
3. **Skill Orchestration**: Skills mediate between Blue MCP and Claude Code tasks
4. **No MCP Push**: MCP's request-response nature means Blue cannot initiate sync

### Key Perspectives

**Expert 3 (File System Philosopher):**
> "The file must win, always. When divergence is detected, the file's state is ground truth; the database state is an error to be corrected."

**Expert 9 (Simplicity Advocate):**
> "The integration isn't worth the complexity... Strip this down to read-only exposure. If a user wants to update the Blue plan after completing a Claude Code task, they run `blue_rfc_task_complete` explicitly."

**Expert 4 (MCP Protocol Expert):**
> "Pure skill orchestration is sufficient. The MCP server stays pure—it only answers queries about its documents, never tries to manage external task state."

**Expert 7 (UI Designer):**
> "Make task state transitions (not progress updates) the trigger for filesystem writes."

### Tensions

| Tension | Position A | Position B | Experts |
|---------|-----------|-----------|---------|
| Integration Scope | Full bidirectional sync | Read-only context only | 1,2,5,6,7,8,11 vs 9 |
| New Blue Tools | Add `blue_task_context` | Pure skill orchestration | 11 vs 4,9 |
| Sync Timing | Automatic on completion | Explicit user command | 2,7 vs 5,6,9 |

### Round 1 Convergence: ~75%

Strong agreement on principles, divergence on implementation scope.

---

## Round 2: Resolving Tensions

### Votes

| Expert | Tension 1 | Tension 2 | Tension 3 |
|--------|-----------|-----------|-----------|
| 1 | - | - | - |
| 2 | B | B | B |
| 3 | A | B | B |
| 4 | B | B | B |
| 5 | B | B | B |
| 6 | B | B | B |
| 7 | B | B | B |
| 8 | B | B | B |
| 9 | B | B | B |
| 10 | B | B | B |
| 11 | B | B | B |
| 12 | B | B | B |

### Results

| Tension | Position A | Position B | Winner |
|---------|-----------|-----------|--------|
| 1. Integration Scope | 1 (9%) | 10 (91%) | **B: Read-only context injection** |
| 2. New Blue Tools | 0 (0%) | 11 (100%) | **B: Pure skill orchestration** |
| 3. Sync Timing | 0 (0%) | 11 (100%) | **B: Explicit sync command** |

### Round 2 Convergence: 97%

Target of 95% achieved.

---

## Consensus

The 12 experts converged on the following RFC specification:

### Core Principles

1. **File Authority**: `.plan.md` files are the single source of truth for RFC task state
2. **Ephemeral Mirror**: Claude Code tasks are session-local projections, not persistent state
3. **Skill Orchestration**: A `/blue-plan` skill mediates using existing tools only
4. **Explicit Sync**: Users invoke `blue_rfc_task_complete` manually to persist changes

### Architecture

```
┌─────────────────┐     read      ┌─────────────────┐
│   .plan.md      │◄──────────────│  /blue-plan     │
│   (authority)   │               │    skill        │
└─────────────────┘               └────────┬────────┘
        ▲                                  │
        │                                  │ create
        │ explicit                         ▼
        │ blue_rfc_task_complete   ┌─────────────────┐
        │                          │  Claude Code    │
        └──────────────────────────│    Tasks        │
              user invokes         │  (ephemeral)    │
                                   └─────────────────┘
```

### What the Skill Does

1. On `/blue-plan <rfc-title>`:
   - Calls `blue_rfc_get` to fetch RFC with plan tasks
   - Creates Claude Code tasks via `TaskCreate` for each plan task
   - Stores mapping in task metadata: `{ blue_rfc: "title", blue_task_index: N }`

2. During work:
   - User works normally, Claude marks tasks in_progress/completed
   - Claude Code UI shows progress

3. On task completion:
   - User (or skill prompt) calls `blue_rfc_task_complete` explicitly
   - Plan file updated, becomes source of truth for next session

### What We Don't Build

- No automatic writeback from Claude Code to plan files
- No new Blue MCP tools (existing tools sufficient)
- No bidirectional sync machinery
- No watcher processes or polling

### ADR Alignment

| ADR | Alignment |
|-----|-----------|
| ADR 5 (Single Source) | `.plan.md` is sole authority |
| ADR 8 (Honor) | Explicit sync = say what you do |
| ADR 10 (No Dead Code) | No new tools needed |
| ADR 11 (Constraint) | Simple one-way flow |

---

## Status

~~CONVERGED at 97%~~ - User rejected skills and explicit sync.

---

## Round 3: User Constraints

**User Requirements:**
1. No explicit sync - automatic/implicit instead
2. No skills - don't add Claude Code skills
3. Use injection - context appears automatically

### New Consensus

| Expert | Position | Key Insight |
|--------|----------|-------------|
| 1 | MCP Resources | Expose `.plan.md` as resource, inject on RFC access |
| 2 | Seamless UX | Zero onboarding, tasks appear naturally |
| 3 | Visible Sync | Automatic OK if auditable (git commits) |
| 4 | Tool-Triggered | `blue_rfc_get` returns `_plan_uri` for injection |
| 5 | Lazy Injection | Inject on-demand when RFC referenced |
| 6 | Hash Versioning | Content-hash with three-way merge on conflict |
| 7 | Audit Trail | Sync events logged, visible in status |
| 8 | Confirmation | Three-phase handshake for reliability |
| 9 | File Watcher | Session-scoped injection + file watcher |
| 10 | **DISSENT** | Automatic file writes are security risk |
| 11 | Hooks | Option B+C: tool injection + hook writeback |
| 12 | Observable | Automatic sync honors ADR 8 if transparent |

### Convergence: ~92%

Expert 10 dissents on automatic writeback security.

### Proposed Architecture

```
┌─────────────────┐                    ┌─────────────────┐
│   .plan.md      │◄───── MCP ────────│  blue_rfc_get   │
│   (authority)   │     Resource       │                 │
└────────┬────────┘                    └────────┬────────┘
         │                                      │
         │ auto-inject                          │ returns tasks +
         │ as context                           │ creates CC tasks
         ▼                                      ▼
┌─────────────────┐                    ┌─────────────────┐
│  Claude Code    │◄───────────────────│   TaskCreate    │
│    Context      │    auto-populate   │   (automatic)   │
└────────┬────────┘                    └─────────────────┘
         │
         │ on task complete
         │ (hook triggers)
         ▼
┌─────────────────┐
│ blue_rfc_task   │────────► Updates .plan.md
│    _complete    │          (automatic writeback)
└─────────────────┘
```

### Implementation Approach

1. **MCP Resource**: Expose `.plan.md` files via `blue://docs/rfcs/{id}/plan`
2. **Tool Enhancement**: `blue_rfc_get` includes `_plan_uri` and auto-creates Claude Code tasks
3. **Hook Integration**: Claude Code hook watches task state → calls `blue_rfc_task_complete`
4. **Audit Trail**: All syncs logged with timestamps, visible in `blue status`

### Security Mitigation (for Expert 10's concern)

- Writeback only for tasks with valid `blue_rfc` metadata
- Content-hash validation before write (detect external changes)
- Audit log in `.plan.md` comments for forensics
- Rate limiting on automatic writes

---

## Round 4: Security Resolution

**Question**: How to address Expert 10's security concern about automatic file writes?

| Option | Description |
|--------|-------------|
| A | Accept risk with mitigations only |
| B | First-time confirmation per RFC |
| C | Opt-in via config (disabled by default) |

### Votes

| Expert | Vote | Justification |
|--------|------|---------------|
| 1 | B | Confirmation friction only hits once per RFC |
| 2 | B | Builds confidence after first sync |
| 3 | B | Establishes implicit consent to manage companion file |
| 9 | B | Sweet spot: informed consent without ongoing friction |
| 10 | B | Hash validation + first confirmation = informed consent |
| 12 | B | ADR 8 requires transparency; confirmation makes behavior knowable |

### Result: **Option B Unanimous (100%)**

First-time confirmation per RFC satisfies security concern while preserving seamless UX.

---

## Final Consensus

**Convergence: 97%** - Target achieved.

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
         │ on task complete → hook triggers
         ▼
┌─────────────────────────────────────────────────────────┐
│  First time for this RFC?                               │
│  ├─ YES → Confirm: "Enable auto-sync for RFC X?" [Y/n] │
│  └─ NO  → Automatic writeback                          │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────┐
│ blue_rfc_task   │────────► Updates .plan.md
│    _complete    │
└─────────────────┘
```

### Key Decisions

| Decision | Rationale |
|----------|-----------|
| MCP Resource injection | Context appears automatically, no skills |
| Tool-triggered task creation | `blue_rfc_get` auto-populates Claude Code tasks |
| Hook-based writeback | Task completion triggers `blue_rfc_task_complete` |
| First-time confirmation | Balances security with seamlessness |
| Audit trail | All syncs logged, visible in git |

### ADR Alignment

| ADR | How Honored |
|-----|-------------|
| ADR 5 (Single Source) | `.plan.md` remains authoritative |
| ADR 8 (Honor) | First-time confirmation = explicit consent |
| ADR 11 (Constraint) | Automatic flow with minimal friction |

---

## Round 5: User Override

**User Decision**: Remove first-time confirmation. It adds friction for a low-risk operation.

Rationale:
- User is already in their own project
- Writes are just checkbox updates in `.plan.md`
- Git provides full audit trail and rollback
- The "security risk" is overstated for this context

**Final Architecture**: Fully automatic. No prompts, no confirmation.

---

---

## Round 6: Open Questions

### Q1: Visual indicator for auto-created tasks?
**User Decision**: Yes, use 💙

### Q2: Mid-session task additions?

| Expert | Vote | Rationale |
|--------|------|-----------|
| 1 | B | Honors file authority, syncs at natural interaction points |
| 2 | B | Predictable - sync at interaction, not background |
| 3 | B | File is truth, re-read ensures current state |
| 9 | B | Rebuild-on-read already exists, no new complexity |
| 11 | B | Lazy re-read aligns with `is_cache_stale()` pattern |
| 12 | B | ADR 5 requires trusting `.plan.md` as authority |

**Result: B (Poll on access) - Unanimous**

Re-read plan file on next `blue_rfc_get`, create missing tasks.

---

## Status

**CONVERGED** - All open questions resolved. RFC 0019 ready.
