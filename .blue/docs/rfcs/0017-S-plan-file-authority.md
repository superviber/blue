# RFC 0017: Plan File Authority

| | |
|---|---|
| **Status** | Superseded |
| **Superseded By** | [RFC 0022: Filesystem Authority](./0022-filesystem-authority.md) |
| **Date** | 2026-01-26 |
| **Dialogue** | [0001: Plan Files And Dialogue Separation](../dialogues/2026-01-25-plan-files-and-dialogue-separation.dialogue.md) |

---

## Summary

RFC task tracking currently requires loading entire RFC documents to check status. This creates token inefficiency and couples operational state with design documentation. By introducing `.plan.md` companion files as the authoritative source for task state (with SQLite as a derived index rebuilt on read), we achieve: 60-80% token reduction for status operations, cleaner git diffs, surgical commits, and separation of ephemeral operational state from permanent documentation.

## Problem

1. **Token Waste**: Loading a 2000-line RFC to check task status wastes tokens
2. **Coupled Concerns**: Operational state (tasks) mixed with design documentation (RFC content)
3. **Noisy Diffs**: Task status changes pollute RFC git history
4. **Cross-Repo Limitation**: When using Blue from another repo, no plan file support exists
5. **Header Format Drift**: RFC headers have drifted between table and inline formats

### Header Format Drift

The canonical RFC header uses a metadata table:

```markdown
# RFC 0058: Remove Training Infrastructure

| | |
|---|---|
| **Status** | Accepted |
| **Created** | 2026-01-24 |
| **Dialogue** | [0058-remove-training...](./0058-remove-training...) |
```

But some RFCs use inline format:

```markdown
# RFC 0075: Spot Capacity Providers

**Status**: Draft
**Created**: 2026-01-25
**Author**: Claude
```

The table format is canonical. Blue's `to_markdown()` generates it correctly, but manually-created or external RFCs may drift.

## Architecture

### Authority Inversion

The key insight: make `.plan.md` **authoritative**, not derived.

```
CURRENT (problematic):
  RFC.md (source) → SQLite (derived) → .plan.md (derived)
  Drift accumulates at each derivation step.

PROPOSED:
  .plan.md (authoritative) → SQLite (derived index, rebuild-on-read)
  Single source of truth. SQLite is ephemeral cache.
```

### File Structure

```
.blue/docs/rfcs/
├── 0017-plan-file-authority.md       # Design documentation (permanent)
├── 0017-plan-file-authority.plan.md  # Task state (ephemeral scaffolding)
└── ...
```

### Plan File Format

```markdown
# Plan: 0017-plan-file-authority

| | |
|---|---|
| **RFC** | 0017-plan-file-authority |
| **Status** | in-progress |
| **Updated** | 2026-01-26T10:30:00Z |

## Tasks

- [x] Define architecture
- [x] Run alignment dialogue
- [ ] Implement plan file parsing
- [ ] Add rebuild-on-read to SQLite
- [ ] Update blue_rfc_plan tool
- [ ] Update blue_rfc_task_complete tool
- [ ] Write tests
```

### Atomic Update Sequence

To prevent drift during writes:

```
1. Set RFC status: "updating-plan"
2. Write .plan.md file
3. Clear RFC status flag
4. SQLite rebuilds on next read
```

If interrupted between steps 1-3, the status flag indicates recovery needed.

### SQLite as Derived Index

```rust
// On any plan query:
fn get_plan_tasks(rfc_id: &str) -> Vec<Task> {
    let plan_file = format!("{}.plan.md", rfc_id);

    if plan_file.exists() && plan_file.mtime() > cache.mtime() {
        // Rebuild cache from authoritative source
        let tasks = parse_plan_markdown(&plan_file);
        cache.rebuild(rfc_id, &tasks);
    }

    cache.get_tasks(rfc_id)
}
```

### API Unchanged

Existing tools work identically:

- `blue_rfc_plan` - Creates/updates `.plan.md` (now authoritative)
- `blue_rfc_task_complete` - Marks task done in `.plan.md`

Callers don't know or care about the authority change.

## Guardrails

From Git Workflow Expert (converged dialogue):

1. **Status Gating**: Plans only for `accepted` or `in-progress` RFCs
2. **File Limit**: Maximum 3 companion files per RFC (plan, test-plan, architecture notes)
3. **Single Responsibility**: Each companion file serves exactly one purpose

### Header Format Validation

Add `blue_lint` validation for RFC header format:

```rust
fn validate_rfc_header(content: &str) -> Result<(), LintError> {
    // Must have table format, not inline
    if content.contains("**Status**:") {
        return Err(LintError::HeaderFormat {
            expected: "| **Status** | value |",
            found: "**Status**: value",
        });
    }

    // Must have table structure
    if !content.contains("| | |\n|---|---|") {
        return Err(LintError::MissingMetadataTable);
    }

    Ok(())
}
```

The `blue_lint` tool should:
- Detect inline format (`**Status**: value`)
- Offer auto-fix to convert to table format
- Run on `blue_rfc_create` to catch manual edits

## Key Insight

> "Plans are like scaffolding - essential during construction, removed after completion."

The `.plan.md` file is **operational state**, not **documentation**. It exists to track work, then can be deleted or archived when the RFC completes. This "ephemeral framing" resolved documentation cohesion objections in the alignment dialogue.

## References

- **ADR 0005**: Single Source of Truth
- **ADR 0006**: Relationships (file co-location)
- **Dialogue 0001**: 12-expert alignment achieved 100% convergence on this architecture

## Test Plan

### Plan Files
- [ ] Create plan file for existing RFC
- [ ] Verify SQLite rebuilds from plan on read
- [ ] Verify atomic update sequence handles interruption
- [ ] Verify blue_rfc_plan creates companion file
- [ ] Verify blue_rfc_task_complete updates companion file
- [ ] Test cross-repo usage (Blue in external project)
- [ ] Verify status gating (no plans for draft RFCs)

### Header Format Validation
- [ ] Detect inline format (`**Status**: Draft`)
- [ ] Auto-fix inline to table format
- [ ] Validate table structure exists
- [ ] blue_lint reports header format errors
- [ ] blue_lint --fix converts inline to table

---

*"Right then. Let's get to it."*

— Blue
