# RFC 0075: Document-Aware Guard and RFC Drafting Skill

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-20 |
| **Depends On** | RFC 0049 (Guard), RFC 0073 (Branch Workflow) |

---

## Summary

The guard allowlist (RFC 0049) permits any `.md` file outside `src/` and `crates/` directories. This means Claude Code (or any tool) can create RFC, ADR, spike, PRD, postmortem, and runbook files outside `.blue/docs/` without being blocked. When this happens, documents bypass Blue's lifecycle entirely: no numbering, no SQLite tracking, no status workflow, no worktree enforcement, no Jira sync.

This was observed in production: Claude Code created 4 ADRs and 9 RFCs as plain markdown files in `themove-backend/adr/` and `themove-backend/rfcs/`, completely outside Blue's tracking. The guard allowed every write because the files were `.md` and not in `src/` or `crates/`.

Additionally, when the guard *does* block a write, Claude Code has no skill or context for the correct workflow. It sees a rejected tool call but doesn't know about `blue rfc create` or `blue adr create`.

This RFC addresses both problems: the guard gap and the missing skill.

## Proposal

### Part 1: Document-Aware Guard

Extend `is_in_allowlist_sync()` to detect Blue document types being written outside `.blue/docs/` and block them with an actionable error message.

**Current behavior** (lines 267-269 of `main.rs`):

```rust
if path_str.ends_with(".md") && !path_str.contains("crates/") && !path_str.contains("src/") {
    return true; // All non-source markdown is allowed
}
```

**Proposed behavior:**

```rust
if path_str.ends_with(".md") && !path_str.contains("crates/") && !path_str.contains("src/") {
    // Block Blue document types created outside .blue/docs/
    if !path_str.contains(".blue/docs/") {
        if let Some(doc_type) = detect_blue_document_type(path) {
            eprintln!(
                "guard: blocked write to {} — this looks like a Blue {}",
                path.display(),
                doc_type
            );
            eprintln!(
                "hint: use 'blue {} create \"Title\"' to create {} documents",
                doc_type, doc_type
            );
            eprintln!(
                "hint: Blue tracks documents in .blue/docs/{}s/ with numbering, status, and workflow",
                doc_type
            );
            return false;
        }
    }
    return true;
}
```

**Detection function:**

```rust
fn detect_blue_document_type(path: &std::path::Path) -> Option<&'static str> {
    let path_lower = path.to_string_lossy().to_lowercase();
    let filename_lower = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Check filename prefix patterns (e.g., RFC-001-*, ADR-002-*, etc.)
    let prefix_patterns: &[(&str, &str)] = &[
        ("rfc-", "rfc"),
        ("rfc_", "rfc"),
        ("adr-", "adr"),
        ("adr_", "adr"),
        ("spike-", "spike"),
        ("prd-", "prd"),
        ("runbook-", "runbook"),
        ("postmortem-", "postmortem"),
    ];

    for (prefix, doc_type) in prefix_patterns {
        if filename_lower.starts_with(prefix) {
            return Some(doc_type);
        }
    }

    // Check parent directory names (e.g., rfcs/, adr/, adrs/)
    let dir_patterns: &[(&str, &str)] = &[
        ("/rfcs/", "rfc"),
        ("/rfc/", "rfc"),
        ("/adr/", "adr"),
        ("/adrs/", "adr"),
        ("/spikes/", "spike"),
        ("/prds/", "prd"),
        ("/runbooks/", "runbook"),
        ("/postmortems/", "postmortem"),
    ];

    for (dir_pattern, doc_type) in dir_patterns {
        if path_lower.contains(dir_pattern) {
            return Some(doc_type);
        }
    }

    None
}
```

**Detection strategy uses two signals:**

1. **Filename prefix** — `RFC-001-*.md`, `ADR-002-*.md`, etc. Catches the most common pattern.
2. **Parent directory name** — files inside `rfcs/`, `adr/`, `adrs/`, etc. Catches directory-organized documents.

If neither signal matches, the file is treated as a regular markdown file and allowed through (preserving existing behavior for README.md, CHANGELOG.md, docs/, etc.).

### Part 2: Document Import Command

For documents that were already created outside Blue (like the 9 RFCs and 4 ADRs in the-move-social), provide an import path:

```
blue rfc import --path ./rfcs/RFC-001-enclave-protocol.md
blue adr import --path ./adr/ADR-001-zero-knowledge-zero-trust.md
```

**Import behavior:**

1. Parse the markdown file for title, status, and sections
2. Assign the next available Blue number (NNNN)
3. Copy content into `.blue/docs/{type}s/NNNN-{status}-{slug}.md`
4. Index in SQLite
5. Print: `Imported as RFC 0076: 'Enclave Protocol' → .blue/docs/rfcs/0076-D-enclave-protocol.md`
6. Optionally delete the source file (`--delete-source`)

This is a one-time migration path, not a recurring workflow.

### Part 3: RFC Drafting Skill

Create a `/rfc` skill that guides Claude Code through the correct workflow for creating RFCs within Blue.

**Skill file:** `skills/rfc/SKILL.md`

**Trigger:** User says "draft an RFC", "create an RFC", "write an RFC", or `/rfc`

**Skill behavior:**

1. Read existing ADRs in `.blue/docs/adrs/` for governing constraints
2. Read existing RFCs in `.blue/docs/rfcs/` for context and dependencies
3. If a domain.yaml exists, read it for area/component context
4. Guide the user through:
   - **Problem statement** — what problem does this solve?
   - **Proposal** — what's the design?
   - **Dependencies** — which existing RFCs does this depend on?
   - **Governing ADRs** — which ADRs constrain this design?
5. Create the RFC via `blue rfc create "Title" --problem "..."` (shell command)
6. Fill in the full content via Edit tool on the created file in `.blue/docs/rfcs/`
7. Remind the user about the workflow: draft → approve → worktree → implement

**Key principle:** The skill ensures that Claude Code always uses `blue rfc create` as the entry point, so every RFC gets proper numbering and tracking from the start.

### Part 4: Inter-RFC Dependencies (stretch)

Add a `depends_on` field to the `Rfc` struct in `documents.rs`:

```rust
pub struct Rfc {
    pub title: String,
    pub status: Status,
    pub date: Option<String>,
    pub source_spike: Option<String>,
    pub source_prd: Option<String>,
    pub depends_on: Vec<String>,      // NEW: ["0073", "0049"]
    pub governed_by: Vec<String>,      // NEW: ADR references
    pub problem: Option<String>,
    pub proposal: Option<String>,
    pub goals: Vec<String>,
    pub non_goals: Vec<String>,
    pub plan: Vec<Task>,
}
```

Wire through:
- `blue rfc create --depends-on 0073 --depends-on 0049`
- `blue rfc list` shows dependency tree
- `blue rfc get` includes "Depends On" and "Governed By" in output
- Markdown template includes dependency section in metadata table

## Goals

- Guard blocks creation of Blue document types outside `.blue/docs/`
- Guard error messages tell the user (or Claude Code) the exact command to run instead
- Existing documents created outside Blue can be imported
- A `/rfc` skill guides AI agents through the correct RFC workflow
- Inter-RFC dependencies are tracked as first-class metadata

## Non-Goals

- Retroactively moving all existing Blue repos to the new guard behavior (this is additive)
- Changing the guard behavior for non-document markdown files (README, CHANGELOG, docs/ are unaffected)
- Building a full RFC editor UI (the skill uses the existing CLI + Edit tool)
- Enforcing dependency ordering at approve time (future RFC can add this)

## Plan

- [ ] Add `detect_blue_document_type()` function to `main.rs`
- [ ] Update `is_in_allowlist_sync()` to call detection and block with actionable message
- [ ] Add unit tests for document type detection (filename prefix + directory patterns)
- [ ] Add integration test: guard blocks `rfcs/RFC-001-foo.md` outside `.blue/docs/`
- [ ] Add integration test: guard allows `README.md`, `docs/setup.md` (no false positives)
- [ ] Add `depends_on` and `governed_by` fields to `Rfc` struct in `documents.rs`
- [ ] Wire `--depends-on` and `--governed-by` flags through `rfc create` CLI
- [ ] Update `Rfc::to_markdown()` to include dependency metadata
- [ ] Add `rfc import` and `adr import` subcommands
- [ ] Implement import: parse markdown → assign number → copy to `.blue/docs/` → index
- [ ] Create `/rfc` skill (`skills/rfc/SKILL.md`)
- [ ] Skill reads ADRs and existing RFCs for context before drafting
- [ ] Skill calls `blue rfc create` then fills content via file edit
- [ ] Run `blue install` in test repo to verify hook + skill installation
- [ ] Test end-to-end: Claude Code session triggers `/rfc`, creates RFC through Blue

## Test Plan

- [ ] Unit: `detect_blue_document_type("rfcs/RFC-001-foo.md")` returns `Some("rfc")`
- [ ] Unit: `detect_blue_document_type("adr/ADR-002-bar.md")` returns `Some("adr")`
- [ ] Unit: `detect_blue_document_type("README.md")` returns `None`
- [ ] Unit: `detect_blue_document_type("docs/setup.md")` returns `None`
- [ ] Unit: `detect_blue_document_type("plans/001-architecture.md")` returns `None`
- [ ] Integration: `blue guard --path="rfcs/RFC-001-test.md"` exits 1 with hint message
- [ ] Integration: `blue guard --path="README.md"` exits 0
- [ ] Integration: `blue guard --path=".blue/docs/rfcs/0075-D-foo.md"` exits 0
- [ ] Integration: `blue rfc import --path /tmp/test-rfc.md` creates `.blue/docs/rfcs/NNNN-D-*.md`
- [ ] Integration: `blue adr import --path /tmp/test-adr.md` creates `.blue/docs/adrs/NNNN-*.md`
- [ ] E2E: Claude Code session with guard hook installed, attempt to write `rfcs/foo.md` → blocked → uses `/rfc` skill → RFC created in `.blue/docs/`

---

*"Right then. Let's get to it."*

— Blue
