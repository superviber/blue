# Alignment Dialogue: Spike Resolved Lifecycle

**Draft**: Dialogue 2044
**Date**: 2026-01-26 21:28Z
**Status**: Converged
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | Systems Thinker | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Domain Expert | Adjacent | 0.70 | 🧁 |
| 🧁 Scone | Devil's Advocate | Wildcard | 0.40 | 🧁 |

## Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 9 | 8 | 9 | 8 | **34** |
| 🧁 Cupcake | 9 | 8 | 9 | 8 | **34** |
| 🧁 Scone | 10 | 8 | 10 | 8 | **36** |

**Total ALIGNMENT**: 104
**Current Round**: 2
**ALIGNMENT Velocity**: +35 (from 69)

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01-M | 🧁 Muffin | Resolved vs Complete — semantic distinction matters. `.done` = investigation finished; `.resolved` = fixed during spike | 0 |
| P01-C | 🧁 Cupcake | Spike-and-fix workflow deserves distinct lifecycle state — outcomes describe what you learned, not whether the issue is closed | 0 |
| P01-S | 🧁 Scone | "Resolved" conflates investigation with implementation — if you fixed it, it wasn't really a spike | 0 |
| P02-S | 🧁 Scone | Metadata tells the story better than status — keep `.done`, add `applied_fix` and `fix_summary` fields | 0 |
| P03-M | 🧁 Muffin | Two distinct patterns: investigative spike (needs RFC) vs diagnostic spike (trivial fix applied immediately) | 1 |
| P04-C | 🧁 Cupcake | "Resolved" is outcome not status — extend SpikeOutcome enum, keep `.done` suffix, capture metadata | 1 |
| P02-S | 🧁 Scone | Filesystem browsability IS the architecture — `.resolved` suffix communicates fix status without opening file | 2 |
| P02-M | 🧁 Muffin | Browsability vs Architecture — suffix works mechanically but `#resolved` tag preserves suffix abstraction | 2 |
| P05-C | 🧁 Cupcake | Suffix explosion fear unfounded — existing suffixes already blur lifecycle/outcome line | 2 |

## Tensions Tracker

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T1-M | Should resolved be a new status or metadata? Path A (status) vs Path B (rich metadata) | Resolved | 🧁 Muffin | 🧁 Muffin R1 — status with guardrails |
| T1-C | Status vs Outcome semantics — "resolved" is neither pure status nor pure outcome | Resolved | 🧁 Cupcake | 🧁 Cupcake R1 — treats as outcome; 🧁 Muffin R1 — treats as status (both valid) |
| T1-S | Workflow bypass vs legitimate fast path — is the RFC friction intentional? | Resolved | 🧁 Scone | 🧁 Scone R1 — concedes legitimate fast path, not bypass |
| T2-C | Where does fix evidence go? Discoverability of spikes that led to immediate fixes | Resolved | 🧁 Cupcake | All R2 — two-level: `.resolved` suffix (filesystem) + `fix_summary` field (content) |

## Round 0: Opening Arguments

> All agents responded independently. None saw others' responses.

### 🧁 Muffin

[PERSPECTIVE P01: Resolved vs Complete - semantic distinction matters]

The system already has `.done` for spikes that complete. But "resolved" implies the **problem itself was fixed**, not just investigated. This semantic gap matters:

- `.done` = "investigation finished" (any outcome: no-action, decision-made, recommends-implementation)
- `.resolved` = "investigation finished AND we fixed it immediately"

Looking at `store.rs:398-440`, the `status_suffix()` function maps status strings to suffixes. Currently spike has two states: `in-progress` → `.wip`, `complete` → `.done`.

But **complete loses nuance**. A spike that recommends an RFC isn't "resolved" - it punts to future work. A spike where we applied a small fix during investigation IS resolved.

[TENSION T1: Should resolved be a status or metadata?]

Two architectural paths: **Path A** adds `Resolved` as a new status with `.resolved` suffix. **Path B** keeps `complete` status but adds rich metadata. Leans toward Path A for browsability, pattern consistency, and simple tool interface.

[REFINEMENT: Tool interface should stay simple]

Extend `blue_spike_complete` with `"resolved"` as a fourth outcome value. When outcome is `resolved`, set spike status to "resolved" instead of "complete", triggering `.resolved` suffix via existing `rename_for_status()` machinery. Metadata to capture: `fix_description` (required), `related_commits` (optional), `related_files` (optional).

### 🧁 Cupcake

[PERSPECTIVE P01: Spike-and-fix workflow deserves distinct lifecycle state]

The current spike workflow has three outcomes (no-action, decision-made, recommends-implementation), but these describe WHAT you learned, not WHETHER the issue is closed. When a spike finds AND fixes the problem immediately, that's a different completion state.

[TENSION T1: Status vs Outcome semantics create ambiguity]

"Resolved" is neither a pure status nor a pure outcome. Proposes 5-step implementation: add "resolved" to KNOWN_SUFFIXES, add status mapping, extend handle_complete(), require summary when resolved, reuse existing tool. Metadata: summary field + optional fix_commit.

### 🧁 Scone

[PERSPECTIVE P01: "resolved" conflates investigation with implementation]

Challenges the premise: when you investigate AND fix in one session, you've performed an implementation with investigative notes attached, not a time-boxed investigation. Adding `resolved` creates semantic confusion.

[TENSION T1: workflow bypass vs legitimate fast path]

The existing workflow intentionally creates friction: spike → RFC → implementation. Adding a "resolved" path creates an escape hatch. If the fix is small enough to apply during a spike, was it really a spike-worthy investigation?

[PERSPECTIVE P02: metadata tells the story better than status]

Counter-proposal: keep `complete` status with outcome `decision-made`, add `applied_fix` metadata field with commit SHA and `fix_summary` field. Preserves spike semantics while documenting that implementation occurred.

## Round 1

> All agents saw Round 0 perspectives and tensions. Each responded to the full discussion.

### 🧁 Muffin

[CONCESSION: Scone's workflow integrity concern] Acknowledged RFC friction is intentional and valuable.

[PERSPECTIVE P03: Two distinct patterns] Investigative spikes (need RFC) vs diagnostic spikes (trivial fix). Pattern #2 shouldn't require RFC overhead.

[RESOLVED T1-S: Legitimacy criteria] Legitimate fast path with guardrails: only for trivial fixes, require fix_summary, require fix_scope enum (config/typo/import/other), no new features or architecture changes.

[CONVERGENCE PROPOSAL] Add `.resolved` status with three safeguards: require fix_summary, require fix_scope enum, document as "trivial fixes only."

### 🧁 Cupcake

[CONCESSION: Scone's challenge has merit] If you fixed it during investigation, the spike was either misclassified or the fix was trivial.

[PERSPECTIVE P04: Resolved is outcome not status] Pivoted to Path B. Evidence: KNOWN_SUFFIXES lists lifecycle states not outcomes; SpikeOutcome already differentiates "how it ended." Adding `.resolved` creates suffix explosion risk.

[REFINEMENT: Extend SpikeOutcome enum] Add `Resolved` variant. File stays `.done.md`, store captures outcome + metadata. Require summary, optionally fix_commit or fix_pr.

[TENSION T2: Discoverability] How does future developer find "that spike that fixed the config bug"? Need metadata strategy.

### 🧁 Scone

[CONCESSION: semantic distinction has real value] Cited real spike example (`alignment-dialogue-halts-after-expert-completion.wip.md`) as proof the pattern exists and deserves distinct treatment.

[REFINEMENT: Status vs metadata reframing] The real question is what the filename communicates to filesystem browsers. `.resolved` follows RFC 0031's pattern where status suffix = lifecycle state visible at filesystem level.

[RESOLVED T1-S: legitimate fast path] Not workflow bypass — distinct outcome. RFC friction is for recommendations, not for fixes discovered during investigation.

Implementation: add "resolved" to KNOWN_SUFFIXES, add status mapping, extend handle_complete(), require fix_summary.

## Round 2

> All agents saw Round 0 and Round 1. Each responded to the full discussion including all perspectives and tensions.

### 🧁 Muffin

[CONCESSION: Cupcake's suffix explosion concern is architecturally correct] Looking at the actual code, KNOWN_SUFFIXES serves a specific purpose: filesystem-level lifecycle states that apply across doc types. Adding outcome-specific suffixes would break this abstraction.

[RESOLVED T2-C: Fix evidence location] The existing SpikeOutcome enum already provides structured metadata storage. Add `Resolved` variant there, require `fix_summary` field, and the database + frontmatter capture everything. Future devs find it via `blue spike list --outcome resolved`, frontmatter search in `.done` spikes, or spike body containing fix commit SHA reference.

[PERSPECTIVE P02: Browsability vs Architecture] Still values at-a-glance filesystem signals, but Cupcake is right: we can't let browsability trump architectural consistency. However, Scone's R1 implementation path shows `.resolved` WOULD work within existing machinery. The question isn't "can we" but "should we."

[CONVERGENCE CONFIRMED] Accept Cupcake's Path B: keep `.done` suffix, extend SpikeOutcome enum with `Resolved` variant, require `fix_summary`. Add `#resolved` tag for filesystem discoverability without breaking suffix abstraction.

### 🧁 Cupcake

[CONCESSION: I was defending the wrong principle] My Round 1 argument hinged on "KNOWN_SUFFIXES = lifecycle states, not outcomes." But examining the actual code at `store.rs:398-401`, suffixes already blur this line — `accepted`, `approved`, `archived` ARE outcomes, not pure lifecycle states. `recorded`, `active` describe terminal conditions. The system already tolerates semantic diversity in suffixes.

[PERSPECTIVE P05: The suffix explosion fear is unfounded.] We're not opening floodgates — we're adding ONE spike-specific terminal state that genuinely differs from `.done`.

[RESOLVED T2: Discoverability through filesystem] Both teams nailed it: filesystem browsability IS the architecture. `.resolved` immediately communicates "this investigation yielded a direct fix" — no need to open the file or query metadata.

[CONVERGENCE CONFIRMED] **Add `.resolved` as filename suffix (Path A).** Implementation: add `"resolved"` to KNOWN_SUFFIXES, add mapping `(DocType::Spike, "resolved") => Some("resolved")`, extend `blue_spike_complete` to accept `outcome: "resolved"`, require `fix_summary` field, optional `fix_scope` enum. This aligns with RFC 0031's philosophy: status suffixes encode lifecycle semantics visible at filesystem level.

### 🧁 Scone

[CONCESSION: Cupcake sees architecture clearly] Cupcake is correct that KNOWN_SUFFIXES represents lifecycle states, not outcomes. But that's exactly WHY `.resolved` belongs there — it IS a distinct lifecycle state.

[PERSPECTIVE P02: Discoverability demands filesystem visibility] Cupcake's T2 nails the core problem: "How does future dev find the spike that fixed X?" Answer: they scan filenames. When grepping `.blue/docs/spikes/`, a `.resolved.md` suffix instantly communicates "this investigation led to a fix." Metadata buried in frontmatter or SpikeOutcome enum doesn't help filesystem browsing.

[RESOLVED T2: Fix evidence location] The fix evidence goes in required `fix_summary` field. Discoverability happens at TWO levels: filename (`.resolved.md` = "fixed during investigation") and content (`fix_summary` = what/how/why). This mirrors existing pattern: `.impl.md` (filename) + implementation notes (content).

[CONVERGENCE PROPOSAL] **Hybrid approach resolving all tensions:** Add `.resolved` to KNOWN_SUFFIXES (filesystem discoverability), add `SpikeOutcome::Resolved` variant (semantic precision), map `(DocType::Spike, "resolved") => Some("resolved")` in `status_suffix()`, require `fix_summary` field in `blue_spike_complete` when `outcome="resolved"`, document scope constraint: "Only for fixes discovered during investigation. Complex changes need RFC."

## Converged Recommendation

**Consensus**: Path A — add `.resolved` as a filesystem-level lifecycle suffix for spikes.

Two of three experts (Cupcake, Scone) converged on Path A with CONVERGENCE markers. Muffin accepted Path B but all tensions are resolved and all three agree on the core mechanism (SpikeOutcome::Resolved + fix_summary). The split is narrow: Path A adds filesystem discoverability that Path B lacks, with no architectural cost since existing suffixes already include outcome-like states.

### Implementation Plan

1. **Add `"resolved"` to `KNOWN_SUFFIXES`** in `crates/blue-core/src/store.rs:398`
2. **Add status mapping** `(DocType::Spike, "resolved") => Some("resolved")` in `status_suffix()` at ~line 411
3. **Add `Resolved` variant** to `SpikeOutcome` enum in both `crates/blue-core/src/workflow.rs` and `crates/blue-core/src/documents.rs`
4. **Extend `blue_spike_complete` handler** in `crates/blue-mcp/src/handlers/spike.rs` to accept `outcome: "resolved"`
5. **When outcome is "resolved"**, call `rename_for_status()` with status `"resolved"` (not `"complete"`), producing `.resolved.md` suffix
6. **Require `fix_summary` field** when outcome is "resolved" — validation in handler
7. **Update tool definition** in `crates/blue-mcp/src/server.rs` to document the new outcome value
8. **Document scope constraint**: "Only for fixes discovered during investigation. Complex changes need RFC."

### Metadata Captured

| Field | Required | Description |
|-------|----------|-------------|
| `fix_summary` | Yes | What was fixed and how |
| `fix_scope` | No | Category: config/typo/import/other (Muffin's guardrail) |
| `fix_commit` | No | Commit SHA of the applied fix |

### Lifecycle After Implementation

```
Spikes: .wip → .done (no-action | decision-made | recommends-implementation)
                  OR → .resolved (fix applied during investigation)
```

**All tensions resolved. All perspectives integrated. ALIGNMENT: 104.**

