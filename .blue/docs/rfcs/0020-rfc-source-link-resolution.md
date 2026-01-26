# RFC 0020: RFC Source Link Resolution

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | RFC Source Link Generation |
| **Related** | [RFC 0022: Filesystem Authority](./0022-filesystem-authority.md) |

---

## Summary

RFC metadata's Source Spike field renders as plain text instead of a clickable markdown link. When `blue_rfc_create` is called with `source_spike`, the value is stored and rendered verbatim. Users cannot navigate directly from an RFC to its source spike.

**Current behavior**: `| **Source Spike** | Spike Title |`
**Expected behavior**: `| **Source Spike** | [Spike Title](../spikes/date-title.md) |`

The store already provides `find_document(DocType::Spike, title)` which returns the spike's `file_path`. The fix is to resolve spike titles to markdown links at RFC creation time.

## Test Plan

- [ ] TBD

---

*"Right then. Let's get to it."*

— Blue
