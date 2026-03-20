# RFC 0033: Round Scoped Dialogue Files

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-26 |
| **Source Spike** | [Read tool token limit on assembled dialogue documents](../spikes/2026-01-26T1843Z-read-tool-token-limit-on-assembled-dialogue-documents.wip.md) |
| **Alignment Dialogues** | [Round-scoped file architecture](../dialogues/2026-01-26T1850Z-round-scoped-file-architecture-for-alignment-dialogues.dialogue.recorded.md), [Separated document architecture](../dialogues/2026-01-26T1906Z-rfc-0033-separated-document-architecture.dialogue.recorded.md) |

---

## Summary

Judge agent reads assembled dialogue documents that accumulate to 31K+ tokens, exceeding Read tool's 25K limit. This RFC implements a round-scoped file architecture where each round writes to separate files, keeping all reads under limits while minimizing Opus (Judge) usage.

## Problem

The current alignment dialogue implementation accumulates all expert perspectives and synthesis into a single document. With 3-6 experts, 2-4 rounds, and ~400 words per perspective:

- 4 rounds × 5 experts × 400 words = ~8,000 words
- Plus synthesis, tensions, metadata = ~10KB per round
- Total: 40KB+, or ~31K+ tokens

The Read tool's 25K token limit causes dialogue failure when the Judge attempts to read the assembled document.

## Solution

Replace single-document accumulation with round-scoped files. No `perspectives.md` — agents read peer files directly.

```
/tmp/blue-dialogue/{slug}/
├─ scoreboard.md              ← Judge writes + reads (~500 bytes)
├─ tensions.md                ← Judge writes, both read (~1-2KB)
├─ round-0/
│  ├─ muffin.md               ← Agents write, agents read (~2-3KB each)
│  ├─ cupcake.md
│  └─ scone.md
├─ round-0.summary.md         ← Judge writes, agents read (~1-2KB)
├─ round-1/
│  └─ {agent}.md
└─ round-1.summary.md
```

**Every file has exactly one writer and at least one reader.**

### Cost Optimization

| Actor | Model | Reads | Writes |
|-------|-------|-------|--------|
| Judge | Opus | scoreboard + tensions + prior summary | scoreboard, tensions, summary |
| Agents | Sonnet | tensions + peer files + prior summary | own perspective file |

**Opus reads per round:** ~5KB (scoreboard + tensions + prior summary)
**Sonnet reads per round:** ~15KB (tensions + peer files + prior summary)

### Key Design Decisions

Resolved through alignment dialogue with 100% expert convergence:

| Decision | Resolution |
|----------|------------|
| perspectives.md purpose | **Removed** — write-only artifact with no consumer |
| Agent coordination | **Peer-to-peer** — agents read each other's raw outputs directly |
| Token growth | **Sonnet reads acceptable** — 25K is Read tool limit, not cost constraint |
| Cross-round references | **Global namespace** — T01, T02, T03... never reused across rounds |
| Final assembly | **Post-dialogue tooling** — concatenate files after completion if needed |

### Token Budget

**Judge (Opus) per-round reads:**

| Read Operation | Size |
|----------------|------|
| scoreboard.md | ~500 bytes |
| tensions.md | ~1-2KB |
| Prior round summary | ~1-2KB |
| **Total** | ~3-5KB |

**Agents (Sonnet) per-round reads:**

| Read Operation | Size |
|----------------|------|
| tensions.md | ~1-2KB |
| Peer agent files | ~2-3KB × 2-4 peers = ~6-12KB |
| Prior round summary | ~1-2KB |
| **Total** | ~10-15KB |

Opus usage minimized. Sonnet reads well under 25K limit.

### Agent Return Constraint

**Subagents MUST return summary information to the Judge** to ensure process continuation:

```
After writing your perspective to the file, return a brief summary:
- Key perspective(s) raised
- Tension(s) identified
- Concession(s) made
```

This ensures the Judge receives confirmation that agents completed and has context for synthesis without re-reading files.

### Judge Workflow

**Pre-round (prompt distribution):**
```rust
// Read minimal state (~3-5KB Opus)
let scoreboard = read("scoreboard.md");
let tensions = read("tensions.md");
let prior_summary = read(&format!("round-{}.summary.md", round - 1));

// Spawn agents with context
for agent in experts {
    spawn_agent(agent, topic, tensions, prior_summary);
}
```

**Post-round (synthesis from agent returns):**
```rust
// Agents return summaries — no file reads needed
let summaries = collect_agent_returns();
let synthesis = judge_synthesize(summaries);

// Write separated artifacts
update("scoreboard.md", new_scores);
update("tensions.md", resolved, new_tensions);
write(&format!("round-{}.summary.md", round), synthesis);
```

## Implementation

### Changes to `dialogue.rs`

1. **Update `build_judge_protocol`**: Judge reads only scoreboard + tensions + prior summary
2. **Remove perspectives.md**: No assembled perspectives file
3. **Add agent return requirement**: Agents must return summary to Judge after writing
4. **Update agent prompts**: Agents read peer files directly from `round-N/` directories

### File Structure

**scoreboard.md** (~500 bytes):
- Convergence percentages per agent
- Round count
- Overall status

**tensions.md** (~1-2KB):
- Active tensions with IDs (T01, T02...)
- Resolved tensions (marked, not removed)

**round-N.summary.md** (~1-2KB):
- Judge synthesis only
- Emerging consensus
- Key decisions this round

**round-N/{agent}.md** (~2-3KB each):
- Full agent perspective
- Read by peer agents (Sonnet)
- Judge receives summary via agent return, not file read

## Test Plan

- [ ] Run 3-round alignment dialogue without token limit errors
- [ ] Verify scoreboard.md stays under 1KB
- [ ] Verify tensions.md stays under 3KB
- [ ] Verify round summaries stay under 3KB each
- [ ] Verify Judge (Opus) reads under 5KB per round
- [ ] Verify agents return summary to Judge after writing
- [ ] Verify agents can read peer files from prior rounds
- [ ] Verify final dialogue assembly works post-dialogue

---

*"Right then. Let's get to it."*

— Blue
