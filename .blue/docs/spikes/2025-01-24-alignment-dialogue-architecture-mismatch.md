# Spike: Alignment Dialogue Architecture Mismatch

**Date:** 2025-01-24
**Time-box:** 45 minutes
**Status:** Complete

## Problem

User invoked alignment dialogue expecting:
- Background agents spawned in parallel
- `.dialogue` format output
- Multi-round convergence tracking

Got instead:
- Inline text response (no agents spawned)
- No file created
- No actual expert deliberation

## Root Cause

**Blue's RFC 0012 implementation is architecturally wrong.**

Blue created `blue_alignment_play` as an MCP tool that:
- Runs synchronously in the MCP server
- Uses Ollama to generate fake "expert" responses
- Outputs JSON directly

**Coherence-MCP worked differently:**
- MCP server provides **helper tools only** (extract, lint, validate)
- **Claude orchestrates the dialogue itself** using the Task tool
- Spawns N parallel background agents in a single message
- Collects outputs from JSONL files
- Updates `.dialogue.md` file between rounds

## The Correct Architecture (from coherence-mcp ADR 0006)

```
┌─────────────────────────────────────────────────────────┐
│  CLAUDE SESSION (💙 Judge)                               │
│                                                         │
│  1. Recognize "play alignment" request                  │
│  2. Spawn N agents IN PARALLEL (single message)         │
│     ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │
│     │🧁 Agent1│ │🧁 Agent2│ │🧁 Agent3│ │🧁 Agent4│    │
│     └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘    │
│          │           │           │           │          │
│          ▼           ▼           ▼           ▼          │
│     ┌─────────────────────────────────────────────┐     │
│     │  /tmp/claude/{session}/tasks/{id}.output    │     │
│     └─────────────────────────────────────────────┘     │
│                          │                              │
│  3. Extract outputs via alignment_extract_dialogue      │
│  4. Score responses, update .dialogue.md                │
│  5. Repeat until convergence                            │
│                                                         │
│  MCP TOOLS (helpers only):                              │
│  - alignment_extract_dialogue: read agent JSONL         │
│  - alignment_dialogue_lint: validate format             │
│  - alignment_dialogue_save: persist to docs             │
└─────────────────────────────────────────────────────────┘
```

## Key Differences

| Aspect | Coherence-MCP (Correct) | Blue (Current) |
|--------|-------------------------|----------------|
| Orchestration | Claude + Task tool | MCP server |
| Agent spawning | Parallel background agents | None (fake inline) |
| LLM calls | Each agent is a Claude instance | Ollama in MCP |
| Output format | `.dialogue.md` file | JSON response |
| Multi-round | Real convergence loop | Single response |
| Judge role | Claude session | N/A |

## What Needs to Change

### 1. Delete `blue_alignment_play`

The tool that tries to run dialogues in MCP is wrong. Remove it.

### 2. Add Helper Tools Only

```rust
// Extract text from spawned agent JSONL
blue_extract_dialogue(task_id: str) -> String

// Validate dialogue format
blue_dialogue_lint(file_path: str) -> LintResult

// Save dialogue to .blue/docs/dialogues/
blue_dialogue_save(title: str, content: str) -> Result
```

### 3. Add ADR 0006 to Blue

Copy from coherence-mcp:
- `.blue/docs/adrs/0006-alignment-dialogue-agents.md`

This teaches Claude:
- N+1 agent architecture
- Spawning pattern (parallel Tasks)
- Convergence criteria
- File format

### 4. Reference in CLAUDE.md

Add to Blue's CLAUDE.md:
```markdown
## Alignment Dialogues

When asked to "play alignment" or run expert deliberation:
- See ADR 0006 for the N+1 agent architecture
- Use Task tool to spawn parallel agents
- Collect outputs via blue_extract_dialogue
- Update .dialogue.md file between rounds
```

### 5. Create Skill (Optional)

A `/alignment` skill could encapsulate the pattern, but the core behavior comes from Claude understanding ADR 0006.

## Files to Copy from Coherence-MCP

1. `docs/adrs/0006-alignment-dialogue-agents.md` → `.blue/docs/adrs/`
2. `docs/patterns/alignment-dialogue-pattern.md` → `.blue/docs/patterns/`
3. Handler logic from `crates/alignment-mcp/src/handlers/dialogue.rs`
4. Lint logic from `crates/alignment-mcp/src/handlers/dialogue_lint.rs`

## Immediate Action

The current `blue_alignment_play` tool should be removed. It gives the illusion of working but doesn't actually spawn agents or create proper dialogues.

The ADR that was already copied to Blue (`.blue/docs/adrs/0006-alignment-dialogue-agents.md`) needs to be referenced so Claude knows the pattern.

## Outcome

Blue's alignment dialogue feature was implemented wrong. The MCP server should provide extraction/validation tools, not orchestration. Claude itself orchestrates using the Task tool to spawn parallel agents. Fixing this requires:

1. Removing `blue_alignment_play`
2. Adding `blue_extract_dialogue` helper
3. Ensuring ADR 0006 is in Claude's context
4. Optionally creating a `/alignment` skill
