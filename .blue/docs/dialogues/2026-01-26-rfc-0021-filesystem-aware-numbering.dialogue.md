# Alignment Dialogue: RFC 0021 Filesystem-Aware Numbering

**RFC**: [0021-filesystem-aware-numbering](../rfcs/0021-filesystem-aware-numbering.md)
**Experts**: 12
**Rounds**: 2
**Final Convergence**: 97%

---

## Problem Statement

`next_number()` in `store.rs:2151` only queries SQLite. When files exist on disk but aren't indexed, numbering collisions occur.

---

## Round 1: Initial Positions

| Expert | Position | Confidence |
|--------|----------|------------|
| Database Architect | DB as truth, sync filesystem → DB | 85% |
| Filesystem Engineer | Filesystem is truth, always scan | 90% |
| Distributed Systems | Two-phase: immediate fix + reconciliation | 80% |
| DX Advocate | Whatever prevents user confusion | 75% |
| Rustacean | Type-safe solution with Result handling | 85% |
| ADR Guardian | Must honor ADR 0005 (Single Source) | 90% |
| Performance Engineer | Concerned about scan overhead | 70% |
| Git Workflow | Filesystem aligns with git-native approach | 85% |
| Minimalist | Simplest fix that works | 80% |
| Data Migration | Need migration path for existing data | 75% |
| Testing/QA | Need regression test for exact scenario | 85% |
| Devil's Advocate | What if filesystem is corrupted? | 65% |

**Round 1 Convergence**: 86%

### Key Tensions

1. **Performance vs Correctness**: Scanning filesystem on every call vs caching
2. **Authority**: Database-first vs Filesystem-first
3. **Complexity**: Simple scan vs full reconciliation system

---

## Round 2: Convergence

After reviewing RFC 0018 (Document Import/Sync) which establishes filesystem as source of truth:

| Expert | Final Position | Confidence |
|--------|----------------|------------|
| Database Architect | ALIGN - filesystem is truth per RFC 0018 | 97% |
| Filesystem Engineer | ALIGN - scan both, take max | 78% |
| Distributed Systems | ALIGN - two-phase approach | 85% |
| DX Advocate | ALIGN - prevents user confusion | 88% |
| Rustacean | ALIGN - clean Result handling | 88% |
| ADR Guardian | ALIGN - honors ADR 0005 | 88% |
| Performance Engineer | ALIGN - 1ms scan acceptable | 78% |
| Git Workflow | ALIGN - git-native approach | 82% |
| Minimalist | ALIGN - simple enough | 88% |
| Data Migration | ALIGN - RFC 0018 handles migration | 88% |
| Testing/QA | ALIGN - clear test plan | 85% |
| Devil's Advocate | ALIGN - corruption handled by returning max | 78% |

**Round 2 Convergence**: 97% (12/12 ALIGN, avg confidence 85.25%)

---

## Converged Architecture

```rust
pub fn next_number(&self, doc_type: DocType) -> Result<i32, StoreError> {
    // 1. Get max from database (fast path)
    let db_max: Option<i32> = self.conn.query_row(...)?;

    // 2. Scan filesystem for existing numbered files
    let fs_max = self.scan_filesystem_max(doc_type)?;

    // 3. Take max of both - filesystem is truth
    Ok(std::cmp::max(db_max.unwrap_or(0), fs_max) + 1)
}
```

### Key Consensus Points

1. **Filesystem is truth** (ADR 0005, RFC 0018)
2. **Scan on every call** - correctness over micro-optimization
3. **No caching** - staleness worse than ~1ms cost
4. **Two-phase approach** - immediate fix + RFC 0018 reconciliation
5. **Simple regex pattern** - handles all naming conventions

---

*Convergence achieved. RFC 0021 drafted.*
