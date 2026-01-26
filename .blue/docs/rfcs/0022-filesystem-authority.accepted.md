# RFC 0022: Filesystem Authority

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-26 |
| **Supersedes** | RFC 0017, RFC 0018, RFC 0021 |
| **Alignment** | 12-expert dialogue, 94% convergence (2 rounds) |

---

## Principle

**Filesystem is truth. Database is derived index.**

The `.blue/docs/` directory is the single source of truth for all Blue documents. SQLite (`blue.db`) is a rebuildable cache that accelerates queries but holds no authoritative state. If the database is deleted, the system must fully recover from filesystem content alone.

## Scope

This RFC establishes filesystem authority for:
- **Plan files** (`.plan.md` companions) - task state tracking
- **Document sync** - RFC, Spike, ADR, Dialogue import/reconciliation
- **Number allocation** - collision-free document numbering

## Architecture

```
                     ┌─────────────────────────────┐
                     │    .blue/docs/ (TRUTH)      │
                     │                             │
                     │  rfcs/*.md                  │
                     │  spikes/*.md                │
                     │  adrs/*.md                  │
                     │  dialogues/*.md             │
                     │  *.plan.md                  │
                     └──────────────┬──────────────┘
                                    │
                         scan / hash / rebuild
                                    │
                                    ▼
                     ┌─────────────────────────────┐
                     │   blue.db (DERIVED INDEX)   │
                     │                             │
                     │  Fast queries               │
                     │  Relationship cache         │
                     │  Rebuildable on demand      │
                     └─────────────────────────────┘
```

## Mechanisms

### 1. Rebuild-on-Read

When accessing document state, check filesystem freshness:

```rust
fn get_document(&self, doc_type: DocType, query: &str) -> Result<Document> {
    // Try database first (fast path)
    if let Ok(doc) = self.find_in_db(doc_type, query) {
        if !self.is_stale(&doc) {
            return Ok(doc);
        }
    }

    // Fall back to filesystem scan (authoritative)
    self.scan_and_register(doc_type, query)
}
```

### 1b. Slug-Based Document Lookup

`find_document()` must match documents by slug (kebab-case), not just exact title. When a tool calls `find_document(DocType::Rfc, "filesystem-authority")`, it should match the file `0022-filesystem-authority.md` regardless of whether the DB stores the title as "Filesystem Authority".

```rust
fn find_document(&self, doc_type: DocType, query: &str) -> Result<Document> {
    // Try exact title match in DB
    if let Ok(doc) = self.find_by_title(doc_type, query) {
        return Ok(doc);
    }

    // Try slug match (kebab-case → title)
    if let Ok(doc) = self.find_by_slug(doc_type, query) {
        return Ok(doc);
    }

    // Fall back to filesystem scan
    self.scan_and_register(doc_type, query)
}
```

**Observed bug**: After creating `0022-filesystem-authority.md` on disk, `blue_worktree_create` with title `filesystem-authority` failed even after `blue sync`. Only the exact title `Filesystem Authority` worked. All tool-facing lookups must support slug matching.

### 2. Staleness Detection

Two-tier check (from RFC 0018):

```rust
fn is_stale(&self, doc: &Document) -> bool {
    let Some(path) = &doc.file_path else { return true };

    // Fast path: mtime comparison
    let file_mtime = fs::metadata(path).modified().ok();
    let indexed_at = doc.indexed_at.as_ref().and_then(|t| parse_timestamp(t));

    if let (Some(fm), Some(ia)) = (file_mtime, indexed_at) {
        if fm <= ia { return false; }  // Not modified since indexing
    }

    // Slow path: content hash verification
    let content = fs::read_to_string(path).ok();
    content.map(|c| hash_content(&c) != doc.content_hash.as_deref().unwrap_or(""))
           .unwrap_or(true)
}
```

### 3. Filesystem-Aware Numbering

When allocating document numbers, scan both sources (from RFC 0021):

```rust
pub fn next_number(&self, doc_type: DocType) -> Result<i32> {
    // Database max (fast, possibly stale)
    let db_max: Option<i32> = self.conn.query_row(
        "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
        params![doc_type.as_str()],
        |row| row.get(0),
    )?;

    // Filesystem max (authoritative)
    let fs_max = self.scan_filesystem_max(doc_type)?;

    // Take max of both - filesystem wins
    Ok(std::cmp::max(db_max.unwrap_or(0), fs_max) + 1)
}
```

### 4. Reconciliation (`blue sync`)

Explicit command to align database with filesystem:

```bash
blue sync              # Full reconciliation
blue sync --dry-run    # Report drift without fixing
blue sync rfcs/        # Scope to directory
```

Reconciliation rules:

| Condition | Action |
|-----------|--------|
| File exists, no DB record | Create DB record from file |
| DB record exists, no file | Soft-delete (`deleted_at = now()`) |
| Both exist, hash mismatch | Update DB from file |

### 5. Plan File Authority

Plan files (`.plan.md`) are authoritative for task state:

```
.blue/docs/rfcs/
├── 0022-filesystem-authority.md       # Design (permanent)
└── 0022-filesystem-authority.plan.md  # Tasks (operational)
```

The plan file is parsed on access; SQLite task state is derived and rebuilt as needed.

## Database Schema

Required fields for filesystem authority:

```sql
ALTER TABLE documents ADD COLUMN content_hash TEXT;
ALTER TABLE documents ADD COLUMN indexed_at TEXT;
ALTER TABLE documents ADD COLUMN deleted_at TEXT;  -- Soft-delete

CREATE INDEX idx_documents_deleted
    ON documents(deleted_at) WHERE deleted_at IS NOT NULL;
```

## Guardrails

1. **Never auto-fix** - `blue status` reports drift; `blue sync` fixes it explicitly
2. **Soft-delete only** - DB records for missing files get `deleted_at`, never hard-deleted
3. **30-day retention** - Soft-deleted records purged via `blue purge --older-than 30d`
4. **Gitignore blue.db** - Database is generated artifact, not version-controlled

## ADR Alignment

| ADR | How Honored |
|-----|-------------|
| ADR 0005 (Single Source) | Filesystem is the one truth; database derives from it |
| ADR 0006 (Relationships) | Links stored in DB but validated against filesystem |
| ADR 0007 (Integrity) | Structure (filesystem) matches principle (authority) |
| ADR 0010 (No Dead Code) | Three RFCs consolidated; redundancy eliminated |

## Test Plan

### Core Authority Tests
- [ ] Delete `blue.db`, run any command - full rebuild succeeds
- [ ] Manual file creation detected and indexed on next access
- [ ] File deletion soft-deletes DB record
- [ ] Hash mismatch triggers re-index

### Document Lookup Tests
- [ ] `find_document("filesystem-authority")` matches title "Filesystem Authority"
- [ ] `find_document("Filesystem Authority")` matches slug "filesystem-authority"
- [ ] File created on disk, tool lookup finds it without explicit `blue sync`
- [ ] `blue_worktree_create` works with slug after file-only creation

### Numbering Tests (from RFC 0021)
- [ ] `next_number()` returns correct value when DB empty but files exist
- [ ] `next_number()` returns `max(db, fs) + 1`
- [ ] Handles missing directory gracefully

### Plan File Tests (from RFC 0017)
- [ ] Task completion updates `.plan.md`
- [ ] Plan file changes detected via mtime
- [ ] SQLite rebuilds from plan on access

### Sync Tests (from RFC 0018)
- [ ] `blue sync` creates records for unindexed files
- [ ] `blue sync` soft-deletes orphan records
- [ ] `blue sync --dry-run` reports without modifying
- [ ] `blue status` warns when drift detected

## Migration

1. Add `content_hash`, `indexed_at`, `deleted_at` columns to documents table
2. Add `blue.db` to `.gitignore`
3. Run `blue sync` to populate hashes for existing documents
4. Mark RFCs 0017, 0018, 0021 as `superseded-by: 0022`

## What This RFC Does NOT Cover

**RFC 0020 (Source Link Resolution)** remains separate. It addresses how to *render* links in document metadata, not what the source of truth is. RFC 0020 may reference this RFC for path resolution mechanics.

## Superseded RFCs

| RFC | Title | Disposition |
|-----|-------|-------------|
| 0017 | Plan File Authority | Merged - Section 5 |
| 0018 | Document Import/Sync | Merged - Sections 2, 4 |
| 0021 | Filesystem-Aware Numbering | Merged - Section 3 |

These RFCs should be marked `status: superseded` with reference to this document.

## References

- [ADR 0005: Single Source of Truth](../adrs/0005-single-source.md)
- [ADR 0007: Integrity](../adrs/0007-integrity.md)
- [Alignment Dialogue](../dialogues/2026-01-26-rfc-0022-filesystem-authority.dialogue.md)

---

*"The filesystem is truth. The database is cache. Git remembers everything else."*

— Blue
