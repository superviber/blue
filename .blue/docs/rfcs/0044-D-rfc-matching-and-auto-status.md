# RFC 0044: RFC Matching and Auto Status

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-01 |
| **Source Spike** | [rfc-sdlc-workflow-gaps](../spikes/2026-02-01T0124Z-rfc-sdlc-workflow-gaps.wip.md) |

---

## Summary

Two gaps in the SDLC workflow:

1. **RFC matching fails for NNNN-slug patterns** - `find_document()` in `store.rs:1789` uses `trim_start_matches('0')` which only works on pure numeric strings. Pattern `0107-worker-job-id` fails because `107-worker-job-id` isn't a valid integer.

2. **RFC status not auto-updated on PR merge** - `handle_merge()` in `pr.rs:416` doesn't update RFC status to "implemented". The link exists (worktrees table has document_id + branch_name) but missing `get_worktree_by_branch()` function and no status update code.

Both fixes are mechanical.

---

## Design

### Fix 1: Extract Leading Digits from NNNN-slug Patterns

**File:** `crates/blue-core/src/store.rs:1788-1798`

**Before:**
```rust
// Try number match
let trimmed = query.trim_start_matches('0');
if let Ok(num) = if trimmed.is_empty() {
    "0".parse()
} else {
    trimmed.parse::<i32>()
} {
    if let Ok(doc) = self.get_document_by_number(doc_type, num) {
        return Ok(doc);
    }
}
```

**After:**
```rust
// Try number match - extract leading digits from NNNN-slug format
let num_str: String = query.chars()
    .take_while(|c| c.is_ascii_digit())
    .collect();
if !num_str.is_empty() {
    let trimmed = num_str.trim_start_matches('0');
    if let Ok(num) = if trimmed.is_empty() {
        "0".parse()
    } else {
        trimmed.parse::<i32>()
    } {
        if let Ok(doc) = self.get_document_by_number(doc_type, num) {
            return Ok(doc);
        }
    }
}
```

This handles:
- `0107` → extracts `0107` → parses as 107
- `0107-worker-job-id` → extracts `0107` → parses as 107
- `worker-job-id` → extracts `` → skips number match, falls to substring

---

### Fix 2: Auto-Update RFC Status on Merge

**Step 2a: Add `get_worktree_by_branch()` to store.rs**

```rust
pub fn get_worktree_by_branch(&self, branch_name: &str) -> Result<Option<Worktree>, StoreError> {
    match self.conn.query_row(
        "SELECT id, document_id, branch_name, worktree_path, created_at
         FROM worktrees WHERE branch_name = ?1",
        params![branch_name],
        |row| {
            Ok(Worktree {
                id: Some(row.get(0)?),
                document_id: row.get(1)?,
                branch_name: row.get(2)?,
                worktree_path: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    ) {
        Ok(wt) => Ok(Some(wt)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(StoreError::Database(e.to_string())),
    }
}
```

**Step 2b: Update `handle_merge()` in pr.rs**

After successful merge (line 416), add:

```rust
Ok(()) => {
    // Auto-update RFC status to implemented
    let branch = get_current_branch(&state.home.root).ok();
    if let Some(ref b) = branch {
        if let Ok(Some(wt)) = state.store.get_worktree_by_branch(b) {
            if let Ok(doc) = state.store.get_document_by_id(wt.document_id) {
                if doc.status == "in-progress" {
                    let _ = state.store.update_document_status(
                        DocType::Rfc,
                        &doc.title,
                        "implemented"
                    );
                }
            }
        }
    }

    Ok(json!({ ... }))
}
```

---

## Test Plan

- [ ] `find_document("0044")` returns RFC 0044
- [ ] `find_document("0044-rfc-matching")` returns RFC 0044
- [ ] `find_document("rfc-matching")` returns RFC 0044
- [ ] Merge PR on RFC-linked branch → RFC status changes to "implemented"
- [ ] Merge PR on non-RFC branch → no error, no status change

---

*"Right then. Let's get to it."*

— Blue
