# RFC 0015: Alignment Dialogue Architecture Fix

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-25 |
| **Supersedes** | RFC 0012 (partially - rejects Option B, implements Option A) |
| **Source Spike** | 2025-01-24-alignment-dialogue-architecture-mismatch |

---

## Problem

RFC 0012 identified three options for alignment dialogue orchestration:
- **Option A**: Claude orchestrates via Task tool (recommended in coherence-mcp)
- **Option B**: Blue MCP tool orchestrates via Ollama
- **Option C**: Hybrid

Implementation chose **Option B**, which is architecturally wrong:

| Expected Behavior | Actual Behavior |
|-------------------|-----------------|
| Spawn N parallel Claude agents | No agents spawned |
| Real expert deliberation | Fake Ollama responses |
| `.dialogue.md` file created | Inline JSON returned |
| Multi-round convergence | Single-shot response |
| Background processing | Synchronous blocking |

## Root Cause

coherence-mcp's alignment dialogue worked because:
1. **Claude orchestrated** - recognized "play alignment", spawned agents
2. **MCP provided helpers** - extract dialogue from JSONL, lint, save
3. **ADR 0014** was in context - Claude knew the N+1 agent pattern

Blue's implementation:
1. **MCP orchestrates** - `blue_alignment_play` runs everything
2. **Ollama fakes experts** - not real parallel agents
3. **ADR 0014 exists but isn't followed** - wrong architecture

## Decision

**Reject RFC 0012 Option B. Implement Option A.**

### Remove

- `blue_alignment_play` MCP tool (wrong approach)
- `crates/blue-mcp/src/handlers/alignment.rs` (orchestration code)
- Tool registration in `server.rs`

### Keep/Add

Helper tools only:

```rust
// Extract text from spawned agent JSONL
blue_extract_dialogue {
    task_id: Option<String>,    // e.g., "a6dc70c"
    file_path: Option<String>,  // direct path to JSONL
} -> String

// Validate dialogue format
blue_dialogue_lint {
    file_path: String,
} -> LintResult { score: f64, issues: Vec<Issue> }

// Save dialogue to .blue/docs/dialogues/ (exists)
blue_dialogue_save { ... }
```

### Add to CLAUDE.md

```markdown
## Alignment Dialogues

When asked to "play alignment" or run expert deliberation, follow ADR 0014:

1. Act as the 💙 Judge
2. Spawn N 🧁 agents in PARALLEL (single message with N Task tool calls)
3. Each agent gets fresh context, no memory of others
4. Collect outputs via `blue_extract_dialogue`
5. Update `.dialogue.md` with scoreboard, perspectives, tensions
6. Repeat rounds until convergence (velocity → 0 or threshold met)
7. Save via `blue_dialogue_save`

See `.blue/docs/adrs/0006-alignment-dialogue-agents.md` for full spec.
```

### Optional: Create Skill

```yaml
name: alignment-play
trigger: "play alignment"
description: "Run multi-expert alignment dialogue"
```

The skill would encode the orchestration steps, but the core behavior comes from Claude understanding ADR 0014.

## Architecture (Correct)

```
┌─────────────────────────────────────────────────────────┐
│  CLAUDE SESSION (💙 Judge)                               │
│                                                         │
│  User: "play alignment with 5 experts to 95%"           │
│                                                         │
│  1. Recognize trigger, parse params                     │
│  2. Create .dialogue.md with empty scoreboard           │
│  3. For each round:                                     │
│     a. Spawn N Task agents IN PARALLEL                  │
│        ┌─────────┐ ┌─────────┐ ┌─────────┐             │
│        │🧁 Agent1│ │🧁 Agent2│ │🧁 Agent3│ ...         │
│        └────┬────┘ └────┬────┘ └────┬────┘             │
│             └──────────┬──────────┘                     │
│                        ▼                                │
│     b. Read outputs: blue_extract_dialogue(task_id)     │
│     c. Score contributions, update scoreboard           │
│     d. Check convergence                                │
│  4. Save: blue_dialogue_save(...)                       │
│  5. Validate: blue_dialogue_lint(...)                   │
│                                                         │
│  MCP TOOLS (helpers only, no orchestration):            │
│  ├─ blue_extract_dialogue                               │
│  ├─ blue_dialogue_lint                                  │
│  └─ blue_dialogue_save                                  │
└─────────────────────────────────────────────────────────┘
```

## Implementation Plan

1. [x] Remove `blue_alignment_play` tool and handler
2. [x] Remove `crates/blue-mcp/src/handlers/alignment.rs`
3. [x] Add `blue_extract_dialogue` tool (already existed)
4. [x] Verify `blue_dialogue_lint` exists and works
5. [x] Add alignment section to CLAUDE.md
6. [x] Create `/alignment-play` skill in `skills/alignment-play/SKILL.md`
7. [x] Update `install.sh` to copy skills to `~/.claude/skills/`
8. [ ] Test: "/alignment-play" triggers correct behavior

## Test Plan

- [x] `blue_alignment_play` tool no longer exists
- [x] `blue_extract_dialogue` extracts text from Task JSONL
- [x] `blue_dialogue_lint` validates .dialogue.md format
- [x] CLAUDE.md references ADR 0014
- [x] `/alignment-play` skill installed to `~/.claude/skills/`
- [ ] Manual test: "/alignment-play" spawns parallel Task agents

## Migration

Users who relied on `blue_alignment_play`:
- The tool never worked correctly (produced fake inline responses)
- No migration needed - just use the correct pattern now

---

*"The blind men finally compared notes."*

— Blue
