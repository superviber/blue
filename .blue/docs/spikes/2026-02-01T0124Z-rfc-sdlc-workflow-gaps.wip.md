# Spike: RFC SDLC Workflow Gaps

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-02-01 |
| **Time Box** | 1 hour |

---

## Question

Why does RFC matching fail for NNNN-prefixed patterns, and why doesn't RFC status auto-update on PR merge?

---

## Findings

### Issue 1: RFC Matching Fails for `NNNN-slug` Patterns

**Location:** `crates/blue-core/src/store.rs:1749-1831`

The `find_document()` function tries matches in this order:

1. **Exact title match** (line 1751)
2. **Slug-to-title** (lines 1755-1786): `worker-job-id` → `worker job id`
3. **Number match** (lines 1788-1798): Parse as integer
4. **Substring match** (lines 1800-1824): `LIKE '%query%'`

**Root cause at line 1789:**

```rust
let trimmed = query.trim_start_matches('0');
if let Ok(num) = if trimmed.is_empty() {
    "0".parse()
} else {
    trimmed.parse::<i32>()
} { ... }
```

This only works for **pure numeric strings**. Given `0107-worker-job-id-integration`:
- `trim_start_matches('0')` → `"107-worker-job-id-integration"`
- `parse::<i32>()` → **fails** (not a number)
- Falls through to substring match
- `%0107-worker-job-id-integration%` matches nothing

**Why each pattern failed:**

| Pattern | Why Failed |
|---------|-----------|
| `0107` | Parsed as 107, but no RFC #107 exists |
| `0107-worker-job-id-integration` | Not a pure number, substring finds nothing |
| `worker-job-id-integration` | Substring match succeeds ✓ |

**Fix:** Extract leading digits with regex before number parse:

```rust
// Try number match - extract leading digits from NNNN-slug format
let num_str = query.chars()
    .take_while(|c| c.is_ascii_digit())
    .collect::<String>();
let trimmed = num_str.trim_start_matches('0');
```

---

### Issue 2: RFC Status Not Auto-Updating on Merge

**Location:** `crates/blue-mcp/src/handlers/pr.rs:341-441`

The `handle_merge()` function merges the PR but **never updates RFC status**.

**Existing automation:**

| Handler | Status Update |
|---------|--------------|
| `worktree.rs:218-221` | accepted → in-progress ✓ |
| `pr.rs:341-441` | **None** ❌ |

**Why it's missing - the data exists but isn't connected:**

1. **Worktrees table has the link:**
   ```sql
   CREATE TABLE worktrees (
       document_id INTEGER NOT NULL,  -- Links to RFC
       branch_name TEXT NOT NULL,     -- The branch
       ...
   )
   ```

2. **PR handler can get current branch:**
   ```rust
   fn get_current_branch(repo_path: &Path) -> Result<String, String>
   ```

3. **But no way to look up worktree by branch:**
   - `get_worktree(document_id)` - by RFC id only
   - `list_worktrees()` - all worktrees
   - **Missing:** `get_worktree_by_branch(branch_name)`

**Fix requires two changes:**

1. Add `get_worktree_by_branch()` to `store.rs`:
   ```rust
   pub fn get_worktree_by_branch(&self, branch_name: &str) -> Result<Option<Worktree>, StoreError>
   ```

2. Update `handle_merge()` in `pr.rs` to:
   - Get current branch
   - Look up worktree by branch
   - Get document (RFC) from worktree.document_id
   - Update status to "implemented"

---

## Summary

| Issue | Root Cause | Fix Location |
|-------|-----------|--------------|
| RFC matching | Number extraction only works on pure digits | `store.rs:1789` |
| Auto-status | No worktree→RFC lookup on merge | `pr.rs:416` + `store.rs` |

Both fixes are mechanical. Create RFC to track implementation.
