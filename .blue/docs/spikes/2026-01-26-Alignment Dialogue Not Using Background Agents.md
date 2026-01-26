# Spike: Alignment Dialogue Not Using Background Agents

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

ADR 0014 specifies Task(bg) — parallel background agents for alignment dialogues. In fungal-image-analysis, the latest alignment game spawned 12 agents as foreground parallel Task calls instead. Why did Claude Code stop using `run_in_background: true`? Is the ADR spec ambiguous, did Claude Code behavior change, or is the CLAUDE.md instruction insufficient?

---

## Findings

### Root Cause: Spec-Implementation Gap

The ADR describes "background tasks" conceptually but no operational instruction ever tells Claude Code to use `run_in_background: true`.

### Evidence

**1. ADR 0014 says "background" in three places:**

- Line 250: `"The ALIGNMENT dialogue runs in Claude Code using the Task tool with background agents."`
- Line 260: `"Spawns N Cupcakes as PARALLEL background tasks"`
- Lines 270-271: ASCII art shows `Task(bg)` labels on each agent

**2. CLAUDE.md instruction does NOT say `run_in_background`:**

The Blue CLAUDE.md (the only operational instruction) says:

> "Spawn N 🧁 agents in PARALLEL - single message with N Task tool calls"

This tells Claude Code to put N Task calls in a single message — which produces **foreground parallel** execution, not background. Claude Code only uses `run_in_background: true` when explicitly told to.

**3. fungal-image-analysis has no CLAUDE.md:**

The project has no CLAUDE.md file at all. It relies entirely on Blue's CLAUDE.md being loaded (which happens via the Blue workspace, not the fungal project). When working in fungal-image-analysis, the alignment dialogue instructions may not even be present unless Blue's CLAUDE.md is in scope.

**4. Format drift confirms instruction gap:**

The latest dialogue (`2026-01-26-unified-ecs-scaling.dialogue.md`) drops the 🧁/💙 emoji markers from agent names and uses a different header format ("Expert Panel" with Tier/Relevance columns) compared to older dialogues that follow ADR 0014 format precisely (e.g., `ecs-sqs-autoscaling.dialogue.md` which has proper `🧁 Muffin` naming). This suggests the orchestrating session wasn't following ADR 0014 closely.

### Why It Worked Before (Hypothesis)

Two possibilities:

1. **It never actually used `run_in_background: true`** — foreground parallel Task calls look similar in output. Earlier sessions may have appeared to use background agents because the parallel foreground execution produces similar timing. The user may be noticing a different behavior change (e.g., model, tool usage count).

2. **Earlier Claude Code versions may have defaulted differently** — Claude Code's Task tool behavior for parallel calls may have changed. Foreground parallel still runs all agents simultaneously; the difference is the Judge blocks until all complete rather than continuing to work.

### Foreground vs Background: Does It Matter?

| Aspect | Foreground Parallel | Background Parallel |
|--------|-------------------|-------------------|
| Execution | Simultaneous | Simultaneous |
| Judge blocks? | Yes, until all complete | No, continues immediately |
| Result access | Inline in response | Via output file reads |
| Coordination | Simpler (results arrive together) | Complex (poll/read files) |
| Agent tool use | Full tool access | Full tool access |

For alignment dialogues, the Judge **wants** to wait for all agents before scoring. So foreground parallel is functionally equivalent. The only advantage of background is if the Judge needs to do intermediate work while agents execute (e.g., updating the dialogue file header).

However, the `0 tool uses` shown in the output suggests agents are receiving everything in their prompt and producing text-only output — which means they're not reading the `.dialogue.md` file or the draft RFC themselves. Background agents with tool access would read the source material directly, producing more grounded responses.

## Diagnosis

**Primary**: The CLAUDE.md instruction says "parallel" but not "background." Claude Code does exactly what it's told.

**Secondary**: fungal-image-analysis lacks its own CLAUDE.md, so alignment instructions may not be in scope at all. The orchestrating session is improvising the alignment game format rather than following ADR 0014 precisely.

**Tertiary**: The `0 tool uses` pattern indicates agents receive all context via prompt injection rather than reading files with tools. This reduces grounding — agents can't verify claims against source material.

## Recommendations

### Fix 1: Update CLAUDE.md instruction (minimal)

Change the Blue CLAUDE.md alignment section from:

```
2. **Spawn N 🧁 agents in PARALLEL** - single message with N Task tool calls
```

To:

```
2. **Spawn N 🧁 agents as PARALLEL BACKGROUND tasks** - single message with N Task tool calls, each with `run_in_background: true`
3. **Wait for all agents** - read output files to collect results
```

### Fix 2: Add CLAUDE.md to fungal-image-analysis

Create a CLAUDE.md in fungal-image-analysis that either:
- Includes alignment dialogue instructions directly, or
- References Blue's ADR 0014 as the authoritative source

### Fix 3: Ensure agents use tools (important)

The prompt for each 🧁 agent should instruct it to **read the draft RFC and dialogue file using the Read tool** rather than receiving all content via prompt. This:
- Grounds responses in actual file content
- Enables agents to verify claims
- Makes responses more aligned with source material
- Shows tool uses > 0, confirming agents are engaging with the codebase

### Fix 4: Add format enforcement

The `blue_dialogue_lint` tool should validate that dialogue files follow ADR 0014 format (🧁/💙 markers, proper scoreboard structure). Run it after dialogue creation.

## Answer

The alignment game isn't using background agents because **no instruction tells it to**. ADR 0014 describes background tasks architecturally, but the CLAUDE.md operational instruction only says "parallel" — which Claude Code interprets as foreground parallel. Fix the CLAUDE.md instruction to explicitly specify `run_in_background: true` and ensure agents read files with tools rather than receiving everything via prompt.
