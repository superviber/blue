# Spike: Borrowing from RLabs Memory for Blue Session Continuity

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 2 hours |

---

## Question

What ideas from the RLabs/memory project (semantic memory across sessions for AI CLI tools) could improve Blue's context injection and session continuity? Which are worth implementing?

---

## Investigation

### What RLabs/memory Does

[RLabs/memory](https://github.com/RLabs-Inc/memory) is a Python/FastAPI semantic memory server that gives AI CLI tools (Claude Code, Gemini CLI) persistent understanding across conversations. It hooks into three lifecycle points: session start, message submission, and session end.

**Core loop:**
1. **Session start** -- inject a "session primer" (temporal context: "we last spoke 2 days ago, you were working on X")
2. **Each user message** -- retrieve up to 5 relevant memories and inject them as context
3. **Session end** -- curate the transcript, extract memories worth keeping

**Key design decisions:**
- AI curates memories for AI (Claude analyzes transcripts, decides what's worth remembering)
- Two-stage retrieval: obligatory memories (action-required, importance > 0.85) + intelligent scoring
- Multi-dimensional scoring (10 factors): trigger phrases, vector similarity, semantic tags, importance weight, temporal relevance, confidence, emotional resonance, problem-solution pairing, context type alignment, action priority
- Memories are "knowledge capsules" that stand alone without original context
- Project isolation: separate memory spaces per project
- ChromaDB for vectors, SQLite for metadata, MiniLM-L6 for embeddings

### What Blue Already Has

Blue's context system (RFC 0016 + 0017) operates differently:

| Aspect | RLabs/memory | Blue |
|--------|-------------|------|
| **Storage** | ChromaDB vectors + SQLite | SQLite + FTS5 + Ollama embeddings |
| **Embeddings** | MiniLM-L6 (sentence-transformers) | Ollama qwen2.5:3b |
| **Curation** | AI-curated from transcripts | Manual (documents authored by humans/AI) |
| **Retrieval** | Per-message, 5 memories max | Three-tier injection (identity/workflow/reference) |
| **Session awareness** | Session primer with temporal context | Session table with heartbeat |
| **Staleness** | Temporal relevance field | Content-hash based (SHA-256) |
| **Scope** | Single project memories | Cross-repo realms |
| **Knowledge type** | Extracted insights from conversations | Structured documents (RFCs, ADRs, spikes) |

Blue's strengths: structured knowledge, cross-repo coordination, document lifecycle, alignment dialogues.
Blue's gaps: no conversational memory, no per-message retrieval, no session primers, no automatic curation.

---

## Ideas Worth Borrowing

### 1. Session Primers (High Value)

**What:** At session start, inject a brief temporal orientation -- "Last session was 3 hours ago. You were implementing RFC 0017. Tasks 2 and 4 were completed. Task 5 is next."

**Why it matters:** Blue already tracks sessions and RFC state. A primer would ground Claude immediately instead of requiring `blue_status` calls. This is the lowest-hanging fruit.

**Blue-native implementation:** Extend `blue_session_start` or the MCP `initialize` handler to return a primer built from:
- Last session end time (sessions table)
- Active RFC and task progress (documents + tasks tables)
- Recent git activity (last N commits on branch)
- Pending reminders (reminders table)

**Effort:** Small. Data already exists in blue.db. Just needs a formatter.

### 2. Conversational Memory Curation (High Value, High Effort)

**What:** After a session, analyze what happened and extract durable insights -- decisions made, problems solved, patterns discovered, unresolved questions.

**Why it matters:** Blue captures deliberate documents (RFCs, ADRs) but misses the organic knowledge that emerges in conversation. A developer might explain "we avoided X because of Y" -- that insight dies when the session ends.

**Blue-native implementation:**
- New table: `session_memories` (session_id, content, reasoning, importance, tags, context_type, embedding, created_at)
- Hook into session end to curate via local LLM (Ollama) or Claude
- Store as embeddings in the existing file_index infrastructure
- Surface via `blue_index_search` or a new `blue_memory_recall` tool

**Key difference from RLabs:** Blue should curate into its existing document types where appropriate. An insight that "we decided X" should become an ADR or decision, not just a memory blob. Memory is the catch-all for everything that doesn't warrant a formal document.

**Effort:** Medium-large. Needs curation prompts, new table, embedding pipeline, retrieval integration.

### 3. Per-Message Context Retrieval with Multi-Dimensional Scoring (High Value)

**What:** On every user message, run a full retrieval pipeline that searches across all Blue knowledge sources, scores candidates on multiple dimensions, and injects the highest-value context. Not a keyword search -- a proper memory system.

**Why it matters:** Blue currently injects context at session start (identity tier) and on RFC changes (workflow tier). It has no idea what the user is actually talking about mid-conversation. Every message is an opportunity to surface the right knowledge at the right time.

**Blue-native implementation -- full pipeline:**

#### Stage 0: Obligatory Injection (Gatekeeper)
Before scoring, force-include items that must always surface:
- Reminders with `gate: always` or past-due reminders
- Action-required session memories (importance > 0.9)
- Failing health checks or blocking issues on the active RFC
- Unresolved questions from the last session

These bypass scoring entirely. Cap at 2 obligatory slots.

#### Stage 1: Candidate Retrieval (Wide Net)
Query all Blue knowledge sources in parallel:
- **session_memories** -- vector similarity + FTS5 on curated conversation memories
- **file_index** -- vector similarity + FTS5 on file summaries and symbol descriptions
- **documents** -- FTS5 on RFCs, ADRs, spikes, decisions
- **relevance_edges** -- graph walk from any matched nodes to find related context
- **symbol_index** -- exact and fuzzy symbol name matching

Each source returns its top-K candidates (e.g., 10 per source). This is the wide net -- 50 candidates maximum.

#### Stage 2: Multi-Dimensional Scoring (Narrow Filter)
Score every candidate on these dimensions, weighted:

| Dimension | Weight | Source |
|-----------|--------|--------|
| **Vector similarity** | 0.25 | Cosine distance between message embedding and candidate embedding |
| **FTS5 relevance** | 0.15 | SQLite FTS5 rank score (BM25) |
| **Graph proximity** | 0.15 | Shortest path distance in relevance_edges from active context (current RFC, recent files) |
| **Importance** | 0.12 | Curator-assigned importance weight (session_memories) or document status weight (implemented RFC > draft) |
| **Recency** | 0.10 | Decay function on created_at/updated_at -- recent knowledge scores higher |
| **Trigger phrase match** | 0.08 | Exact or fuzzy match against stored trigger phrases (session_memories) |
| **Context type alignment** | 0.08 | Does the candidate's type match the user's apparent intent? (e.g., asking "how" favors decisions/ADRs, asking "where" favors file_index) |
| **Session novelty** | 0.07 | Penalty for already-injected memories this session (deduplication via context_injections audit log) |

Total: 1.00. Minimum threshold: 0.35 (candidates below this are discarded).

#### Stage 3: Selection and Formatting
- Take top 5 candidates (minus obligatory slots used in Stage 0)
- Format each as a compact context block with source attribution (e.g., `[RFC 0017]`, `[Memory: 2026-01-25]`, `[src/indexer.rs]`)
- Inject as system context via Claude Code hook response
- Log to context_injections table for audit and deduplication

#### Integration Point
Claude Code `UserPromptSubmit` hook calls Blue's retrieval endpoint. Blue runs the full pipeline in Rust (parallel candidate retrieval, scoring, selection) and returns formatted context. Target latency: <200ms for the full pipeline (Rust + SQLite is fast enough).

#### What Makes This State-of-the-Art Beyond RLabs
- **Graph-aware scoring** -- RLabs uses flat vector search. Blue has relevance_edges, giving it structural understanding of how knowledge relates. A memory about "auth middleware" scores higher when the user is working on a file that the graph connects to auth.
- **Cross-source retrieval** -- RLabs searches one memory store. Blue searches across 5 heterogeneous sources (memories, files, documents, symbols, graph edges) and unifies scoring.
- **Structured knowledge advantage** -- RLabs treats everything as memory blobs. Blue knows the difference between an ADR (authoritative decision), an RFC (active work), a spike (investigation), and a session memory (organic insight). Context type alignment exploits this.
- **Realm-aware** -- Retrieval can span repos within a realm when relevant.
- **Auditable** -- Every injection is logged with content hash and token count, enabling staleness detection and quality measurement over time.

#### Prerequisite: Blue Has No Vector Search Today

The `embedding BLOB` column exists in `file_index` (`store.rs:169`) but is never populated. The indexer generates text summaries via Ollama and stores them as strings. All current search is FTS5 (BM25 keyword matching). There is no cosine similarity computation, no ANN index, no `sqlite-vec` extension.

This means the highest-weighted scoring dimension (vector similarity at 0.25) is currently impossible. FTS5 is lexical -- it matches words. If a user asks about "authentication patterns" and Blue has a memory about "login middleware design", FTS5 won't find it. Vector search will.

**Embedding strategy for Blue:**

Blue needs to generate embeddings for:
- User messages (at query time, for retrieval)
- Session memories (at curation time)
- File index entries (at indexing time -- the BLOB column is already there)
- Document summaries (at document creation/update time)

Embedding model options:
- **Ollama with a small embedding model** (e.g., `nomic-embed-text`, `all-minilm`) -- consistent with Blue's local-first approach, no external API dependency
- **sentence-transformers via ONNX runtime in Rust** -- faster inference, no Ollama dependency for embeddings, but adds a native dependency

**Vector search strategy for Blue:**

| Option | Verdict |
|--------|---------|
| **`sqlite-vec`** | **Recommended.** Zero runtime deps (single C file compiled via `cc`). No `libsqlite3-sys` conflict -- just exports a function pointer for `sqlite3_auto_extension`. Compatible with Blue's rusqlite 0.32. Vectors live in SQLite shadow tables alongside all Blue data. Full CRUD. Same virtual table pattern as FTS5 (Blue already uses FTS5). SIMD-accelerated distance functions. Binary quantization (32x compression, ~95% accuracy). ANN indexes planned for v0.2.0+. |
| **Brute-force cosine in Rust** | Viable but worse. Same performance at project scale, but you manage serialization yourself, lose SQL semantics, and can't upgrade to ANN later without rewriting. |
| **In-process Rust HNSW** (`hnsw_rs`, `instant-distance`) | Wrong tradeoffs. No deletion support (memories get superseded). Separate persistence (consistency drift with SQLite). Parameter tuning. Approximate results where brute-force is already fast enough. Solves a latency problem that doesn't exist at project scale. |
| **ChromaDB** | Wrong fit. Python sidecar, poor concurrency, uses SQLite internally anyway. |

#### Why sqlite-vec from the start

**Compatibility:** Zero runtime dependencies. Compiles a single C file (`sqlite-vec.c`) via the `cc` crate at build time. Does NOT depend on `libsqlite3-sys`. Dev-dependency is `rusqlite = "0.31.0"` (older than Blue's 0.32). Integration is one unsafe block:

```rust
unsafe {
    sqlite3_auto_extension(Some(std::mem::transmute(
        sqlite3_vec_init as *const ()
    )));
}
```

**Same pattern Blue already knows:** `vec0` virtual tables work exactly like FTS5 virtual tables. Blue already creates FTS5 tables in `store.rs`. Adding a `vec0` table is the same pattern:

```sql
CREATE VIRTUAL TABLE vec_memories USING vec0(
    embedding float[384]
);
```

Query via standard SQL:
```sql
SELECT rowid, distance
FROM vec_memories
WHERE embedding MATCH ?1
ORDER BY distance
LIMIT 10
```

**Performance at Blue's scale:** Benchmarks from the v0.1.0 release (sift1m dataset):
- 1M vectors, 128-dim: 33ms query (vec0 virtual table)
- 500K vectors, 960-dim: 41ms query
- 100K vectors, 3072-dim: 214ms (float), 11ms (binary quantized)

Blue's scale: ~5K vectors, 384-dim. Extrapolating conservatively: **<1ms per query**. Well within the 200ms pipeline budget.

**Binary quantization:** sqlite-vec supports 1-bit quantization via `bit[N]` columns. 32x size reduction with ~95% accuracy for models trained with binary quantization loss (nomic-embed-text, MixedBread). This keeps blue.db small even as memories accumulate.

**Full CRUD:** Unlike HNSW libraries, sqlite-vec supports INSERT, UPDATE, DELETE. When a memory is superseded or a file is renamed, you can update or remove its vector. No ghost entries. No index rebuild.

**Single source of truth:** Vectors, metadata, FTS5 indices, and relational data all live in one blue.db. No consistency drift between an HNSW file and a database.

**Upgrade path built in:** sqlite-vec v0.2.0+ plans ANN indexes (HNSW, IVF, DiskANN) behind the same SQL interface. When Blue's vector count grows, the query stays the same -- the index changes underneath.

**Risks:**
- Pre-v1 (currently 0.1.6) -- expect breaking changes. But the API is SQL, and SQL is stable.
- 135 open issues -- active project, not abandoned. Most are feature requests.
- No metadata filtering yet (planned for v0.2.0). Blue works around this by joining vec0 results with metadata tables.
- Windows build issue (#21) -- Blue primarily targets macOS/Linux.
- 6.7K stars, Mozilla-backed, MIT/Apache-2.0.

Recommendation: **`sqlite-vec` from day one.** It fits Blue's architecture (single SQLite binary, rusqlite, FTS5 patterns), solves the vector search gap without introducing new operational complexity, and has a clear upgrade path to ANN when scale demands it.

**Effort:** Large. This is the centrepiece feature. Needs: sqlite-vec integration, embedding generation pipeline (Ollama), vec0 table schema, parallel candidate retrieval across 5 sources, multi-dimensional scoring engine, hook integration, context formatting, audit logging. Deserves its own RFC.

### 5. Obligatory Memory Pattern (Medium Value)

**What:** Some memories are flagged as "must inject" regardless of query relevance -- action-required items, critical decisions, unresolved blockers.

**Blue-native implementation:** This maps directly to Blue's reminders system. Reminders with `gate: session_start` already serve this purpose. Could extend to:
- Reminders with `gate: always` (inject on every message)
- High-priority unresolved items from RFC task lists
- Failing health checks from `blue_health_check`

**Effort:** Small. Reminders infrastructure exists. Just need to surface them more aggressively.

### 6. Project Isolation (Already Solved)

Blue's realm system already handles project isolation far more sophisticatedly than RLabs/memory's per-project memory spaces. No action needed.

---

## What NOT to Borrow

1. **ChromaDB** -- Python sidecar, poor concurrency, uses SQLite internally. Blue should use `sqlite-vec` instead -- same database, zero runtime deps, compiles a single C file, same virtual table pattern as FTS5.

2. **In-process Rust HNSW** (`hnsw_rs`, `instant-distance`) -- No deletion support (memories get superseded in Blue), separate persistence (consistency drift with SQLite), parameter tuning overhead, approximate results where brute-force is already sub-millisecond. `sqlite-vec` gives 100% recall, full CRUD, single database, and plans ANN indexes for v0.2.0+ behind the same SQL interface.

3. **Python/FastAPI server** -- Blue is Rust. The memory engine should be native Rust, not a sidecar process.

4. **Transcript-based curation** -- RLabs reads JSONL conversation logs. Blue should use Claude Code hooks to capture session context directly, not parse transcript files.

5. **Emotional resonance scoring** -- Interesting but off-brand for Blue. Blue values evidence and integrity over sentiment.

6. **"Consciousness continuity" framing** -- Blue has its own philosophy. Borrowing ideas is good; borrowing metaphysics is not.

---

## Recommended Path

**Phase 1: Session Primers + Obligatory Context**
- Extend `blue_session_start` to return a structured primer
- Include: time since last session, active RFC summary, pending tasks, due reminders, critical health failures
- Wire into MCP initialize or as an auto-injected resource
- This is table stakes -- gets Blue oriented immediately

**Phase 2: sqlite-vec Integration + Embedding Pipeline** (requires RFC)
- Add `sqlite-vec` as a dependency (`cargo add sqlite-vec`), register via `sqlite3_auto_extension`
- Create `vec0` virtual tables for memories, file index, and document embeddings
- Build embedding generation pipeline via Ollama (`nomic-embed-text` or `all-minilm`, 384-dim)
- Populate embeddings for existing file_index entries and documents
- Consider binary quantization (`bit[384]`) to keep blue.db compact
- This is the infrastructure -- everything else depends on vectors being searchable

**Phase 3: Conversational Memory Curation** (requires RFC)
- Add `session_memories` table with full metadata (importance, tags, trigger phrases, context type)
- Corresponding `vec0` virtual table for memory embeddings
- Build curation pipeline using Ollama at session end
- Curate into existing document types where appropriate (decisions become ADRs, not just memory blobs)
- This is the foundation -- retrieval is only as good as what's been remembered

**Phase 4: Multi-Dimensional Per-Message Retrieval** (requires RFC)
- Full pipeline: obligatory injection → parallel candidate retrieval across 5 sources (vec0 + FTS5 + relevance_edges) → multi-dimensional scoring → selection → formatted injection → audit logging
- Claude Code `UserPromptSubmit` hook integration
- Graph-aware, cross-source, realm-aware, auditable
- Target: <200ms latency, top-5 context injection per message
- This is the centrepiece -- the thing that makes Blue feel like it genuinely knows the project

**Phase 5: Feedback Loop**
- Track which injected memories the user actually references or acts on
- Use engagement signal to adjust importance weights over time
- Memories that never surface decay; memories that consistently help strengthen
- Zero-weight initialization: new memories must prove their value

---

## Outcome

**Recommends implementation.** Session primers (Phase 1) are a quick win. sqlite-vec integration (Phase 2) is the infrastructure foundation. Conversational memory (Phase 3) and per-message retrieval (Phase 4) are the transformative features and each deserve their own RFC.

The vector search foundation should be **`sqlite-vec`** from day one -- zero runtime deps, single C file compiled via `cc`, compatible with Blue's rusqlite 0.32, same virtual table pattern as FTS5, full CRUD, sub-millisecond at project scale, with ANN indexes planned for v0.2.0+ behind the same SQL interface.

The core insight from RLabs/memory: **Blue is excellent at deliberate knowledge (RFCs, ADRs, decisions) but has no mechanism for organic knowledge from conversation, and no way to surface the right knowledge at the right moment.** The retrieval pipeline (Phase 4) is where Blue can leapfrog RLabs -- by combining structured documents, semantic memories, file index, symbol index, and a relevance graph into a unified scoring pipeline that no flat vector store can match.
