# Spike: Consistent Table Headers In Generated Docs

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

How can we ensure all Blue-generated documents use consistent table header formatting? Currently two patterns exist: (1) headerless metadata tables `| | |\n|---|---|` used by most doc types, and (2) inline bold metadata like `**Date**: value` used by Decision and Alignment Dialogue. We need to catalog all inconsistencies, decide on a single standard, and identify the code changes needed.

---

## Findings

### Current State: 10 Document Types, 3 Metadata Patterns

**Pattern A — Headerless metadata table (the standard):**
```markdown
| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
```
Used by: RFC, Spike, ADR, Audit, Postmortem, Runbook, PRD, Dialogue — **8 of 10 types**.

**Pattern B — Inline bold with colon:**
```markdown
**Date:** 2026-01-26
```
Used by: **Decision** only (`crates/blue-core/src/documents.rs:413`).

**Pattern C — Inline bold with colon (multiline):**
```markdown
**Draft**: Dialogue 0001
**Date**: 2026-01-26 14:30
**Status**: In Progress
**Participants**: 💙 Judge, 🧁 Croissant, 🧁 Brioche
```
Used by: **Alignment Dialogue** only (`crates/blue-mcp/src/handlers/dialogue.rs:781-794`).

### Named-Column Data Tables (Already Consistent)

All data tables across doc types use proper column headers with separator rows. No inconsistency here:

| Document | Table | Columns |
|----------|-------|---------|
| Postmortem | Timeline | Time, Event |
| Postmortem | Action Items | Item, Owner, Due, Status, RFC |
| Runbook | Escalation | Level, Contact, When |
| Dialogue | Rounds | Round, Topic, Outcome |
| Alignment | Expert Panel | Agent, Role, Emoji |
| Alignment | Scoreboard | Agent, Wisdom, Consistency, Truth, Relationships, Total |
| Alignment | Perspectives | ID, Agent, Perspective, Round |
| Alignment | Tensions | ID, Tension, Status, Raised, Resolved |
| PRD | Success Metrics | Metric, Current, Target |

### Files Requiring Changes

Only **2 functions** need updating:

1. **`crates/blue-core/src/documents.rs`** — `Decision::to_markdown()` (line 413)
   - Change: `**Date:** {}` inline format → headerless metadata table
   - Add `| | |\n|---|---|` block with Date row

2. **`crates/blue-mcp/src/handlers/dialogue.rs`** — `generate_alignment_dialogue_markdown()` (lines 781-794)
   - Change: inline bold metadata block → headerless metadata table
   - Move Draft, Date, Status, Participants, RFC into table rows

### Validation Approach

The codebase already has `validate_rfc_header()` and `convert_inline_to_table_header()` in `documents.rs` (lines 585-688) that detect and convert inline headers to table format. These could be extended as a lint/migration tool for existing docs, but the primary fix is in the generation code itself.

## Recommendation

Standardize on **Pattern A** (headerless metadata table `| | |\n|---|---|`) for all document metadata. This is a 2-file, ~20-line change. Named-column data tables are already consistent and need no changes.

---

*Investigation notes by Blue*
