# RFC 0006: Document Deletion Tools

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Ported From** | coherence-mcp RFC 0050 |
| **Alignment** | 94% (12 experts, 5 tensions resolved) |

---

## Summary

Blue has no way to cleanly remove documents (RFCs, spikes, decisions). Users must manually delete files and hope the database syncs correctly. This creates orphaned records, broken links, and confusion about what documents exist.

## Goals

- Add unified `blue_delete` tool with safety checks
- Add `blue_restore` tool for recovering soft-deleted documents
- Implement soft-delete with 7-day retention before permanent deletion
- Provide dry-run flag to preview deletions
- Block deletion of documents with ADR dependents (ADRs are permanent)
- Check realm sessions before allowing deletion
- Support CLI usage via `blue delete`

## Design

### Unified Delete Tool

#### `blue_delete`

Delete any document type with consistent behavior.

```
Parameters:
  doc_type: string (required) - "rfc" | "spike" | "decision" | "prd"
  title: string (required) - Document title to delete
  force: boolean - Required for non-draft documents
  permanent: boolean - Skip soft-delete, remove immediately
  dry_run: boolean - Preview what would be deleted without acting
```

#### `blue_restore`

Recover a soft-deleted document within the retention period.

```
Parameters:
  doc_type: string (required) - "rfc" | "spike" | "decision" | "prd"
  title: string (required) - Document title to restore
```

### Deletion Flow

```
1. Validate document exists
2. Check status:
   - draft → proceed
   - other → require force=true
3. Check realm sessions:
   - If active sessions exist → block with message listing repos
   - Unless force=true
4. Check dependents:
   - ADR references → BLOCK (no override, ADRs are permanent)
   - Other links → warn but allow
5. If dry_run=true:
   - Return preview of what would be deleted
   - Exit without changes
6. Set status = 'deleting' (prevents concurrent operations)
7. Delete files:
   - Primary .md file
   - Companion files (.plan.md, .dialogue.md)
   - Worktree if exists
8. Update database:
   - If permanent=true: DELETE record (cascades)
   - Else: SET deleted_at = now()
9. Return summary
```

### Two-Phase Delete (Failure Recovery)

If file deletion fails mid-operation:
1. Revert status from 'deleting' to previous value
2. Return error with list of files that couldn't be deleted
3. User can retry or manually clean up

### Soft Delete Schema

```sql
ALTER TABLE documents ADD COLUMN deleted_at TIMESTAMP NULL;

-- Hide soft-deleted from normal queries
CREATE VIEW active_documents AS
SELECT * FROM documents WHERE deleted_at IS NULL;

-- Auto-cleanup after 7 days (run periodically)
DELETE FROM documents
WHERE deleted_at IS NOT NULL
AND deleted_at < datetime('now', '-7 days');
```

### Safety Matrix

| Status | force | Result |
|--------|-------|--------|
| draft | - | Soft delete |
| accepted | no | "Use force=true to delete accepted RFC" |
| accepted | yes | Soft delete |
| in-progress | no | "Active work with worktree. Use force=true" |
| in-progress | yes | Soft delete + remove worktree |
| implemented | no | "Historical record. Use force=true" |
| implemented | yes | Soft delete (if no ADR) |
| any | - | **BLOCKED if ADR exists** |

### ADR Protection

ADRs are permanent architectural records. They are never auto-deleted.

If a document has an ADR referencing it:
```
Cannot delete RFC 'feature-x'.

This RFC has ADR 0005 documenting its architectural decisions.
ADRs are permanent records and cannot be cascade-deleted.

To proceed:
1. Update ADR 0005 to remove the RFC reference, or
2. Mark this RFC as 'superseded' instead of deleting
```

### Realm Session Check

Before deletion, query active sessions:
```sql
SELECT realm, repo FROM active_sessions
WHERE document_id = ? AND ended_at IS NULL;
```

If found:
```
Cannot delete RFC 'feature-x'.

Active realm sessions:
  - myproject/api-service (session started 2h ago)
  - myproject/web-client (session started 1h ago)

End these sessions first with `blue session stop`, or use force=true.
```

### CLI Support

```bash
# Soft delete (recoverable for 7 days)
blue delete rfc my-feature
blue delete spike investigation-x --force

# Preview what would be deleted
blue delete rfc my-feature --dry-run

# Permanent deletion (no recovery)
blue delete rfc old-draft --permanent

# Restore soft-deleted document
blue restore rfc my-feature

# List soft-deleted documents
blue deleted list
```

### Error Messages (User-Friendly)

Instead of terse errors, provide context:

```
# Bad
"Use force=true"

# Good
"Cannot delete RFC 'auth-refactor'.

Status: in-progress
Worktree: /path/to/worktrees/rfc/auth-refactor
Last activity: 3 hours ago

This RFC has active work. Use --force to delete anyway,
which will also remove the worktree."
```

## Non-Goals

- No ADR deletion - ADRs are permanent architectural records
- No bulk delete - delete one at a time for safety
- No cross-realm cascade - each repo manages its own documents

## Test Plan

- [ ] Soft delete hides document from listings
- [ ] Soft-deleted docs appear in `blue deleted list`
- [ ] Soft-deleted docs recoverable with `blue restore`
- [ ] Documents auto-purge after 7 days
- [ ] Permanent delete removes DB record and files
- [ ] Dry-run shows what would be deleted without acting
- [ ] Realm session check blocks deletion appropriately
- [ ] Force flag overrides session check
- [ ] ADR dependency permanently blocks (no override)
- [ ] Partial failure reverts status to original
- [ ] CLI `blue delete` works end-to-end
- [ ] Concurrent deletion attempts handled safely
- [ ] Companion files (.plan.md) are removed
- [ ] Worktree cleanup happens on RFC deletion

## Alignment Dialogue Summary

**12 Expert Perspectives Integrated:**

| Expert | Key Contribution |
|--------|------------------|
| Database Engineer | Two-phase delete, correct ordering |
| Security Analyst | Audit trail via soft-delete |
| UX Designer | Contextual error messages |
| Distributed Systems | Realm session coordination |
| API Designer | Unified `blue_delete` tool |
| Test Engineer | Expanded test scenarios |
| Product Manager | Soft-delete with recovery |
| DevOps Engineer | CLI support, dry-run flag |
| Data Integrity | ADR protection (no cascade) |
| Performance Engineer | (Deferred: DB-stored companion paths) |
| Documentation Writer | Resolved ADR contradiction |
| Chaos Engineer | Partial failure recovery |

**Tensions Resolved:**
1. Deletion order → Two-phase with status lock
2. ADR cascade contradiction → ADRs never auto-deleted
3. No undo capability → Soft-delete with 7-day retention
4. Realm coordination → Session check before delete
5. API inconsistency → Single unified tool

## Implementation Plan

- [x] Add deleted_at column to documents table
- [x] Add schema migration v2 -> v3
- [x] Update Document struct with deleted_at field
- [x] Update all queries to exclude soft-deleted documents
- [x] Add soft_delete_document method to store
- [x] Add restore_document method to store
- [x] Add get_deleted_document method to store
- [x] Add list_deleted_documents method to store
- [x] Add purge_old_deleted_documents method to store
- [x] Add has_adr_dependents method to store
- [x] Create handlers/delete.rs with all handlers
- [x] Register blue_delete, blue_restore, blue_deleted_list, blue_purge_deleted in MCP
- [ ] Add CLI commands (blue delete, blue restore, blue deleted list)
- [ ] Write integration tests for soft-delete flow
- [ ] Update documentation

---

*"Delete boldly. Git remembers. But so does soft-delete, for 7 days."*

— Blue
