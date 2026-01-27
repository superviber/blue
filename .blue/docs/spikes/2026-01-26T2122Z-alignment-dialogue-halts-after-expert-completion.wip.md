# Spike: Alignment Dialogue Halts After Expert Completion

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 30 minutes |

---

## Question

Why does the alignment dialogue halt after expert agents complete, requiring user to type "proceed"?

---

## Root Cause

**`run_in_background: true`** in the Judge Protocol prompt at `crates/blue-mcp/src/handlers/dialogue.rs:1013`.

### The mechanism

When the Judge spawns N expert agents, the prompt instructs it to use `run_in_background: true` on each Task call. This causes the following flow:

1. Judge sends ONE message with N Task tool calls, all with `run_in_background: true`
2. All N Task calls return **immediately** with `output_file` paths (not results)
3. **The Judge's turn ends.** It has produced output and is now waiting for user input.
4. The background agents run and complete asynchronously
5. The user sees completion notifications ("Agent Cupcake expert deliberation completed")
6. The Judge does **not** automatically wake up to process results
7. **User must type "proceed"** to give the Judge another turn to collect and score

### The contradiction

Line 1013 says `run_in_background: true`, but line 1015 says:

> "All {agent_count} results return when complete WITH SUMMARIES"

This is false when `run_in_background: true`. Background tasks return output_file paths immediately, not summaries. The prompt assumes blocking semantics but specifies non-blocking execution.

### Why blocking works

If `run_in_background` is `false` (the default), multiple Task calls in a single message:

1. All start in **parallel** (same parallelism benefit)
2. All **block** until every agent completes
3. All results return **in the same response** with full summaries
4. The Judge **immediately** proceeds to scoring, artifact writing, and convergence checking
5. If not converged, the Judge spawns the next round **in the same turn**
6. The entire multi-round dialogue runs to completion **without user intervention**

## Fix

Change `dialogue.rs:1013` from:

```
- run_in_background: true
```

to:

```
- run_in_background: false
```

Or simply remove the line entirely (false is the default).

No other changes needed. The "spawn ALL in a SINGLE message" instruction already provides parallelism. The `run_in_background` flag is orthogonal to parallel execution and only controls whether the Judge blocks on results.

## Evidence

- `crates/blue-mcp/src/handlers/dialogue.rs:1013` - the offending instruction
- `skills/alignment-play/SKILL.md:80-84` - SKILL.md does NOT mention `run_in_background`; it only says "Send ONE message with N Task tool invocations" (which is correct for parallelism)
- User symptom: "the alignment game keeps halting and i need to explicitly tell claude to proceed"

## Risk Assessment

**Low risk.** The only behavioral change is that the Judge's turn blocks until all agents complete instead of returning immediately. This is the desired behavior — the Judge needs results before it can score. Parallelism is preserved because multiple Task calls in one message always run concurrently regardless of the background flag.

One consideration: if agents take a long time, the Judge's turn will be longer. But this is preferable to halting and requiring manual intervention, which defeats the purpose of automated convergence.
