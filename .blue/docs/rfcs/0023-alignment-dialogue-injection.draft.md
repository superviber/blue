# RFC 0023: Alignment Dialogue Injection

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | Alignment Dialogue Not Using Background Agents |

---

## Problem

Alignment dialogues in external projects don't follow ADR 0014. The spike found three failures:

1. **No background agents** — Task calls use foreground parallel instead of `run_in_background: true`
2. **No tool use** — Agents receive all context via prompt injection, never read files (0 tool uses)
3. **Format drift** — Dialogues drop emoji markers, use non-standard headers, miss scoreboard structure

Root cause: the orchestrating session has no alignment instructions unless the project has a CLAUDE.md that includes them. External projects (fungal-image-analysis, future repos) won't have Blue's CLAUDE.md.

## Constraint

**No CLAUDE.md files.** The solution must use MCP tool response injection so that any project with Blue connected gets correct alignment behavior automatically. CLAUDE.md is the wrong layer — it requires per-project maintenance and can drift from the source of truth.

## Design

### Injection Point

When `blue_dialogue_create` or the alignment game is initiated, Blue's MCP response injects the full orchestration protocol into the tool response. The Judge receives instructions as part of the tool output, not from a static file.

### What Gets Injected

The tool response from `blue_dialogue_create` includes a `## Judge Protocol` section containing:

```
## Judge Protocol (ADR 0014)

You are the Judge. Follow this protocol exactly.

### Spawning Agents
- Spawn N agents using the Task tool with `run_in_background: true` on each call
- All N Task calls in a SINGLE message (parallel background execution)
- Wait for all agents to complete by reading their output files

### Agent Prompt Template
Each agent prompt MUST include:
- Agent name with emoji: "You are Muffin, a [role]."
- Instruction to READ the dialogue file and any source documents using the Read tool
- The specific question or topic for this round
- Instruction to output structured JSONL for extraction

### After All Agents Complete
- Read each agent's output file
- Use `blue_extract_dialogue` to parse contributions
- Score using ALIGNMENT = Wisdom + Consistency + Truth + Relationships (UNBOUNDED)
- Update the .dialogue.md with scoreboard, perspectives, tensions
- Run `blue_dialogue_lint` to validate format
- Repeat rounds until convergence

### Format Requirements
- Participants line: `Muffin | Cupcake | ... | Judge`
- Agent markers: each agent contribution prefixed with agent name
- Scoreboard: standard table with UNBOUNDED dimensions
- Perspectives Inventory and Tensions Tracker sections required
```

### Agent Grounding Requirement

Each agent prompt injected by the Judge must instruct the agent to:

1. **Read the dialogue file** — `Read(.blue/docs/dialogues/{name}.dialogue.md)`
2. **Read source documents** — any RFC, spike, or PRD referenced in the dialogue
3. **Produce grounded output** — cite specific sections, quote relevant passages

This ensures `tool uses > 0` and responses anchored in actual file content.

### MCP Tool Changes

**`blue_dialogue_create`** (new or updated):
- Creates the `.dialogue.md` scaffold
- Returns the Judge Protocol in the tool response body
- Includes the list of source documents to pass to agents

**`blue_dialogue_save`** (existing):
- No change — continues to persist dialogue state

**`blue_extract_dialogue`** (existing):
- No change — continues to parse agent JSONL outputs

**`blue_dialogue_lint`** (existing):
- Add validation for: background agent markers, scoreboard format, perspectives/tensions sections, emoji markers

### Flow

```
User: "play alignment on [topic]"
  │
  ▼
Judge calls blue_dialogue_create(topic, agents, sources)
  │
  ▼
MCP returns:
  - Created .dialogue.md file path
  - Judge Protocol (injected instructions)
  - Source document paths for agents
  │
  ▼
Judge spawns N Task calls in ONE message
  - Each with run_in_background: true
  - Each prompt includes: agent name, role, instruction to Read files
  │
  ▼
Judge reads output files when all complete
  │
  ▼
Judge scores, updates .dialogue.md, runs blue_dialogue_lint
  │
  ▼
Repeat rounds until convergence
```

## ADR Alignment

- **ADR 0005 (Single Source)** — Protocol lives in one place (MCP tool response), not scattered across CLAUDE.md files
- **ADR 0014 (Alignment Dialogues)** — This RFC implements ADR 0014's architectural spec faithfully
- **ADR 0004 (Evidence)** — Agent tool use provides evidence of grounding (file reads, not prompt-only)
- **ADR 0007 (Integrity)** — Injection ensures structural wholeness regardless of which project runs the dialogue

## Migration

1. Implement `blue_dialogue_create` with Judge Protocol injection
2. Update `blue_dialogue_lint` with format validation rules
3. Remove alignment dialogue instructions from Blue's CLAUDE.md (single source — it now lives in the MCP layer)
4. Test in fungal-image-analysis without any CLAUDE.md present

## Decisions

1. **Pastry names and expert roles** — The injected protocol includes both pastry names and expert roles. The Judge assigns roles based on the topic but uses the canonical pastry name list.
2. **Model parameter** — `blue_dialogue_create` accepts a `model` parameter to specify which model agents use (e.g., sonnet, opus, haiku).
3. **No agent cap** — No maximum agent count. The Judge decides how many agents a topic needs.
