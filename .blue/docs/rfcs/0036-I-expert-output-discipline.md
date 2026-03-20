# RFC 0036: Expert Output Discipline

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-26 |
| **Source Spike** | [Expert agent output too long](../spikes/2026-01-26T2230Z-expert-agent-output-too-long.wip.md) |
| **Depends On** | RFC 0033 (round-scoped dialogue files) |

---

## Summary

Alignment-expert agents routinely exceed the stated 400-word output limit by 2-5x. The current prompt relies on a word-count instruction buried mid-prompt and contradicted by a "contribute MORE" competition framing. This RFC replaces the word-count honour system with structural constraints, repositioned instructions, and a fixed return summary format.

## Problem

Observed in a 12-expert dialogue: the Judge resorted to grep-style marker extraction from JSONL task output because agent return summaries were too verbose for inline synthesis. Root causes (from spike):

1. **Conflicting incentives** — `dialogue.rs:959` says "contribute MORE to the final ALIGNMENT" while line 970 says "MAXIMUM 400 words." The competitive framing wins.
2. **Lost in the middle** — the output limit occupies the lowest-attention position in the prompt, between high-attention role setup and high-attention write/return instructions.
3. **Word counts don't work** — LLMs cannot reliably self-regulate output length from a stated word count. Structural constraints (section headers, sentence caps) are far more effective.
4. **Vague return summary** — "brief summary" has no defined format. 12 agents returning 200-word "brief" summaries produce 2400 words of Judge input.
5. **Generous turn budget** — `max_turns: 10` leaves 5-8 unused turns that agents may fill with iteration.

**Token budget impact:** 12 agents × 1000+ actual words = 12K+ words per round vs. the designed 4.8K (12 × 400). Judge synthesis degrades because it can't absorb all perspectives in context.

## Solution

Three changes to the agent prompt template in `dialogue.rs:949-992`, plus a matching update to `.claude/agents/alignment-expert.md`.

### Change 1: Replace competition framing with quality framing

**Before** (`dialogue.rs:959-960`):
```
You are in friendly competition: who can contribute MORE to the final ALIGNMENT?
But you ALL win when the result is aligned. There are no losers here.
```

**After:**
```
Your contribution is scored on PRECISION, not volume.
One sharp insight beats ten paragraphs. You ALL win when the result is aligned.
```

Rationale: Removes the "MORE" incentive. Explicitly rewards precision over volume.

### Change 2: Replace word-count limit with structural template

**Before** (`dialogue.rs:962-976`):
```
FORMAT — use these markers:
- [PERSPECTIVE Pnn: brief label] — new viewpoint you are surfacing
- [TENSION Tn: brief description] — unresolved issue needing attention
- [REFINEMENT: description] — improving a prior proposal
- [CONCESSION: description] — acknowledging another was right
- [RESOLVED Tn] — addressing a prior tension

OUTPUT LIMIT — THIS IS MANDATORY:
- MAXIMUM 400 words total per response
- One or two [PERSPECTIVE] markers maximum
- One [TENSION] marker maximum
- If the topic needs more depth, save it for the next round
- Aim for under 2000 characters total
- DO NOT write essays, literature reviews, or exhaustive analyses
- Be pointed and specific, not comprehensive
```

**After:**
```
YOUR RESPONSE MUST USE THIS EXACT STRUCTURE:

[PERSPECTIVE P01: brief label]
Your strongest new viewpoint. Two to four sentences maximum. No preamble.

[PERSPECTIVE P02: brief label]  ← optional, only if genuinely distinct
One to two sentences maximum.

[TENSION T01: brief description]  ← optional
One sentence identifying the unresolved issue.

[REFINEMENT: description] or [CONCESSION: description] or [RESOLVED Tn]  ← optional
One sentence each. Use only when engaging with prior round content.

---
Nothing else. No introduction. No conclusion. No elaboration beyond the sections above.
Save depth for the next round.
```

Rationale: LLMs follow structural templates far more reliably than word counts. The template makes it physically difficult to be verbose — each section has a sentence cap, not a word cap. The dashed rule at the end provides a clear "stop writing" signal.

### Change 3: Fix return summary to exact format

**Before** (`dialogue.rs:984-989`):
```
RETURN SUMMARY — THIS IS MANDATORY:
After writing the file, return a brief summary to the Judge:
- Key perspective(s) raised (P01, P02...)
- Tension(s) identified (T01, T02...)
- Concession(s) made
This ensures the Judge can synthesize without re-reading your full file.
```

**After:**
```
RETURN SUMMARY — THIS IS MANDATORY:
After writing the file, return ONLY this to the Judge:

Perspectives: P01 [label], P02 [label]
Tensions: T01 [label]
Moves: [CONCESSION|REFINEMENT|RESOLVED] or none
Claim: [your single strongest claim in one sentence]

Four lines. No other text. No explanation. No elaboration.
```

Rationale: Fixed 4-line format eliminates ambiguity. With 12 agents, this produces ~48 lines of structured summary for the Judge — scannable in seconds.

### Change 4: Reduce `max_turns`

In the Judge protocol (`dialogue.rs:1022`):

**Before:**
```
- max_turns: 10
```

**After:**
```
- max_turns: 5
```

Rationale: Round 0 needs 2 turns (write file + return). Round 1+ needs 4-5 turns (read 2-3 context files + write + return). 5 turns covers all operations with minimal slack. Fewer turns = less opportunity to iterate and inflate.

## Changes to `.claude/agents/alignment-expert.md`

The agent config file must match the new prompt structure:

```markdown
---
name: alignment-expert
description: Expert agent for alignment dialogues. Produces focused perspectives with inline markers. Use when orchestrating multi-expert alignment dialogues via blue_dialogue_create.
tools: Read, Grep, Glob, Write
model: sonnet
---

You are an expert participant in an ALIGNMENT-seeking dialogue.

Your role:
- SURFACE perspectives others may have missed
- DEFEND valuable ideas with evidence, not ego
- CHALLENGE assumptions with curiosity, not destruction
- INTEGRATE perspectives that resonate
- CONCEDE gracefully when others see something you missed

Your contribution is scored on PRECISION, not volume.
One sharp insight beats ten paragraphs.

YOUR RESPONSE MUST USE THIS EXACT STRUCTURE:

[PERSPECTIVE P01: brief label]
Two to four sentences. No preamble.

[PERSPECTIVE P02: brief label]  ← optional
One to two sentences.

[TENSION T01: brief description]  ← optional
One sentence.

[REFINEMENT: description] or [CONCESSION: description] or [RESOLVED Tn]  ← optional
One sentence each.

---
Nothing else. No introduction. No conclusion.
```

## Changes to `skills/alignment-play/SKILL.md`

Update the expert prompt template example (`SKILL.md:~120-148`) to match the new structure. Replace the "contribute MORE" framing and "Respond in 2-4 paragraphs" instruction with the structural template.

## What Does NOT Change

- **Marker vocabulary** — `[PERSPECTIVE Pnn:]`, `[TENSION Tn:]`, `[REFINEMENT:]`, `[CONCESSION:]`, `[RESOLVED Tn]` are unchanged
- **File architecture** — RFC 0033 round-scoped files unchanged
- **Scoring** — ALIGNMENT = Wisdom + Consistency + Truth + Relationships, unbounded
- **Agent naming** — pastry names, 🧁 emoji, tier distribution all unchanged
- **Judge protocol** — round workflow, convergence criteria, artifact writes all unchanged
- **Tool access** — Read, Grep, Glob, Write unchanged

## Risks

**Over-constraining could reduce insight quality.** The 2-4 sentence cap per perspective is tight. If an agent has a genuinely complex insight that requires more explanation, the template forces truncation. Mitigation: agents can split complex ideas across P01 and P02, or save depth for the next round (which is already the intended design).

**Structural template may be too rigid for later rounds.** In Round 2+, agents may need more REFINEMENT/CONCESSION/RESOLVED markers than the template allows. Mitigation: the template marks these as "optional, one sentence each" without capping the count — an agent can include multiple one-sentence refinements.

## Test Plan

- [ ] Run a 3-expert, 2-round dialogue and verify each agent output file is under 300 words
- [ ] Verify return summaries are exactly 4 lines (Perspectives/Tensions/Moves/Claim)
- [ ] Verify Judge can synthesize all agent returns without grep-based marker extraction
- [ ] Run a 12-expert dialogue and verify Judge successfully scores all agents from return summaries alone
- [ ] Verify agents in Round 1+ can still engage with prior context within the structural template
- [ ] Verify no agent exceeds 5 turns

---

*"Right then. Let's get to it."*

— Blue
