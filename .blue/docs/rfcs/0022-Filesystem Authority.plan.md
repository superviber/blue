# Plan: Filesystem Authority

| | |
|---|---|
| **RFC** | Filesystem Authority |
| **Status** | complete |
| **Updated** | 2026-01-26T02:38:21.586730+00:00 |

## Tasks

- [x] Add content_hash and indexed_at columns to documents table (migration)
- [x] Implement hash_content() for staleness detection
- [x] Implement is_stale() with mtime fast path and hash slow path
- [x] Implement scan_and_register() to parse frontmatter and create DB record from file
- [x] Implement find_document() fallback to filesystem scan when DB lookup fails
- [x] Add slug-based document lookup (kebab-case title matching)
- [x] Implement scan_filesystem_max() for filesystem-aware numbering
- [x] Modify next_number() to use max(db, fs) + 1
- [x] Implement reconcile() for blue sync - scan filesystem and register unindexed files
- [x] Implement orphan detection - soft-delete records for missing files
- [x] Add --dry-run flag to blue sync
- [x] Update blue status to detect and warn about index drift
- [x] Add blue.db to .gitignore
- [x] Write core authority tests (delete DB rebuild, manual file creation, hash mismatch)
- [x] Write document lookup tests (slug matching, file-only creation)
- [x] Write numbering tests (empty DB, max of both sources, missing directory)
- [x] Write sync tests (unindexed files, orphan records, dry-run, drift warning)
