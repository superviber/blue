# RFC 0010: Realm Semantic Index

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-24 |
| **Source Spike** | Realm Semantic Index |
| **Dialogue** | [realm-semantic-index.dialogue.md](../dialogues/realm-semantic-index.dialogue.md) |
| **Alignment** | 97% |

---

## Summary

An AI-maintained semantic index for files within a realm. Each file gets a summary and symbol-level descriptions with line references. Enables semantic search for impact analysis: "what depends on this file?" and "what's the blast radius of this change?"

## Problem

When working across repos in a realm:
- No quick way to know what a file does without reading it
- No way to find files related to a concept ("authentication", "S3 access")
- No impact analysis before making changes
- Existing search is keyword-only, misses semantic matches

## Proposal

### Index Structure

Each indexed file contains:

```yaml
file: src/realm/domain.rs
last_indexed: 2026-01-24T10:30:00Z
file_hash: abc123

summary: "Domain definitions for cross-repo coordination"

relationships: |
  Core types used by service.rs for realm state management.
  Loaded/saved by repo.rs for persistence.
  Referenced by daemon/client.rs for cross-repo messaging.

symbols:
  - name: Domain
    kind: struct
    lines: [13, 73]
    description: "Coordination context between repos with name, members, timestamps"

  - name: Binding
    kind: struct
    lines: [76, 143]
    description: "Declares repo exports and imports within a domain"

  - name: ImportStatus
    kind: enum
    lines: [259, 274]
    description: "Binding status: Pending, Current, Outdated, Broken"
```

### Storage: SQLite + FTS5

Use existing blue.db with full-text search:

```sql
-- File-level index
CREATE TABLE file_index (
    id INTEGER PRIMARY KEY,
    realm TEXT NOT NULL,
    repo TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    summary TEXT,
    relationships TEXT,  -- AI-generated relationship descriptions
    indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    prompt_version INTEGER DEFAULT 1,  -- Invalidate on prompt changes
    embedding BLOB,  -- Optional, for future vector search
    UNIQUE(realm, repo, file_path)
);

-- Symbol-level index
CREATE TABLE symbol_index (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES file_index(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    start_line INTEGER,
    end_line INTEGER,
    description TEXT
);

-- FTS5 for search
CREATE VIRTUAL TABLE file_search USING fts5(
    file_path,
    summary,
    relationships,
    content=file_index,
    content_rowid=id
);

CREATE VIRTUAL TABLE symbol_search USING fts5(
    name,
    description,
    content=symbol_index,
    content_rowid=id
);
```

### Update Triggers: Git-Driven

**Primary: Pre-commit hook on diff**

```bash
# .git/hooks/pre-commit (installed by blue index --install-hook)
#!/bin/sh
blue index --diff
```

The hook runs `blue index --diff` which:
1. Gets staged files from `git diff --cached --name-only`
2. Indexes only those files
3. Commits include fresh index entries

**Bootstrap: Full index from scratch**

```bash
# First time setup - index everything
blue index --all

# Or index specific directory
blue index --all src/
```

**On-demand: Single file or refresh**

```bash
# Re-index specific file
blue index --file src/domain.rs

# Refresh stale entries (re-index files where hash changed)
blue index --refresh
```

**MCP inline**: When called from Claude, can index files during conversation.

### Staleness Detection

```
blue index status

Index status:
  Total files: 147
  Indexed: 142 (96%)
  Stale: 3 (hash mismatch)
  Unindexed: 2 (new files)

  Stale:
    - src/realm/domain.rs
    - src/realm/service.rs

  Unindexed:
    - src/new_feature.rs
    - tests/new_test.rs
```

### Relationships: AI-Generated at Index Time

When indexing a file, AI generates a concise `relationships` description alongside the summary:

```yaml
file: src/realm/service.rs
summary: "RealmService coordinates cross-repo state and notifications"

relationships: |
  Uses Domain and Binding from domain.rs for state representation.
  Calls RepoConfig from config.rs for realm settings.
  Provides notifications consumed by daemon/server.rs.
  Tested by tests/realm_service_test.rs.

symbols:
  - name: RealmService
    kind: struct
    lines: [15, 89]
    description: "Main service coordinating realm operations"
```

The `relationships` field is a natural language description — searchable via FTS5:

```
Query: "what uses Domain"
→ Matches service.rs: "Uses Domain and Binding from domain.rs..."

Query: "what provides notifications"
→ Matches service.rs: "Provides notifications consumed by..."
```

AI does the relationship analysis once during indexing. Search is just text matching over stored descriptions. Fast and deterministic.

### AI Model: Qwen2.5:3b via Ollama

**Recommended**: `qwen2.5:3b` — optimal balance of speed and quality for code indexing.

| Model | Speed (M2) | Quality | Verdict |
|-------|------------|---------|---------|
| qwen2.5:1.5b | ~150 tok/s | Basic | Too shallow for code analysis |
| **qwen2.5:3b** | ~100 tok/s | Very Good | **Sweet spot** — fast, accurate |
| qwen2.5:7b | ~50 tok/s | Excellent | Too slow for batch indexing |

At 3b, a 500-token file indexes in ~5 seconds. A 5-file commit takes ~25 seconds — acceptable for pre-commit hook.

```
Model priority:
1. Ollama qwen2.5:3b (default) - fast, local, private
2. --model flag - explicit override (e.g., qwen2.5:7b for quality)
3. Inline Claude - when called from MCP, use active model
```

Privacy: code stays local by default. API requires explicit opt-in.

### Large File Handling

Files under 1000 lines: index whole file.
Files over 1000 lines: summarize with warning "Large file, partial index."

No chunking for MVP. Note the limitation, move on.

### Indexing Prompt

Versioned prompt for structured extraction:

```
Analyze this source file and provide:
1. A one-sentence summary of what this file does
2. A paragraph describing relationships to other files (imports, exports, dependencies)
3. A list of key symbols (functions, classes, structs, enums) with:
   - name
   - kind (function/class/struct/enum/const)
   - start and end line numbers
   - one-sentence description

Output as YAML.
```

Store `prompt_version` in file_index. When prompt changes, all entries are stale.

### CLI Commands

```bash
# Bootstrap: index everything from scratch
blue index --all

# Install git pre-commit hook
blue index --install-hook

# Index staged files (called by hook)
blue index --diff

# Index single file
blue index --file src/domain.rs

# Refresh stale entries
blue index --refresh

# Check index freshness
blue index status

# Search the index
blue search "S3 permissions"
blue search --symbols "validate"

# Impact analysis
blue impact src/domain.rs
```

### MCP Tools

| Tool | Purpose |
|------|---------|
| `blue_index_realm` | Index all files in current realm |
| `blue_index_file` | Index a single file |
| `blue_index_status` | Show index freshness |
| `blue_index_search` | Search across indexed files |
| `blue_index_impact` | Show files depending on target |

## Non-Goals

- Cross-realm search (scope to single realm for MVP)
- Automatic relationship storage (query-time only)
- Required embeddings (FTS5 is sufficient, embeddings are optional)
- Language-specific parsing (AI inference works across languages)

## Test Plan

- [ ] Schema created in blue.db on first index
- [ ] `blue index --all` indexes all files in realm, extracts symbols
- [ ] `blue index --diff` indexes only staged files
- [ ] `blue index --file` indexes single file, updates existing entry
- [ ] `blue index --install-hook` creates valid pre-commit hook
- [ ] `blue index --refresh` re-indexes stale entries only
- [ ] `blue index status` shows staleness accurately
- [ ] `blue search` returns relevant files ranked by match quality
- [ ] `blue impact` shows files with symbols referencing target
- [ ] Staleness detection works (file hash comparison)
- [ ] Prompt version tracked; old versions marked stale
- [ ] Qwen2.5:3b produces valid YAML output
- [ ] Large files (>1000 lines) get partial index warning
- [ ] Ollama integration works for local indexing
- [ ] `--model` flag allows override to different model
- [ ] MCP tools available and functional
- [ ] FTS5 search handles partial matches
- [ ] Pre-commit hook runs without blocking commit on failure
- [ ] Relationships field searchable via FTS5

## Implementation Plan

- [x] Add schema to blue.db (file_index, symbol_index, FTS5 tables)
- [x] Create versioned indexing prompt for structured YAML extraction
- [x] Implement Ollama integration with qwen2.5:3b default
- [x] Implement `blue index --all` for bootstrap
- [x] Implement `blue index --diff` for staged files
- [x] Implement `blue index --file` for single-file updates
- [x] Implement `blue index --install-hook` for git hook setup
- [x] Implement `blue index --refresh` for stale entry updates
- [x] Implement `blue index status` for freshness reporting
- [x] Add large file handling (>1000 lines warning)
- [x] Implement `blue search` with FTS5 backend
- [x] Implement `blue impact` for dependency queries
- [x] Add MCP tools (5 tools)
- [x] Add `--model` flag for model override
- [ ] Optional: embedding column support

## Open Questions (Resolved)

| Question | Resolution | Alignment |
|----------|------------|-----------|
| Storage backend | SQLite + FTS5, optional embedding column | 92% |
| Update triggers | Git pre-commit hook on diff, `--all` for bootstrap | 98% |
| Relationships | AI-generated descriptions stored at index time | 96% |
| AI model | Qwen2.5:3b via Ollama, `--model` for override | 94% |
| Granularity | Symbol-level with line numbers | 92% |
| Large files | Whole-file <1000 lines, warning for larger | 92% |
| Prompt design | Structured YAML, versioned | 96% |

---

*"Index the realm. Know the impact. Change with confidence."*

— Blue
