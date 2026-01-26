# Spike: Alignment Dialogue Output Size & Background Agents

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time-box** | 1 hour |

## Question

Why are alignment dialogue agent outputs extremely large (411K+ characters) and why can't the Judge collect outputs via `blue_extract_dialogue`?

## Investigation

### Root Cause 1: Unbounded Agent Output

The agent prompt template said "Be concise but substantive (300-500 words)" — a suggestion, not a hard constraint. With 12 experts on a complex topic, agents produced comprehensive essays instead of focused perspectives. One agent (Muffin) generated 411,846 characters.

### Root Cause 2: Foreground Tasks Instead of Background

The Judge protocol specified `run_in_background: true` but the Judge was spawning foreground parallel tasks. Foreground tasks return results inline in the message — they do NOT write to `/tmp/claude/.../tasks/{task_id}.output`. When the Judge then called `blue_extract_dialogue(task_id=...)`, no output files existed.

### Root Cause 3: Vague Output Collection Instructions

The protocol said "Wait for all agents to complete, then use blue_extract_dialogue to read outputs" but didn't explain the explicit loop: spawn background → get task IDs → call `blue_extract_dialogue` for each task ID.

### Reference: coherence-mcp JSONL Analysis

Analyzed all coherence-mcp session JSONL files for actual Task call patterns. Key finding:

**Coherence-mcp dispatched agents as SEPARATE messages, each with `run_in_background: true`:**
```
Line 127: Task(Muffin, run_in_background=True)  → returns immediately
Line 128: Task(Scone, run_in_background=True)   → returns immediately
Line 129: Task(Eclair, run_in_background=True)  → returns immediately
```

Each agent was a separate assistant message with 1 Task call. But because `run_in_background: true`, each returned instantly and all ran in parallel.

The SKILL.md ideal of "ALL N Task calls in ONE message" was NEVER achieved in practice. The parallelism came from `run_in_background: true`, not from batching calls.

Some sessions mixed approaches (`run_in_background=True` and `NOT SET`), but the working dialogues consistently used `run_in_background: true`.

## Findings

| Issue | Root Cause | Fix |
|-------|-----------|-----|
| 411K agent output | No hard word limit | Added 400-word max, 2000-char target |
| Serial execution | Foreground tasks block until completion | `run_in_background: true` returns immediately |
| Can't find task outputs | Foreground tasks don't write output files | Background tasks write to `/tmp/claude/.../tasks/` |
| Missing markers | No REFINEMENT/CONCESSION | Added from coherence-mcp ADR 0006 |

## Changes Made

### Phase 1: Prompt & Protocol Fixes

**Agent Prompt Template (`dialogue.rs`)**
- Added collaborative tone from coherence-mcp ADR 0006 (SURFACE, DEFEND, CHALLENGE, INTEGRATE, CONCEDE)
- Added REFINEMENT and CONCESSION markers
- Added hard output limit: "MAXIMUM 400 words", "under 2000 characters"
- Added "DO NOT write essays, literature reviews, or exhaustive analyses"

**Judge Protocol (`dialogue.rs`)**
- `run_in_background: true` for parallel execution
- Score ONLY after reading outputs
- Structured as numbered workflow steps

### Phase 2: Portability (CLAUDE.md → MCP instructions)

- Moved Blue voice patterns and ADR list from `CLAUDE.md` to MCP server `instructions` field in initialize response
- Deleted `CLAUDE.md` — all portable content now lives in MCP `instructions`
- Deleted global skill `~/.claude/skills/alignment-play/` — replaced by subagent approach

### Phase 3: Subagent Architecture

**Key insight**: Claude Code custom subagents (`.claude/agents/`) provide tool restrictions, model selection, and system prompts — the right mechanism for expert agents.

**Created `.claude/agents/alignment-expert.md`** (project + user level):
- `tools: Read, Grep, Glob` — no MCP tools (experts don't need them)
- `model: sonnet` — cost-effective for focused perspectives
- System prompt includes collaborative tone, markers, and hard output limits

**Updated Judge Protocol** to use:
- `subagent_type: "alignment-expert"` instead of generic Task agents
- `max_turns: 3` to bound agent compute
- `run_in_background: true` for parallel dispatch

**Portability**: Agent definition installed at both:
- `.claude/agents/alignment-expert.md` (blue repo)
- `~/.claude/agents/alignment-expert.md` (user level, works from any repo)

## Outcome

Binary installed. Restart Claude Code and test from fungal-image-analysis to verify:
1. Subagents use `alignment-expert` type (not generic Task)
2. `max_turns: 3` bounds agent compute
3. `run_in_background: true` enables parallel execution
4. Outputs are under 2000 characters each
5. Emoji markers present, no pre-filled scores
