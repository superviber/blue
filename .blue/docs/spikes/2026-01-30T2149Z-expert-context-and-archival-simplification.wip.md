# Spike: Expert Context and Archival Simplification

**Created:** 2026-01-30
**Status:** WIP

## Problem Statement

Two issues with current alignment dialogue workflow:

1. **Redundant archival**: Judge copies full expert responses into dialogue file after each round, but responses already exist on disk at `{output_dir}/round-N/*.md`. Violates ADR 0005 (Single Source).

2. **Limited context for experts**: Round 1+ experts only get the previous round's summary. They should get ALL summaries to understand the full arc of convergence.

## Current Behavior

### Judge Protocol Step 6 (Archival)
```
6. UPDATE ARCHIVAL RECORD — after writing artifacts:
   Use the Edit tool to append to {dialogue_file}:
   - Agent responses under the correct Round section  ← REDUNDANT
   - Updated Scoreboard table
   - Updated Perspectives Inventory
   - Updated Tensions Tracker
```

### Expert Context (Round 1+)
```
READ CONTEXT — THIS IS MANDATORY:
1. {output_dir}/tensions.md
2. {output_dir}/round-{N-1}.summary.md  ← ONLY previous round
3. Each .md file in {output_dir}/round-{N-1}/
```

## Proposed Changes

### 1. Simplify Archival (ADR 0005 Compliance)

**Before**: Copy full agent responses into dialogue file
**After**: Only include summary reference; full responses stay on disk

```
6. UPDATE ARCHIVAL RECORD — after writing artifacts:
   Use the Edit tool to append to {dialogue_file}:
   - Round summary (from round-N.summary.md) — NOT full agent responses
   - Updated Scoreboard table
   - Updated Perspectives Inventory
   - Updated Tensions Tracker
   Full agent responses stay in {output_dir}/round-N/*.md
```

**Benefits**:
- Follows ADR 0005: "Reference, don't copy"
- Fewer Opus API calls (no large Edit operations)
- Smaller dialogue file
- Single source of truth for expert responses

### 2. Expand Expert Context

**Before**: Only previous round summary
**After**: ALL summaries + optional access to previous round files

```
READ CONTEXT — THIS IS MANDATORY:
1. {output_dir}/tensions.md — accumulated tensions from all rounds
2. ALL summary files: {output_dir}/round-*.summary.md — full dialogue arc
You MUST read these files.

OPTIONAL — read at your discretion:
3. Previous round files in {output_dir}/round-{N-1}/ — peer perspectives
Only read these if summaries are insufficient for engaging with specific points.
```

**Benefits**:
- Experts understand full convergence trajectory
- Summaries are compact (~1-2KB each), so reading all is feasible
- Previous round files available but not required (reduces token usage)
- Experts can deep-dive when needed

## Token Budget Analysis

Current per-round expert reads:
- tensions.md: ~1-2KB
- round-{N-1}.summary.md: ~1-2KB
- round-{N-1}/*.md (5 experts): ~10-15KB
- **Total**: ~15KB

Proposed per-round expert reads:
- tensions.md: ~1-2KB
- ALL summaries (5 rounds max): ~5-10KB
- Previous round files: OPTIONAL
- **Total mandatory**: ~7-12KB (less than current!)

Optional reads add ~10-15KB if expert chooses to access them.

## Implementation

### Files to Change

1. `dialogue.rs:build_judge_protocol()` — Step 6 archival instructions (already done)
2. `dialogue.rs:handle_round_prompt()` — Context instructions for round 1+

### Context Instructions Update

```rust
let context_instructions = if round == 0 {
    String::new()
} else {
    format!(
        r#"READ CONTEXT — THIS IS MANDATORY:
Use the Read tool to read these files BEFORE writing your response:
1. {output_dir}/tensions.md — accumulated tensions from all rounds
2. ALL summary files matching {output_dir}/round-*.summary.md — the full dialogue arc

OPTIONAL — at your discretion:
3. Previous round files in {output_dir}/round-{prev}/ — peer perspectives from last round
   Only read these if the summaries are insufficient to engage with specific points.

Your response MUST engage with prior tensions and build on the convergence trajectory."#,
        output_dir = output_dir,
        prev = round - 1,
    )
};
```

## Risks

1. **Ephemeral storage**: `/tmp/blue-dialogue/` files could be deleted before `blue_dialogue_save`.
   - Mitigation: `blue_dialogue_save` should archive round files to permanent storage.

2. **Expert skips optional reads**: May miss nuance from peer responses.
   - Mitigation: Summaries should capture key points. Experts can choose to deep-dive.

## Decision

Proceed with implementation. Changes align with ADR 0005 and reduce token usage while giving experts better context.
