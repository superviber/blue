# RFC 0018: Document Import/Sync Mechanism

| | |
|---|---|
| **Status** | Superseded |
| **Superseded By** | [RFC 0022: Filesystem Authority](./0022-filesystem-authority.md) |
| **Date** | 2026-01-25 |
| **Dialogue** | [rfc-document-import-sync](../dialogues/rfc-document-import-sync.dialogue.md) |

---

## Summary

Blue maintains documents in both filesystem (`.blue/docs/*.md`) and database (`blue.db`). When these diverge, Blue reports "not found" for files that visibly exist. This RFC establishes the filesystem as the single source of truth, with the database serving as a rebuildable index/cache.

## Problem

1. **Files invisible to Blue**: Manually created files, copied files, or files after database reset aren't found by `find_document()`
2. **ADR 0005 violation**: Two sources of truth (filesystem and database) inevitably diverge
3. **Git collaboration broken**: Database doesn't survive `git clone`, so collaborators can't see each other's documents
4. **Branch isolation**: Database state persists across branch switches, causing phantom documents

## Architecture

### Authority Model

```
CURRENT (problematic):
  blue_rfc_create → writes file AND database (can diverge)
  find_document() → queries database ONLY (misses files)

PROPOSED:
  Filesystem = SOURCE OF TRUTH (survives git clone)
  Database = DERIVED INDEX (rebuildable, disposable)
  find_document() = checks index, falls back to filesystem scan
```

### Metadata Location

| Location | Contents | Rationale |
|----------|----------|-----------|
| **Frontmatter** | title, number, status, date | Human-readable identity |
| **Content** | Relationships (as links) | Parseable from text |
| **Database Only** | id, file_path, content_hash, indexed_at, computed relationships | Derived/computed |

**Principle**: If the database is deleted, files alone must be sufficient for full rebuild.

### Staleness Detection

Hash-based lazy revalidation:

```rust
fn is_document_stale(doc: &Document, file_path: &Path) -> bool {
    // Fast path: check mtime
    let file_mtime = fs::metadata(file_path).modified();
    if file_mtime <= doc.indexed_at { return false; }

    // Slow path: verify with hash
    let content = fs::read_to_string(file_path)?;
    let current_hash = hash_content(&content);
    current_hash != doc.content_hash
}
```

No file watchers - they're fragile across platforms and introduce race conditions.

### Reconciliation

| Condition | Action |
|-----------|--------|
| File exists, no DB record | Create DB record from file |
| DB record exists, no file | Soft-delete DB record (`deleted_at = now()`) |
| Both exist, hash mismatch | Update DB from file (filesystem wins) |

### User-Facing Commands

```bash
# Explicit reconciliation
blue sync                # Full filesystem scan, reconcile all
blue sync --dry-run      # Report drift without fixing
blue sync rfcs/          # Scope to directory

# Status shows drift
blue status              # Warns if index drift detected

# Normal operations use index (fast)
blue search "feature"    # Queries index
blue rfc get 0042        # Queries index, falls back to filesystem
```

## Implementation

### Phase 1: Add content_hash to Document

```rust
pub struct Document {
    // ... existing fields ...
    pub content_hash: Option<String>,
    pub indexed_at: Option<String>,
}
```

### Phase 2: Implement `find_document` fallback

```rust
pub fn find_document(&self, doc_type: DocType, query: &str) -> Result<Document, StoreError> {
    // Try database first (fast path)
    if let Ok(doc) = self.find_document_in_db(doc_type, query) {
        return Ok(doc);
    }

    // Fall back to filesystem scan
    self.scan_and_register(doc_type, query)
}
```

### Phase 3: Add `blue sync` command

```rust
pub fn reconcile(&self) -> ReconcileResult {
    let mut result = ReconcileResult::default();

    // Scan filesystem
    for file in glob(".blue/docs/**/*.md") {
        if !self.has_record_for(&file) {
            self.register_from_file(&file);
            result.added.push(file);
        }
    }

    // Check for orphan records
    for doc in self.all_documents() {
        if let Some(path) = &doc.file_path {
            if !Path::new(path).exists() {
                self.soft_delete(doc.id);
                result.orphaned.push(doc);
            }
        }
    }

    result
}
```

### Phase 4: Update `blue status` to show drift

```
$ blue status
RFC 0042 in-progress (3/5 tasks)

⚠ Index drift detected:
  + rfcs/0043-new-feature.md (not indexed)
  - rfcs/0037-old-thing.md (file missing)

Run `blue sync` to reconcile.
```

## Implementation Plan

### Phase 1: Schema & Hashing
- [ ] Add `content_hash` and `indexed_at` fields to Document struct in `store.rs`
- [ ] Add migration to create `content_hash` and `indexed_at` columns in documents table
- [ ] Update document creation/update to populate `content_hash` via `hash_content()`

### Phase 2: Fallback Logic
- [ ] Implement `is_document_stale()` with mtime fast path and hash slow path
- [ ] Add `scan_and_register()` to parse frontmatter and create DB record from file
- [ ] Modify `find_document()` to fall back to filesystem scan when DB lookup fails

### Phase 3: Sync Command
- [ ] Create `blue_sync` MCP handler with `ReconcileResult` struct
- [ ] Implement `reconcile()` - scan filesystem, register unindexed files
- [ ] Implement orphan detection - soft-delete records for missing files
- [ ] Add `--dry-run` flag to report drift without fixing
- [ ] Add directory scoping (`blue sync rfcs/`)

### Phase 4: Status Integration
- [ ] Update `blue_status` to detect and warn about index drift
- [ ] Show count of unindexed files and orphan records in status output

## Guardrails

1. **Never auto-fix**: Always report drift, require explicit `blue sync`
2. **Soft delete only**: DB records for missing files get `deleted_at`, never hard-deleted
3. **30-day retention**: Soft-deleted records purged after 30 days via `blue purge`
4. **Frontmatter validation**: Files with malformed frontmatter get indexed with warnings, not rejected

## Test Plan

- [ ] `find_document` returns file that exists but has no DB record
- [ ] `blue sync` creates records for unindexed files
- [ ] `blue sync` soft-deletes records for missing files
- [ ] `blue status` warns when drift detected
- [ ] Database can be deleted and rebuilt from files
- [ ] Frontmatter parse errors don't block indexing
- [ ] Hash-based staleness detection works correctly

## References

- **ADR 0005**: Single Source of Truth - "One truth, one location"
- **ADR 0007**: Integrity - "Hidden state is a crack"
- **RFC 0008**: Status Update File Sync - Already syncs status to files
- **RFC 0017**: Plan File Authority - Companion files as source of truth
- **Dialogue**: 6-expert alignment achieved 97% convergence

---

*"If I can `cat` the file, Blue should know about it."*

— The 🧁 Consensus

