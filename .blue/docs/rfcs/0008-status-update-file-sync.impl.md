# RFC 0008: Status Update File Sync

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Source Spike** | rfc-status-update-not-persisting |

---

## Summary

Status update handlers only update the database but not the markdown files, causing a sync mismatch between what users see in files and what's in the database.

## Problem

When status is updated via MCP tools:
- `blue_rfc_update_status` → Updates DB only ❌
- `blue_rfc_complete` → Updates DB only ❌
- `blue_spike_complete` → Updates both DB and file ✅

This causes confusion when users check markdown files expecting to see updated status.

## Proposal

Add a helper function to update markdown file status, then call it from all status-changing handlers.

### Implementation

1. Create `update_markdown_status(file_path, old_status, new_status)` helper in `blue-core`
2. Update `handle_rfc_update_status` in `server.rs` to call helper after DB update
3. Update `handle_rfc_complete` in `rfc.rs` to call helper after DB update
4. Consider adding to ADR handlers if they have status changes

### Helper Function

```rust
pub fn update_markdown_status(
    file_path: &Path,
    new_status: &str
) -> Result<(), std::io::Error> {
    let content = fs::read_to_string(file_path)?;

    // Match common status formats
    let patterns = [
        (r"\| \*\*Status\*\* \| [^|]+ \|", format!("| **Status** | {} |", new_status)),
        (r"\*\*Status:\*\* \w+", format!("**Status:** {}", new_status)),
    ];

    let mut updated = content;
    for (pattern, replacement) in patterns {
        updated = regex::Regex::new(pattern)
            .unwrap()
            .replace(&updated, replacement.as_str())
            .to_string();
    }

    fs::write(file_path, updated)
}
```

## Test Plan

- [x] `blue_rfc_update_status` updates both DB and markdown file
- [x] `blue_rfc_complete` updates both DB and markdown file
- [x] Status patterns are correctly replaced (table format, inline format)
- [x] No changes to files without status fields

## Implementation Plan

- [x] Add `update_markdown_status` helper to blue-core (`documents.rs:391`)
- [x] Update `handle_rfc_update_status` in server.rs
- [x] Update `handle_complete` in rfc.rs
- [x] Add unit tests for status replacement
- [x] Refactor `blue_spike_complete` to use shared helper

---

*"Right then. Let's get to it."*

— Blue
