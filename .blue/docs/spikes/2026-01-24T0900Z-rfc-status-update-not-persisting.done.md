# Spike: Rfc Status Update Not Persisting

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-24 |
| **Time Box** | 1 hour |

---

## Question

Why isn't blue_rfc_update_status (and possibly spike/ADR status updates) persisting to the database?

---

## Root Cause

The issue is **bidirectional sync failure** between database and markdown files:

### Problem 1: RFC/ADR status updates don't update markdown files

When `blue_rfc_update_status` or `blue_rfc_complete` are called:
- ✅ Database is updated via `update_document_status()`
- ❌ Markdown file is NOT updated

Compare to `blue_spike_complete` which correctly updates BOTH:
```rust
// spike.rs lines 118-139
state.store.update_document_status(DocType::Spike, title, "complete")?;

// ALSO updates the markdown file:
if spike_path.exists() {
    let content = fs::read_to_string(&spike_path)?;
    let updated = content
        .replace("| **Status** | Complete |", "| **Status** | Complete |");
    fs::write(&spike_path, updated)?;
}
```

### Problem 2: Manual file edits don't update database

When users edit markdown files directly (or Claude edits them):
- ✅ Markdown file is updated
- ❌ Database is NOT updated

### Evidence

| Document | DB Status | File Status |
|----------|-----------|-------------|
| `docs-path-resolution-bug` | `in-progress` | `Completed` |
| `dialogue-to-blue-directory` | `in-progress` | `Complete` |
| `consistent-branch-naming` | `implemented` | `Implemented` |

The first two were edited manually. The third was updated correctly because we used `blue_rfc_complete`.

## Recommended Fix

**Option A: Update all status change handlers to also update markdown files**
- Add markdown file update logic to `blue_rfc_update_status`
- Add markdown file update logic to `blue_rfc_complete`
- Add markdown file update logic to `blue_adr_*` handlers

**Option B: Single source of truth**
- Treat database as authoritative
- Generate markdown on-the-fly from database when needed
- More fundamental change but eliminates sync issues

**Recommendation:** Option A for now - it's simpler and matches the existing pattern in `blue_spike_complete`.

---

*"Two sources of truth means zero sources of truth."*

— Blue
