# RFC 0030: ISO 8601 Document Filename Timestamps

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | ISO 8601 Timestamp Prefix for Blue Document Filenames |
| **Dialogue** | iso-8601-document-filename-timestamps-rfc-design (Converged, 3 rounds) |

---

## Summary

Blue documents with date-prefixed filenames (spikes, dialogues, decisions, postmortems, audits) use `YYYY-MM-DD` format. On a productive day this creates 15+ files with identical prefixes and no temporal ordering. Adopt filename-safe ISO 8601 hybrid timestamps (`YYYY-MM-DDTHHMMZ`) to provide creation-order, uniqueness, and timezone consistency across all date-prefixed document types.

## Problem

Current filename format: `2026-01-26-native-kanban-apps-for-blue.md`

On 2026-01-26, the spikes directory accumulated 15 files all starting with `2026-01-26-`. There is no way to determine:
- What order they were created
- Which came from the same investigation session
- Whether timestamps in the file content match the filename

Additionally, the 5 affected handlers use **mixed timezones** (3 use UTC, 2 use Local), which means the same wall-clock moment produces different date prefixes depending on document type.

## Design

### New Filename Format

```
YYYY-MM-DDTHHMMZ-slug.md
```

ISO 8601 filename-safe hybrid notation: extended date (`YYYY-MM-DD`) with basic time (`HHMM`), `T` separator, and `Z` suffix for UTC. Colons are omitted because they are illegal in filenames on macOS and Windows. This hybrid is the cross-platform standard used by AWS S3 keys, Docker image tags, and RFC 3339 filename recommendations.

**Examples:**
```
Before: 2026-01-26-native-kanban-apps-for-blue.md
After:  2026-01-26T0856Z-native-kanban-apps-for-blue.md

Before: 2026-01-26-thin-plugin-fat-binary.dialogue.md
After:  2026-01-26T0912Z-thin-plugin-fat-binary.dialogue.md
```

### Affected Document Types

| Document Type | Handler File | Current TZ | Change |
|---|---|---|---|
| Spike | `spike.rs:33` | UTC | Format `%Y-%m-%dT%H%MZ` |
| Dialogue | `dialogue.rs:348` | Local | Switch to UTC + new format |
| Decision | `decision.rs:42` | UTC | New format |
| Postmortem | `postmortem.rs:83` | Local | Switch to UTC + new format |
| Audit | `audit_doc.rs:37` | UTC | New format |

**Not affected:** RFCs, ADRs, PRDs, Runbooks (these use numbered prefixes like `0030-slug.md`, not dates).

### Code Changes

#### 1. Shared timestamp helper (blue-core `documents.rs`)

Replace the existing `today()` helper:

```rust
/// Get current UTC timestamp in ISO 8601 filename-safe format
fn utc_timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H%MZ").to_string()
}
```

#### 2. Each handler's filename generation

```rust
// Before (spike.rs)
let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
let filename = format!("spikes/{}-{}.md", date, title_to_slug(title));

// After
let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H%MZ").to_string();
let filename = format!("spikes/{}-{}.md", timestamp, title_to_slug(title));
```

Same pattern for dialogue, decision, postmortem, audit.

**Note:** The audit handler has a pre-existing bug (raw title instead of `title_to_slug()`). This is a separate fix and should be landed independently before or alongside this RFC.

### Backwards Compatibility

**No migration needed.** The spike investigation confirmed:

1. **No code parses dates from filenames.** The only filename regex (`store.rs:2232`) extracts RFC/ADR *numbers* (`^\d{4}-`), not dates. Date-prefixed files are never parsed by their prefix.
2. **Existing files keep their names.** Old `2026-01-26-slug.md` files continue to work. New files get `2026-01-26T0856Z-slug.md`.
3. **Document lookups use the SQLite store**, not filename patterns. The `find_document()` function matches by title, not filename prefix.

### Timezone Standardization

All 5 handlers switch to `chrono::Utc::now()`. This means:
- Filenames always reflect UTC, matching the `Z` suffix
- A developer in UTC-5 creating a spike at 11pm local time gets `2026-01-27T0400Z` (next day UTC), which is correct -- the timestamp is the machine-truth moment of creation
- The `Date` field inside the markdown body can remain human-friendly (`2026-01-26`) or also switch to ISO 8601 -- either way, the filename is the authoritative timestamp

## Test Plan

- [ ] Unit test: `utc_timestamp()` produces format matching `^\d{4}-\d{2}-\d{2}T\d{4}Z$`
- [ ] Integration: Create one of each affected document type, verify filename matches new format
- [ ] Integration: Verify existing `YYYY-MM-DD-slug.md` files still load and are findable by title
- [ ] Integration: Verify `scan_filesystem_max` regex still works (only applies to numbered docs, but confirm no regression)

## Future Work

- **Handler overwrite protection:** Document creation handlers (`spike.rs`, `dialogue.rs`, `postmortem.rs`, `audit_doc.rs`) call `fs::write` without checking file existence. If two documents with identical slugs are created in the same UTC minute, the second silently overwrites the first. A follow-up change should add `create_new(true)` semantics or existence checks to all 5 handlers. (`decision.rs` already has this check at line 51.)
- **Audit slug bug:** `audit_doc.rs:37` uses raw title instead of `title_to_slug()` for filenames. Fix independently.

---

*"Right then. Let's get to it."*

-- Blue
