# RFC 0026: Unified Document Architecture

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | consistent-table-headers-in-generated-docs |
| **Supersedes** | RFC 0025 (blue-next-cortex) |

---

## Summary

Blue generates 10 document types across 2 codepaths with no shared abstraction. Each type builds markdown independently via string concatenation. Two types use inline bold metadata instead of the standard headerless table. The SQLite index parses metadata back out with regex — brittle if the format drifts.

RFC 0025 proposed wiring existing queries into `blue_next`/`blue_status` but didn't address the root cause: Blue lacks a unified document model that enforces consistent structure at generation time, enables reliable round-trip parsing, and powers both the SQLite index and the cortex routing layer.

This RFC solves the structural problem first, then absorbs RFC 0025's cortex routing as a natural consequence.

## Problem

### 1. No Shared Abstraction

Document generation lives in two places with no shared contract:

| Pattern | Location | Types |
|---------|----------|-------|
| `impl Type { fn to_markdown() }` | `blue-core/src/documents.rs` | RFC, Spike, ADR, Decision, Audit |
| `fn generate_*_markdown()` | `blue-mcp/src/handlers/*.rs` | Postmortem, Runbook, PRD, Dialogue, Alignment Dialogue |

Each function independently assembles markdown. No trait, no shared builder, no format enforcement.

### 2. Inconsistent Metadata Format

| Document Type | Metadata Format | Location |
|---------------|----------------|----------|
| RFC | `\| \| \|\n\|---\|---\|` table | `documents.rs:219` |
| Spike | `\| \| \|\n\|---\|---\|` table | `documents.rs:301` |
| ADR | `\| \| \|\n\|---\|---\|` table | `documents.rs:362` |
| Decision | `**Date:** value` inline | `documents.rs:413` |
| Audit | `\| \| \|\n\|---\|---\|` table | `documents.rs:461` |
| Postmortem | `\| \| \|\n\|---\|---\|` table | `postmortem.rs:382` |
| Runbook | `\| \| \|\n\|---\|---\|` table | `runbook.rs:262` |
| PRD | `\| \| \|\n\|---\|---\|` table | `prd.rs:308` |
| Dialogue | `\| \| \|\n\|---\|---\|` table | `dialogue.rs:640` |
| Alignment Dialogue | `**Key**: value` inline | `dialogue.rs:781` |

8 of 10 types use the table format. 2 deviate. The parser (`update_markdown_status`) tries both formats with fallback regex — this works but is fragile.

### 3. Parser Depends on Generator Consistency

The SQLite index pipeline:
1. `parse_document_from_file()` extracts title, number, status, doc_type from markdown
2. Status extraction regex: `\|\s*\*\*Status\*\*\s*\|\s*([^|]+)\s*\|`
3. Falls back to inline format if table format not found
4. `reconcile()` re-parses files on disk and diffs against DB

If a document type generates markdown the parser doesn't expect, the index silently gets wrong data or misses fields.

### 4. RFC 0025 Built on a Shaky Foundation

RFC 0025's cortex routing (`blue_next` priority chain, `blue_status` dashboard) queries the SQLite index. If the index can't reliably parse all document types, the cortex gives incomplete answers. The fix order must be: **structure → parsing → routing**.

## Document Type Inventory

### Complete Type Registry

| # | Type | DB `doc_type` | Numbered | Has Status | Has Plan | Metadata Fields | Generation Location |
|---|------|--------------|----------|------------|----------|----------------|-------------------|
| 1 | RFC | `rfc` | Yes (0001+) | Yes | Yes | Status, Date, Source Spike, Source PRD | `documents.rs:208` |
| 2 | Spike | `spike` | No | Yes | No | Status, Date, Time Box, Outcome | `documents.rs:296` |
| 3 | ADR | `adr` | Yes (0000+) | Yes | No | Status, Date, RFC | `documents.rs:353` |
| 4 | Decision | `decision` | No | No | No | Date (inline only) | `documents.rs:409` |
| 5 | Audit | `audit` | No | Yes | No | Status, Date, Type, Scope | `documents.rs:456` |
| 6 | Postmortem | `postmortem` | No | No | No | Date, Severity, Duration, Author | `postmortem.rs:364` |
| 7 | Runbook | `runbook` | No | Yes | No | Status, Actions, Owner, Created, Source RFC | `runbook.rs:245` |
| 8 | PRD | `prd` | Yes (0001+) | Yes | No | Status, Author, Created, Stakeholders | `prd.rs:278` |
| 9 | Dialogue | `dialogue` | Yes (0001+) | Yes | No | Date, Status, RFC | `dialogue.rs:620` |
| 10 | Alignment Dialogue | `dialogue` | Yes (0001+) | Yes | No | Draft, Date, Status, Participants, RFC | `dialogue.rs:763` |

### SQLite Index Integration Assessment

| Type | Indexed in DB | Status Parsed | Number Parsed | Content Hash | Reconciled | FTS Searchable |
|------|:---:|:---:|:---:|:---:|:---:|:---:|
| RFC | Yes | Yes | Yes | Yes | Yes | Yes |
| Spike | Yes | Yes | No | Yes | Yes | Yes |
| ADR | Yes | Yes | Yes | Yes | Yes | Yes |
| Decision | Yes | **Fallback** | No | Yes | Yes | Yes |
| Audit | Yes | Yes | No | Yes | Yes | Yes |
| Postmortem | Yes | **Fallback** | No | Yes | Yes | Yes |
| Runbook | Yes | Yes | No | Yes | Yes | Yes |
| PRD | Yes | Yes | Yes | Yes | Yes | Yes |
| Dialogue | Yes | Yes | Yes | Yes | Yes | Yes |
| Alignment Dialogue | Yes | **Fallback** | Yes | Yes | Yes | Yes |

"Fallback" means the primary table-format regex fails and the parser falls back to inline detection. This works today but isn't guaranteed.

### Named-Column Data Tables (Already Consistent)

| Document | Table | Columns |
|----------|-------|---------|
| Postmortem | Timeline | Time, Event |
| Postmortem | Action Items | Item, Owner, Due, Status, RFC |
| Runbook | Escalation | Level, Contact, When |
| Dialogue | Rounds | Round, Topic, Outcome |
| Alignment Dialogue | Expert Panel | Agent, Role, Emoji |
| Alignment Dialogue | Scoreboard | Agent, Wisdom, Consistency, Truth, Relationships, Total |
| Alignment Dialogue | Perspectives | ID, Agent, Perspective, Round |
| Alignment Dialogue | Tensions | ID, Tension, Status, Raised, Resolved |
| PRD | Success Metrics | Metric, Current, Target |

These already use proper column headers. No changes needed.

## Design

### Principle

> One shape, many contents. The format is the contract.

### Phase 1: Document Trait — The Abstract Solution

Introduce a `BlueDocument` trait in `blue-core` that all document types implement:

```rust
/// The contract every Blue document must satisfy.
pub trait BlueDocument {
    /// Document type identifier (matches DocType enum)
    fn doc_type(&self) -> DocType;

    /// Document title
    fn title(&self) -> &str;

    /// Optional document number (RFCs, ADRs, PRDs, Dialogues)
    fn number(&self) -> Option<u32> { None }

    /// Metadata key-value pairs for the header table.
    /// Every implementation MUST include Status if the type has one.
    fn metadata(&self) -> Vec<(&str, String)>;

    /// Ordered list of body sections: (heading, content).
    /// Content is raw markdown.
    fn sections(&self) -> Vec<(String, String)>;

    /// Optional signature line at the bottom.
    fn signature(&self) -> Option<&str> { None }

    /// Render to markdown. Default implementation enforces the standard format.
    /// Types SHOULD NOT override this.
    fn to_markdown(&self) -> String {
        let mut md = String::new();

        // Title line
        match self.number() {
            Some(n) => md.push_str(&format!(
                "# {} {:04}: {}\n\n",
                self.doc_type().prefix(),
                n,
                self.title()
            )),
            None => md.push_str(&format!(
                "# {}: {}\n\n",
                self.doc_type().prefix(),
                self.title()
            )),
        }

        // Metadata table — ALWAYS headerless, ALWAYS table format
        let meta = self.metadata();
        if !meta.is_empty() {
            md.push_str("| | |\n|---|---|\n");
            for (key, value) in &meta {
                md.push_str(&format!("| **{}** | {} |\n", key, value));
            }
            md.push_str("\n---\n\n");
        }

        // Body sections
        for (heading, content) in self.sections() {
            md.push_str(&format!("## {}\n\n", heading));
            md.push_str(&content);
            md.push_str("\n\n");
        }

        // Signature
        if let Some(sig) = self.signature() {
            md.push_str("---\n\n");
            md.push_str(sig);
            md.push('\n');
        }

        md
    }
}
```

Key design decisions:
- `metadata()` returns `Vec<(&str, String)>` — ordered, typed key-value pairs
- `to_markdown()` has a **default implementation** that enforces the table format — types don't override it
- `sections()` allows type-specific body content (embedded tables, checklists, etc.)
- The trait lives in `blue-core`, keeping handlers in `blue-mcp` as thin wrappers

### Phase 2: Migrate Existing Types

Each of the 10 types implements `BlueDocument`:

| Type | `metadata()` returns | Changes required |
|------|---------------------|-----------------|
| RFC | Status, Date, Source Spike?, Source PRD? | Replace `to_markdown()` body |
| Spike | Status, Date, Time Box?, Outcome? | Replace `to_markdown()` body |
| ADR | Status, Date, RFC? | Replace `to_markdown()` body |
| Decision | Date | **Add table format** (currently inline) |
| Audit | Status, Date, Type, Scope | Replace `to_markdown()` body |
| Postmortem | Date, Severity, Duration?, Author | Move from handler fn to trait impl |
| Runbook | Status, Actions?, Owner?, Created, Source RFC? | Move from handler fn to trait impl |
| PRD | Status, Author, Created, Stakeholders | Move from handler fn to trait impl |
| Dialogue | Date, Status, RFC? | Move from handler fn to trait impl |
| Alignment Dialogue | Draft, Date, Status, Participants, RFC? | **Add table format** (currently inline) |

### Phase 3: Unified Parser

Replace the regex-based parser with a structured one that assumes the trait's output format:

```rust
pub fn parse_metadata_table(content: &str) -> Vec<(String, String)> {
    // Find the headerless table after the title
    // Pattern: | | |\n|---|---|\n followed by | **Key** | Value | rows
    // Returns ordered key-value pairs
}
```

This eliminates the fallback path. One format in, one parser out.

### Phase 4: Cortex Routing (Absorbs RFC 0025)

With reliable parsing across all types, `blue_status` and `blue_next` can query the full document landscape:

1. **`blue_status`**: Show active work across all 10 types, not just RFCs
2. **`blue_next`**: Priority chain from RFC 0025 — postmortem actions > reminders > stalled RFCs > ready RFCs > stale runbooks
3. **ADR auto-check at RFC creation**: existing function, new call site
4. **Proactive runbook lookup**: at spike completion and RFC status change

All of RFC 0025's changes are preserved. The difference: they now operate on a reliable, uniform data model.

## Files Changed

| File | Change | Phase |
|------|--------|-------|
| `crates/blue-core/src/documents.rs` | Add `BlueDocument` trait; implement for RFC, Spike, ADR, Decision, Audit | 1-2 |
| `crates/blue-core/src/lib.rs` | Export `BlueDocument` trait | 1 |
| `crates/blue-mcp/src/handlers/postmortem.rs` | Move struct + `BlueDocument` impl to core, handler calls trait | 2 |
| `crates/blue-mcp/src/handlers/runbook.rs` | Move struct + `BlueDocument` impl to core, handler calls trait | 2 |
| `crates/blue-mcp/src/handlers/prd.rs` | Move struct + `BlueDocument` impl to core, handler calls trait | 2 |
| `crates/blue-mcp/src/handlers/dialogue.rs` | Move struct + `BlueDocument` impl to core, handler calls trait | 2 |
| `crates/blue-core/src/store.rs` | Replace regex parser with `parse_metadata_table()` | 3 |
| `crates/blue-mcp/src/server.rs` | Enrich `handle_status()` and `handle_next()` per RFC 0025 design | 4 |

## What This Does NOT Do

- No new database tables or schema changes
- No new MCP tool definitions
- No templating engine (the trait IS the template)
- No breaking changes to existing documents on disk (parser retains fallback for legacy files)

## ADR Alignment

| ADR | How Served |
|-----|-----------|
| 0. Never Give Up | Overdue postmortem actions surface first (Phase 4) |
| 2. Presence | Full system state visible through blue_status (Phase 4) |
| 4. Evidence | Metadata table is parseable evidence, not free-form prose |
| 5. Single Source | One trait defines the format; one parser reads it |
| 6. Relationships | Cross-type awareness via unified index (Phase 4) |
| 7. Integrity | Every document type has the same structural contract |
| 10. No Dead Code | Eliminates duplicated markdown assembly; removes fallback paths |
| 11. Freedom Through Constraint | The trait constrains format, freeing reliable parsing |

## Test Plan

- [ ] `BlueDocument` trait compiles with default `to_markdown()` implementation
- [ ] All 10 types implement `BlueDocument`
- [ ] Every type's `to_markdown()` output starts with `| | |\n|---|---|` metadata table
- [ ] Decision output uses table format (not inline bold)
- [ ] Alignment Dialogue output uses table format (not inline bold)
- [ ] `parse_metadata_table()` extracts all key-value pairs from trait output
- [ ] `parse_document_from_file()` works without fallback path for new documents
- [ ] Legacy inline-format documents still parse (backward compat)
- [ ] `blue_status` returns postmortem actions, stale runbooks, overdue reminders
- [ ] `blue_next` priority chain: postmortem P1/P2 > reminders > stalled RFC > ready RFC > stale runbooks
- [ ] `blue_rfc_create` returns relevant ADRs in response
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean

---

*"One shape, many contents. The format is the contract."*

— Blue
