# RFC 0021: Filesystem-Aware Numbering

| | |
|---|---|
| **Status** | Superseded |
| **Superseded By** | [RFC 0022: Filesystem Authority](./0022-filesystem-authority.md) |
| **Date** | 2026-01-26 |
| **Source Spike** | RFC Numbering Collision |
| **Alignment** | 12-expert dialogue, 97% convergence (2 rounds) |

---

## Problem

`next_number()` in `store.rs:2151` only queries SQLite to determine the next document number. When files exist on disk but aren't indexed in the database, numbering collisions occur.

**Observed behavior**: RFC 0018 was assigned when `0018-document-import-sync.md` and `0019-claude-code-task-integration.md` already existed on disk but weren't in the database.

**Root cause**:
```rust
pub fn next_number(&self, doc_type: DocType) -> Result<i32, StoreError> {
    let max: Option<i32> = self.conn.query_row(
        "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
        params![doc_type.as_str()],
        |row| row.get(0),
    )?;
    Ok(max.unwrap_or(0) + 1)
}
```

This violates ADR 0005 (Single Source of Truth): the filesystem is the authoritative source, not the database.

## Proposal

Two-phase fix aligned with RFC 0018 (Document Import/Sync):

### Phase 1: Immediate Safety (This RFC)

Modify `next_number()` to scan both database AND filesystem, taking the maximum:

```rust
pub fn next_number(&self, doc_type: DocType) -> Result<i32, StoreError> {
    // 1. Get max from database (fast path)
    let db_max: Option<i32> = self.conn.query_row(
        "SELECT MAX(number) FROM documents WHERE doc_type = ?1",
        params![doc_type.as_str()],
        |row| row.get(0),
    )?;

    // 2. Scan filesystem for existing numbered files
    let fs_max = self.scan_filesystem_max(doc_type)?;

    // 3. Take max of both - filesystem is truth
    Ok(std::cmp::max(db_max.unwrap_or(0), fs_max) + 1)
}

fn scan_filesystem_max(&self, doc_type: DocType) -> Result<i32, StoreError> {
    let dir = self.docs_path.join(doc_type.plural());
    if !dir.exists() {
        return Ok(0);
    }

    let pattern = Regex::new(r"^(\d{4})-.*\.md$")?;
    let mut max = 0;

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if let Some(caps) = pattern.captures(name) {
                if let Ok(num) = caps[1].parse::<i32>() {
                    max = std::cmp::max(max, num);
                }
            }
        }
    }

    Ok(max)
}
```

### Phase 2: Reconciliation Gate (RFC 0018)

Ensure `blue_sync` reconciliation runs before any numbering operation. This is already specified in RFC 0018 - this RFC just ensures the immediate safety net exists while that work proceeds.

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Scan on every `next_number()` call | Correctness over micro-optimization; directory read is ~1ms |
| Regex pattern for number extraction | Handles all naming conventions (date-prefixed, kebab-case) |
| Return max + 1, not gaps | Predictable numbering; gaps are fine (git remembers deletions) |
| No caching | Filesystem changes between calls; staleness worse than cost |

## ADR Alignment

| ADR | How Honored |
|-----|-------------|
| ADR 0005 (Single Source) | Filesystem is truth; database is derived index |
| ADR 0010 (No Dead Code) | Simple implementation, no speculative features |
| ADR 0011 (Constraint) | Constraint (always scan) enables freedom (no collision bugs) |

## Test Plan

- [ ] Unit test: `next_number()` returns correct value when DB is empty but files exist
- [ ] Unit test: `next_number()` returns max(db, fs) + 1 when both have data
- [ ] Unit test: handles missing directory gracefully (returns 1)
- [ ] Integration test: create RFC, delete from DB, create another - no collision
- [ ] Regression test: the exact scenario that caused this bug

## Implementation

1. Add `scan_filesystem_max()` helper to `Store`
2. Modify `next_number()` to use both sources
3. Add tests
4. Document in CHANGELOG

## References

- [Spike: RFC Numbering Collision](../spikes/2026-01-26-RFC%20Numbering%20Collision.md)
- [RFC 0018: Document Import/Sync](./0018-document-import-sync.md)
- [ADR 0005: Single Source of Truth](../adrs/0005-single-source.md)
- [Alignment Dialogue](../dialogues/2026-01-26-rfc-0021-filesystem-aware-numbering.dialogue.md)

---

*"The filesystem is truth. The database is cache."*

— Blue
