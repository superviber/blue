# RFC 0001: Dialogue SQLite Metadata

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Source Spike** | sqlite-storage-expansion |

---

## Summary

Dialogue files (.dialogue.md) are not indexed in SQLite. Can't query them, link them to RFCs, or track relationships. Need to add DocType::Dialogue and store metadata while keeping content in markdown.

## Background

Dialogues are transcripts of conversations - different from RFCs/spikes which are living documents with status transitions.

Current state:
- Dialogues exist as `.dialogue.md` files in `docs/dialogues/`
- No SQLite tracking
- No way to search or link them

## Proposal

### 1. Add DocType::Dialogue

```rust
pub enum DocType {
    Rfc,
    Spike,
    Adr,
    Decision,
    Prd,
    Postmortem,
    Runbook,
    Dialogue,  // NEW
}
```

### 2. Dialogue Metadata (SQLite)

Store in `documents` table:
- `doc_type`: "dialogue"
- `title`: Dialogue title
- `status`: "complete" (dialogues don't have status transitions)
- `file_path`: Path to .dialogue.md file

Store in `metadata` table:
- `date`: When dialogue occurred
- `participants`: Who was involved (e.g., "Claude, Eric")
- `linked_rfc`: RFC this dialogue relates to (optional)
- `topic`: Short description of what was discussed

### 3. New Tool: `blue_dialogue_create`

```
blue_dialogue_create title="realm-design-session" linked_rfc="cross-repo-realms"
```

Creates:
- Entry in documents table
- Metadata entries
- Skeleton .dialogue.md file

### 4. Dialogue File Format

```markdown
# Dialogue: Realm Design Session

| | |
|---|---|
| **Date** | 2026-01-24 |
| **Participants** | Claude, Eric |
| **Topic** | Designing cross-repo coordination |
| **Linked RFC** | [cross-repo-realms](../rfcs/0001-cross-repo-realms.md) |

---

## Context

[Why this dialogue happened]

## Key Decisions

- Decision 1
- Decision 2

## Transcript

[Full conversation or summary]

---

*Extracted by Blue*
```

### 5. Keep Content in Markdown

Unlike other doc types, dialogue content stays primarily in markdown:
- Full transcripts can be large
- Human-readable format preferred
- Git diff friendly

SQLite stores metadata only for:
- Fast searching
- Relationship tracking
- Listing/filtering

### 6. New Tool: `blue_dialogue_get`

```
blue_dialogue_get title="realm-design-session"
```

Returns dialogue metadata and file path.

### 7. New Tool: `blue_dialogue_list`

```
blue_dialogue_list linked_rfc="cross-repo-realms"
```

Returns all dialogues, optionally filtered by linked RFC.

### 8. Integration with `blue_extract_dialogue`

Existing `blue_extract_dialogue` extracts text from Claude JSONL. Extend to:

```
blue_extract_dialogue task_id="abc123" save_as="realm-design-session" linked_rfc="cross-repo-realms"
```

- Extract dialogue from JSONL
- Create .dialogue.md file
- Register in SQLite with metadata

### 9. Migration of Existing Dialogues

On first run, scan `docs/dialogues/` for `.dialogue.md` files:
- Parse frontmatter for metadata
- Register in documents table
- Preserve file locations

## Security Note

Dialogues may contain sensitive information discussed during development. Before committing:
- Review for credentials, API keys, or secrets
- Use `[REDACTED]` for sensitive values
- Consider if full transcript is needed vs summary

## Example Transcript Section

```markdown
## Transcript

**Eric**: How should we handle authentication for the API?

**Claude**: I'd recommend JWT tokens with short expiry. Here's why:
1. Stateless - no session storage needed
2. Can include claims for authorization
3. Easy to invalidate by changing signing key

**Eric**: What about refresh tokens?

**Claude**: Store refresh tokens in httpOnly cookies. When access token expires,
use refresh endpoint to get new pair. This balances security with UX.

**Decision**: Use JWT + refresh token pattern.
```

## Implementation

1. Add `DocType::Dialogue` to enum
2. Create `blue_dialogue_create` handler
3. Create `blue_dialogue_list` handler
4. Update `blue_search` to include dialogues
5. Add dialogue markdown generation

## Test Plan

- [ ] Create dialogue with metadata
- [ ] Link dialogue to RFC
- [ ] Dialogue without linked RFC works
- [ ] Search finds dialogues by title/topic
- [ ] List dialogues by RFC works
- [ ] List all dialogues works
- [ ] Get specific dialogue returns metadata
- [ ] Dialogue content stays in markdown
- [ ] Metadata stored in SQLite
- [ ] Existing dialogues migrated on first run
- [ ] Extract dialogue from JSONL creates proper entry

---

*"Right then. Let's get to it."*

— Blue
