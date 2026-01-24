# Spike: Realm Semantic Index

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-24 |
| **Time Box** | 4 hours |

---

## Question

How can we create an AI-maintained semantic index of files within a realm, tracking what each file does (with line references), its relationships to other files, and enabling semantic search for change impact analysis?

---

## Context

Realms coordinate across repos. Domains define relationships (provider/consumer, exports/imports). But when a file changes, there's no quick way to know:
- What does this file actually do?
- What other files depend on it?
- What's the blast radius of a change?

We want an AI-maintained index that answers these questions via semantic search.

## Design Space

### What Gets Indexed

For each file in a realm:

```yaml
file: src/realm/domain.rs
last_indexed: 2026-01-24T10:30:00Z
hash: abc123  # for change detection

summary: "Domain definitions for cross-repo coordination. Defines Domain, Binding, ExportBinding, ImportBinding types."

symbols:
  - name: Domain
    kind: struct
    lines: [13, 73]
    description: "A coordination context between repos with name, description, creation time, and member list"

  - name: Binding
    kind: struct
    lines: [76, 143]
    description: "Declares what a repo exports or imports in a domain"

  - name: ImportStatus
    kind: enum
    lines: [259, 274]
    description: "Status of an import binding: Pending, Current, Outdated, Broken"

relationships:
  - target: src/realm/service.rs
    kind: used_by
    description: "RealmService uses Domain and Binding to manage cross-repo state"

  - target: src/realm/repo.rs
    kind: used_by
    description: "Repo operations load/save Domain and Binding files"
```

### Storage Options

| Option | Pros | Cons |
|--------|------|------|
| **SQLite + FTS5** | Already have blue.db, full-text search built-in | No semantic/vector search |
| **SQLite + sqlite-vec** | Vector similarity search, keeps single DB | Requires extension, Rust bindings unclear |
| **Separate JSON files** | Human-readable, git-tracked | Slow to search at scale |
| **Embedded vector DB (lancedb)** | Purpose-built for semantic search | Another dependency |

**Recommendation:** Start with SQLite + FTS5 for keyword search. Add embeddings later if needed.

### Index Update Triggers

1. **On-demand** - `blue index` command regenerates
2. **Git hook** - Post-commit hook calls `blue index --changed`
3. **File watcher** - Daemon watches for changes (already have daemon infrastructure)
4. **MCP tool** - `blue_index_file` for AI agents to update during work

Likely want combination: daemon watches + on-demand refresh.

### Semantic Search Approaches

**Phase 1: Keyword + Structure**
- FTS5 for text search across summaries and descriptions
- Filter by file path, symbol kind, relationship type
- Good enough for "find files related to authentication"

**Phase 2: Embeddings**
- Generate embeddings for each symbol description
- Store in sqlite-vec or similar
- Query: "what handles S3 bucket permissions" → vector similarity

### Relationship Detection

AI needs to identify relationships. Approaches:

1. **Static analysis** - Parse imports/uses (language-specific, complex)
2. **AI inference** - "Given file A and file B, describe their relationship"
3. **Explicit declarations** - Like current ExportBinding/ImportBinding
4. **Hybrid** - AI suggests, human confirms

**Recommendation:** AI inference with caching. When indexing file A, ask AI to describe relationships to files it references.

## Proposed Schema

```sql
-- File-level index
CREATE TABLE file_index (
    id INTEGER PRIMARY KEY,
    realm TEXT NOT NULL,
    repo TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    summary TEXT,
    indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(realm, repo, file_path)
);

-- Symbol-level index
CREATE TABLE symbol_index (
    id INTEGER PRIMARY KEY,
    file_id INTEGER REFERENCES file_index(id),
    name TEXT NOT NULL,
    kind TEXT NOT NULL,  -- struct, fn, enum, class, etc.
    start_line INTEGER,
    end_line INTEGER,
    description TEXT
);

-- Relationships between files
CREATE TABLE file_relationships (
    id INTEGER PRIMARY KEY,
    source_file_id INTEGER REFERENCES file_index(id),
    target_file_id INTEGER REFERENCES file_index(id),
    kind TEXT NOT NULL,  -- uses, used_by, imports, exports, tests
    description TEXT
);

-- FTS5 virtual table for search
CREATE VIRTUAL TABLE file_search USING fts5(
    file_path,
    summary,
    symbol_names,
    symbol_descriptions,
    content=file_index
);
```

## Proposed MCP Tools

| Tool | Purpose |
|------|---------|
| `blue_index_realm` | Index all files in a realm |
| `blue_index_file` | Index a single file (for incremental updates) |
| `blue_index_search` | Semantic search across the index |
| `blue_index_impact` | Given a file, show what depends on it |
| `blue_index_status` | Show indexing status and staleness |

## Open Questions

1. **Which AI model for indexing?** Local (Ollama) for cost, or API for quality?
2. **How to handle large files?** Chunk by function/class? Summary only?
3. **Cross-realm relationships?** Index within realm first, cross-realm later?
4. **Embedding model?** If we go vector route, which embedding model?

## Next Steps

If this spike looks good:
1. Create RFC for the full design
2. Start with SQLite schema + FTS5
3. Add `blue_index_file` tool that takes AI-generated index data
4. Add daemon file watcher for auto-indexing

---

*Investigation notes by Blue*
