# RFC 0035: Spike Resolved Lifecycle Suffix

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-27 |
| **Source Dialogue** | 2026-01-26T2128Z-spike-resolved-lifecycle |

---

## Summary

Add `.resolved.md` as a filesystem-level lifecycle suffix for spikes where the investigation discovered and immediately fixed the problem. This extends the existing spike lifecycle (`.wip.md` -> `.done.md`) with a new terminal state that communicates "fix applied during investigation" at a glance.

## Problem

The current spike workflow has three outcomes (`no-action`, `decision-made`, `recommends-implementation`), but all non-RFC completions produce the same `.done.md` suffix. When a spike discovers a trivial fix and applies it immediately, that information is lost in the filename. A developer browsing `.blue/docs/spikes/` cannot distinguish "investigated, no action needed" from "investigated and fixed it" without opening each file.

Real example: `2026-01-26T2122Z-alignment-dialogue-halts-after-expert-completion.wip.md` — this spike found the root cause (`run_in_background: true`) and identified the fix. It should end as `.resolved.md`, not `.done.md`.

## Design

### Lifecycle After Implementation

```
Spikes: .wip.md -> .done.md      (no-action | decision-made | recommends-implementation)
                -> .resolved.md  (fix applied during investigation)
```

### Changes Required

**1. `crates/blue-core/src/store.rs`**

Add `"resolved"` to `KNOWN_SUFFIXES` and add the mapping:

```rust
(DocType::Spike, "resolved") => Some("resolved"),
```

**2. `crates/blue-core/src/workflow.rs`**

Add `Resolved` variant to `SpikeOutcome` enum:

```rust
pub enum SpikeOutcome {
    NoAction,
    DecisionMade,
    RecommendsImplementation,
    Resolved,  // Fix applied during investigation
}
```

Update `as_str()` and `parse()` implementations.

Add `Resolved` variant to `SpikeStatus` enum:

```rust
pub enum SpikeStatus {
    InProgress,
    Completed,
    Resolved,  // Investigation led directly to fix
}
```

Update `as_str()` and `parse()` implementations.

**3. `crates/blue-core/src/documents.rs`**

Add `Resolved` variant to the duplicate `SpikeOutcome` enum and its `as_str()`.

**4. `crates/blue-mcp/src/handlers/spike.rs`**

Extend `handle_complete()`:
- Accept `"resolved"` as an outcome value
- Require `fix_summary` parameter when outcome is "resolved" (return error if missing)
- Use status `"resolved"` for `update_document_status()`, `rename_for_status()`, and `update_markdown_status()`

**5. `crates/blue-mcp/src/server.rs`**

Update `blue_spike_complete` tool definition:
- Add `"resolved"` to the outcome enum
- Add `fix_summary` property: "What was fixed and how (required when outcome is resolved)"

### Metadata Captured

| Field | Required | Description |
|-------|----------|-------------|
| `fix_summary` | Yes (when resolved) | What was fixed and how |
| `summary` | No | General investigation findings |

### Scope Constraint

The `resolved` outcome is only for fixes discovered during investigation. Complex changes that require design decisions, new features, or architectural changes still need the `recommends-implementation` -> RFC path.

## Alternatives Considered

**Path B: Outcome-only with `.done.md` suffix** — Keep `.done.md` for all completions, add `Resolved` only to `SpikeOutcome` enum, use metadata/tags for discoverability. Rejected because filesystem browsability is the primary discovery mechanism and existing suffixes (`.accepted.md`, `.archived.md`) already include outcome-like states.

Both paths were debated across 3 rounds of alignment dialogue. Path A won 2-of-3 (Cupcake, Scone) with all tensions resolved.

## Test Plan

- [ ] `cargo build` compiles without errors
- [ ] `cargo test` passes all existing tests
- [ ] `cargo clippy` produces no warnings
- [ ] `blue_spike_complete` with `outcome: "resolved"` and `fix_summary` produces `.resolved.md` file
- [ ] `blue_spike_complete` with `outcome: "resolved"` without `fix_summary` returns error
- [ ] Existing outcomes (`no-action`, `decision-made`, `recommends-implementation`) work unchanged
