# RFC 0031: Document Lifecycle Filenames

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | Document Lifecycle Filenames |
| **Supersedes** | RFC 0030 (ISO 8601 Document Filename Timestamps) |
| **Dialogue** | document-lifecycle-filenames-rfc-design (Converged, 3 rounds, 12 experts, 100%) |

---

## Summary

Blue documents store lifecycle status in SQLite and markdown frontmatter, but filenames reveal nothing about document state. Browsing a directory of 15+ spikes or RFCs requires opening each file to determine if it's a draft, in-progress, complete, or superseded. This RFC combines ISO 8601 timestamps (from RFC 0030) with status-in-filename visibility to create a unified document lifecycle filename convention across all 9 document types.

## Problem

### Timestamp Problem (from RFC 0030)

Date-prefixed documents use `YYYY-MM-DD` format. On a productive day this creates 15+ files with identical prefixes and no temporal ordering. The 5 affected handlers also use mixed timezones (3 UTC, 2 Local).

### Status Visibility Problem (new)

Nine document types have lifecycle statuses stored only in SQLite + markdown frontmatter:

| Type | Current Pattern | Statuses | Browse Problem |
|---|---|---|---|
| RFC | `0030-slug.md` | draft, accepted, in-progress, implemented, superseded | Can't tell if draft or shipped |
| Spike | `2026-01-26-slug.md` | in-progress, complete (+outcome) | Can't tell if resolved |
| ADR | `0004-slug.md` | accepted, in-progress, implemented | Can't tell if active |
| Decision | `2026-01-26-slug.md` | recorded | Always same (no problem) |
| PRD | `0001-slug.md` | draft, approved, implemented | Can't tell if approved |
| Postmortem | `2026-01-26-slug.md` | open, closed | Can't tell if resolved |
| Runbook | `slug.md` | active, archived | Can't tell if current |
| Dialogue | `2026-01-26-slug.dialogue.md` | draft, published | Can't tell if final |
| Audit | `2026-01-26-slug.md` | in-progress, complete | Can't tell if done |

You cannot determine document state without opening every file.

## Design

### Part 1: ISO 8601 Timestamps (from RFC 0030)

#### New Timestamp Format

```
YYYY-MM-DDTHHMMZ-slug.md
```

ISO 8601 filename-safe hybrid notation: extended date (`YYYY-MM-DD`) with basic time (`HHMM`), `T` separator, and `Z` suffix for UTC. Colons omitted for cross-platform filesystem safety.

**Examples:**
```
Before: 2026-01-26-native-kanban-apps-for-blue.md
After:  2026-01-26T0856Z-native-kanban-apps-for-blue.md
```

#### Affected Document Types (timestamps)

| Document Type | Handler File | Current TZ | Change |
|---|---|---|---|
| Spike | `spike.rs:33` | UTC | Format `%Y-%m-%dT%H%MZ` |
| Dialogue | `dialogue.rs:348` | Local | Switch to UTC + new format |
| Decision | `decision.rs:42` | UTC | New format |
| Postmortem | `postmortem.rs:83` | Local | Switch to UTC + new format |
| Audit | `audit_doc.rs:37` | UTC | New format |

**Not affected:** RFCs, ADRs, PRDs, Runbooks (numbered prefixes, not dates).

#### Shared Timestamp Helper

```rust
/// Get current UTC timestamp in ISO 8601 filename-safe format
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H%MZ").to_string()
}
```

### Part 2: Status-in-Filename

#### Approach: Status Suffix Before `.md`

Encode document lifecycle status as a dot-separated suffix before the file extension:

```
{prefix}-{slug}.{status}.md
```

When status is the default/initial state, the suffix is omitted (no visual noise for new documents).

#### Complete Filename Format by Type

**Date-prefixed types (5 types):**
```
2026-01-26T0856Z-slug.md                    # spike: in-progress (default, no suffix)
2026-01-26T0856Z-slug.done.md               # spike: complete (any outcome)

2026-01-26T0912Z-slug.dialogue.md           # dialogue: draft (default)
2026-01-26T0912Z-slug.dialogue.pub.md       # dialogue: published

2026-01-26T0930Z-slug.md                    # decision: recorded (always, no suffix)

2026-01-26T1015Z-slug.md                    # postmortem: open (default)
2026-01-26T1015Z-slug.closed.md             # postmortem: closed

2026-01-26T1100Z-slug.md                    # audit: in-progress (default)
2026-01-26T1100Z-slug.done.md               # audit: complete
```

**Number-prefixed types (3 types):**
```
0031-slug.md                                 # RFC: draft (default, no suffix)
0031-slug.accepted.md                        # RFC: accepted
0031-slug.wip.md                             # RFC: in-progress
0031-slug.impl.md                            # RFC: implemented
0031-slug.super.md                           # RFC: superseded

0004-slug.md                                 # ADR: accepted (default, no suffix)
0004-slug.impl.md                            # ADR: implemented

0001-slug.md                                 # PRD: draft (default, no suffix)
0001-slug.approved.md                        # PRD: approved
0001-slug.impl.md                            # PRD: implemented
```

**No-prefix types (1 type):**
```
slug.md                                      # runbook: active (default, no suffix)
slug.archived.md                             # runbook: archived
```

#### Status Abbreviation Vocabulary

A consistent set of short status tags across all document types:

| Tag | Meaning | Used By |
|---|---|---|
| (none) | Default/initial state | All types |
| `.done` | Complete/closed | Spike, Audit, Postmortem |
| `.impl` | Implemented | RFC, ADR, PRD |
| `.super` | Superseded | RFC |
| `.accepted` | Accepted/approved | RFC |
| `.approved` | Approved | PRD |
| `.wip` | In-progress (active work) | RFC |
| `.closed` | Closed | Postmortem |
| `.pub` | Published | Dialogue |
| `.archived` | Archived/inactive | Runbook |

#### Design Principle: Store Authority

The SQLite store is the authoritative source of document status. Filenames are derived views. If filename and store disagree, the store wins. `blue_sync` reconciles.

#### Default-State Omission

Files without status suffixes are in their initial state. Within each document type's directory, absence of a suffix unambiguously means the initial/default state for that type. Legacy files created before this RFC are treated identically -- no migration required.

#### The Rename Problem

Status-in-filename requires renaming files when status changes. Consequences:

1. **Git history**: `git log --follow` tracks renames, but `git blame` shows only current name
2. **Cross-references**: Markdown links like `[RFC 0031](../rfcs/0031-slug.md)` break on rename
3. **External bookmarks**: Browser bookmarks, shell aliases break
4. **SQLite file_path**: Must update `documents.file_path` on every rename

**Mitigations:**
- Update `file_path` in store on every status change (already touches store + markdown)
- Cross-references use title-based lookups, not filename -- most survive
- Git detects renames automatically via content similarity (`git diff --find-renames`); no explicit `git mv` needed
- Accept that external bookmarks break (they already break on file deletion)

#### Overwrite Protection

Document creation handlers call `fs::write` without checking file existence. If two documents with identical slugs are created in the same UTC minute, the second silently overwrites the first. All 5 date-prefixed handlers must check file existence before writing:

```rust
let path = docs_path.join(&filename);
if path.exists() {
    return Err(anyhow!("File already exists: {}", filename));
}
fs::write(&path, content)?;
```

This is a prerequisite for status suffixes, not optional future work.

### Code Changes

#### 1. Shared helpers (blue-core)

```rust
/// Get current UTC timestamp in ISO 8601 filename-safe format
pub fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H%MZ").to_string()
}

/// Map document status to filename suffix
pub fn status_suffix(doc_type: DocType, status: &str) -> Option<&'static str> {
    match (doc_type, status) {
        // Default states: no suffix
        (DocType::Spike, "in-progress") => None,
        (DocType::Rfc, "draft") => None,
        (DocType::Adr, "accepted") => None,
        (DocType::Prd, "draft") => None,
        (DocType::Decision, "recorded") => None,
        (DocType::Postmortem, "open") => None,
        (DocType::Runbook, "active") => None,
        (DocType::Dialogue, "draft") => None,
        (DocType::Audit, "in-progress") => None,

        // Spike outcomes
        (DocType::Spike, "complete") => Some("done"),

        // RFC lifecycle
        (DocType::Rfc, "accepted") => Some("accepted"),
        (DocType::Rfc, "in-progress") => Some("wip"),
        (DocType::Rfc, "implemented") => Some("impl"),
        (DocType::Rfc, "superseded") => Some("super"),

        // ADR
        (DocType::Adr, "implemented") => Some("impl"),

        // PRD
        (DocType::Prd, "approved") => Some("approved"),
        (DocType::Prd, "implemented") => Some("impl"),

        // Postmortem
        (DocType::Postmortem, "closed") => Some("closed"),

        // Runbook
        (DocType::Runbook, "archived") => Some("archived"),

        // Dialogue
        (DocType::Dialogue, "published") => Some("pub"),

        // Audit
        (DocType::Audit, "complete") => Some("done"),

        _ => None,
    }
}
```

#### 2. Rename-on-status-change

Each handler's `update_status` path gains a rename step. Filesystem-first with rollback:

```rust
fn rename_for_status(state: &ProjectState, doc: &Document, new_status: &str) -> Result<(), Error> {
    if let Some(ref old_path) = doc.file_path {
        let old_full = state.home.docs_path.join(old_path);
        let new_suffix = status_suffix(doc.doc_type, new_status);
        let new_filename = rebuild_filename(old_path, new_suffix);
        let new_full = state.home.docs_path.join(&new_filename);

        if old_full != new_full {
            // Step 1: Rename file (filesystem-first)
            fs::rename(&old_full, &new_full)?;

            // Step 2: Update store — rollback rename on failure
            if let Err(e) = state.store.update_document_file_path(doc.doc_type, &doc.title, &new_filename) {
                // Attempt rollback
                if let Err(rollback_err) = fs::rename(&new_full, &old_full) {
                    eprintln!("CRITICAL: rename rollback failed. File at {:?}, store expects {:?}. Rollback error: {}",
                        new_full, old_path, rollback_err);
                }
                return Err(e);
            }

            // Step 3: Update markdown frontmatter status (non-critical)
            if let Err(e) = update_markdown_status(&new_full, new_status) {
                eprintln!("WARNING: frontmatter update failed for {:?}: {}. Store is authoritative.", new_full, e);
            }
        }
    }
    Ok(())
}
```

#### 3. Handler timestamp updates (5 handlers)

Same changes as RFC 0030: replace `%Y-%m-%d` with `%Y-%m-%dT%H%MZ` in spike.rs, dialogue.rs, decision.rs, postmortem.rs, audit_doc.rs. Standardize all to `chrono::Utc::now()`.

### Backwards Compatibility

**No migration needed.** The spike investigation confirmed:

1. **No code parses dates from filenames.** The only filename regex (`store.rs:2232`) extracts RFC/ADR *numbers* (`^\d{4}-`), not dates.
2. **Existing files keep their names.** Old `2026-01-26-slug.md` files continue to work. New files get the new format.
3. **Document lookups use the SQLite store**, not filename patterns.
4. **Status suffixes are additive.** Existing files without suffixes are treated as default state.

### Spike Outcome Visibility

For the user's specific request -- seeing spike outcomes from filenames:

| Outcome | Filename Example |
|---|---|
| In-progress | `2026-01-26T0856Z-kanban-apps.md` |
| Complete (any outcome) | `2026-01-26T0856Z-kanban-apps.done.md` |

All completed spikes get `.done` regardless of outcome. The specific outcome (no-action, decision-made, recommends-implementation) is recorded in the markdown `## Outcome` section and the SQLite `outcome` field. Spike-to-RFC linkage lives in the RFC's `source_spike` field, not the spike filename.

## Test Plan

- [ ] Unit test: `utc_timestamp()` produces format matching `^\d{4}-\d{2}-\d{2}T\d{4}Z$`
- [ ] Unit test: `status_suffix()` returns correct suffix for all 9 doc types and all statuses
- [ ] Unit test: `rebuild_filename()` correctly inserts/removes/changes status suffix
- [ ] Integration: Create one of each affected document type, verify filename matches new format
- [ ] Integration: Change status on a document, verify file is renamed and store is updated
- [ ] Integration: Verify existing `YYYY-MM-DD-slug.md` files still load and are findable by title
- [ ] Integration: Verify `scan_filesystem_max` regex still works (only applies to numbered docs)
- [ ] Integration: Verify `fs::rename` failure leaves store unchanged
- [ ] Integration: Verify store update failure after rename triggers rollback rename
- [ ] Integration: Verify legacy files (pre-RFC) without suffixes are treated as default state
- [ ] Integration: Verify overwrite protection rejects duplicate filenames within same UTC minute

## Future Work

- **Audit slug bug:** `audit_doc.rs:37` uses raw title instead of `title_to_slug()` for filenames. Fix independently.
- **Cross-reference updater:** A `blue_rename` tool that updates markdown cross-references when files are renamed. Not required for MVP but useful long-term.
- **Auto-complete source spike:** When `rfc_create` is called with `source_spike`, auto-complete the source spike with `decision-made` outcome. This closes the spike-to-RFC workflow loop without manual intervention.

---

*"Right then. Let's get to it."*

-- Blue
