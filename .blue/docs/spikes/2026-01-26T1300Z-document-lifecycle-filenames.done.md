# Spike: Document Lifecycle Filenames

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 30 minutes |

---

## Question

How should Blue encode document lifecycle status in filenames so that browsing a directory reveals document state at a glance? What format works across all 9 document types, preserves lexicographic sorting, remains filesystem-safe, and integrates with the ISO 8601 timestamp work from RFC 0030?

---

## Investigation

### Current State

9 document types. Status stored in SQLite + markdown frontmatter. **Zero status information in filenames.**

| Type | Filename Pattern | Statuses | Browse Problem |
|---|---|---|---|
| RFC | `0030-slug.md` | draft, accepted, in-progress, implemented, superseded | Can't tell if draft or shipped |
| Spike | `2026-01-26-slug.md` | in-progress, complete (+outcome) | Can't tell if resolved |
| ADR | `0004-slug.md` | accepted, in-progress, implemented | Can't tell if active |
| Decision | `2026-01-26-slug.md` | recorded | Always same (no problem) |
| PRD | `0001-slug.md` | draft, approved, implemented | Can't tell if approved |
| Postmortem | `2026-01-26-slug.md` | open, closed | Can't tell if resolved |
| Runbook | `slug.md` | active, archived | Can't tell if current |
| Dialogue | `2026-01-26-slug.dialogue.md` | draft, published | Can't tell if final |
| Audit | `2026-01-26-slug.md` | in-progress, complete | Can't tell if done |

### Key Constraint: No Code Parses Filenames

From RFC 0030 investigation: `store.rs:2232` regex only extracts numbered doc prefixes. Document lookups use SQLite by title. **Renaming files does not break internal lookups** — only `file_path` in the documents table needs updating.

### Design Options

#### Option A: Status suffix before `.md`

```
0030-iso-8601-timestamps.draft.md     → 0030-iso-8601-timestamps.impl.md
2026-01-26T0856Z-kanban-apps.wip.md   → 2026-01-26T0856Z-kanban-apps.done.md
```

**Pros:** Clear at a glance, sorts by number/date first, status is visual suffix
**Cons:** Renaming on status change breaks git history, external links

#### Option B: Status prefix after number/date

```
0030-DRAFT-iso-8601-timestamps.md     → 0030-IMPL-iso-8601-timestamps.md
2026-01-26T0856Z-WIP-kanban-apps.md   → 2026-01-26T0856Z-DONE-kanban-apps.md
```

**Pros:** Status visible early in filename
**Cons:** Disrupts slug, ALL_CAPS is noisy, breaks cross-references

#### Option C: Status subdirectories

```
rfcs/draft/0030-iso-8601-timestamps.md    → rfcs/implemented/0030-iso-8601-timestamps.md
spikes/active/2026-01-26T0856Z-kanban.md  → spikes/complete/2026-01-26T0856Z-kanban.md
```

**Pros:** Clean filenames, easy browsing by status, familiar (like git branches)
**Cons:** Moving files between directories, deeper path nesting, complex for tools

#### Option D: Status dot-notation (minimal)

```
0030-iso-8601-timestamps.d.md  → 0030-iso-8601-timestamps.i.md
```

**Pros:** Minimal visual noise
**Cons:** Single letter is cryptic, easy to miss

#### Option E: Combination — timestamp + status suffix

```
2026-01-26T0856Z-kanban-apps.spike.wip.md
2026-01-26T0856Z-kanban-apps.spike.done-rfc.md
0030-iso-8601-timestamps.rfc.draft.md
0030-iso-8601-timestamps.rfc.impl.md
```

**Pros:** Self-documenting (type + status), works across all doc types
**Cons:** Long filenames, multiple dots

### The Rename Problem

All status-in-filename approaches require renaming files when status changes. This has consequences:

1. **Git history**: `git log --follow` tracks renames, but `git blame` shows only current name
2. **Cross-references**: Markdown links like `[RFC 0030](../rfcs/0030-slug.md)` break on rename
3. **External bookmarks**: Browser bookmarks, shell aliases break
4. **SQLite file_path**: Must update `documents.file_path` on every rename

**Mitigation strategies:**
- Update `file_path` in store on every status change (already touches store + markdown)
- Cross-references use title-based lookups, not filename — most survive
- `git mv` preserves history tracking
- Accept that external bookmarks break (they already break on file deletion)

### Spike-Specific Requirements

The user specifically wants to see spike outcomes from filenames:

| Outcome | Meaning | Proposed Suffix |
|---|---|---|
| in-progress | Active investigation | `.wip` or no suffix |
| complete: no-action | Dead end | `.done` |
| complete: decision-made | Resolved with decision | `.done` |
| complete: recommends-implementation | Needs RFC | `.rfc` or `.done-rfc` |

### RFC-Specific Requirements

| Status | Proposed Suffix |
|---|---|
| draft | `.draft` or no suffix |
| accepted | `.accepted` |
| in-progress | `.wip` |
| implemented | `.impl` |
| superseded | `.super` |

### Status Abbreviation Vocabulary

A consistent set of short status tags across all document types:

| Tag | Meaning | Used By |
|---|---|---|
| (none) | Active/in-progress/draft (default) | All types |
| `.done` | Complete/closed/recorded | Spike, Audit, Postmortem |
| `.impl` | Implemented | RFC, ADR, PRD |
| `.super` | Superseded | RFC |
| `.archived` | Archived/inactive | Runbook |

## Findings

| Question | Answer |
|---------|--------|
| Can status live in filenames? | Yes — no internal code parses filenames for status |
| Best approach? | Option A (status suffix) or Option C (subdirectories) — needs alignment dialogue |
| Does this integrate with RFC 0030? | Yes — timestamp + status suffix: `2026-01-26T0856Z-slug.done.md` |
| What about the rename problem? | Manageable — `git mv` + store update + title-based lookups survive |
| Biggest risk? | Cross-reference breakage in markdown files |

## Outcome

Recommends implementation. This should supersede RFC 0030 by incorporating both ISO 8601 timestamps AND status-in-filename into a unified "Document Lifecycle Filenames" RFC.
