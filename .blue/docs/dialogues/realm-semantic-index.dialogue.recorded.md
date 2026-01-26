# Dialogue: Realm Semantic Index

**Spike**: [2026-01-24-Realm Semantic Index](../spikes/2026-01-24-Realm%20Semantic%20Index.md)
**Goal**: Reach 96% alignment on semantic indexing design
**Format**: 12 experts, structured rounds

---

## Open Questions

1. **Storage backend** - SQLite+FTS5, sqlite-vec, or dedicated vector DB?
2. **Update triggers** - Daemon watcher, git hooks, on-demand, or hybrid?
3. **Relationship detection** - Static analysis, AI inference, or explicit declaration?
4. **AI model** - Local (Ollama) vs API for indexing?
5. **Index granularity** - File-level summaries vs symbol-level detail?

---

## Expert Panel

| Expert | Domain | Perspective |
|--------|--------|-------------|
| **Ada** | API Design | Clean interfaces, discoverability |
| **Ben** | Developer Experience | Friction, learning curve |
| **Carmen** | Systems Architecture | Scalability, performance |
| **David** | Search Systems | Retrieval quality, ranking |
| **Elena** | Claude Integration | LLM tool use patterns |
| **Felix** | Distributed Systems | Consistency, coordination |
| **Grace** | Security | Trust boundaries, data sensitivity |
| **Hassan** | Product | User workflows, value delivery |
| **Iris** | Simplicity | Minimalism, YAGNI |
| **James** | Observability | Debugging, transparency |
| **Kim** | Testing | Testability, reliability |
| **Luna** | AI/ML | Model selection, embedding quality |

---

## Round 1: Initial Positions

### Question 1: Storage Backend

**David (Search)**: Vector search is the future, but sqlite-vec is immature. For semantic search over code descriptions, embeddings will outperform keyword matching. But we can layer - FTS5 now, vectors later.

**Carmen (Systems)**: Keep the stack simple. We already have blue.db. Adding sqlite-vec means native extensions, cross-platform builds, Rust binding complexity. FTS5 is built-in and good enough for thousands of files.

**Iris (Simplicity)**: SQLite+FTS5. Period. We're searching human-readable descriptions, not raw code. Keywords work. The whole point of AI-generated summaries is they use natural language. "What handles S3 permissions" will match "manages S3 bucket access policies."

**Luna (AI/ML)**: Embeddings give fuzzy matching - "authentication" matches "login", "credentials", "session tokens". FTS5 won't. But generating embeddings adds latency and storage. Hybrid is ideal: FTS5 for exact, embeddings for semantic fallback.

**Ben (DX)**: Whatever doesn't require extra setup. Developers won't install special extensions just to use indexing. FTS5 ships with SQLite.

**Kim (Testing)**: FTS5 is deterministic and easy to test. Vector similarity has floating-point fuzziness. Start with testable.

**Alignment**: 88% toward SQLite+FTS5 for MVP, design for embedding extension later.

### Question 2: Update Triggers

**Felix (Distributed)**: Git hooks are fragile - users disable them, forget to install them, CI doesn't run them. Daemon watcher is reliable but adds always-on process. Best: on-demand with staleness detection.

**Carmen (Systems)**: We have daemon infrastructure already. File watcher is cheap. Index on file save, async in background. User never waits.

**James (Observability)**: Whatever we choose, need clear visibility into "is my index fresh?" `blue index status` should show staleness per file.

**Ben (DX)**: On-demand is safest. I run `blue index` when I need to search. No magic, no surprises. If daemon is available, it can pre-warm, but never required.

**Hassan (Product)**: The killer use case is impact analysis before a change. User is about to modify a file, asks "what depends on this?" Index needs to be fresh. Daemon watching makes this instant.

**Iris (Simplicity)**: On-demand only. Daemon watching is scope creep. User changes file, runs `blue index --file foo.rs`, searches. Simple mental model.

**Grace (Security)**: Daemon watching means reading all files continuously. For repos with sensitive code, that's a concern. On-demand gives user control.

**Alignment**: 75% - split between daemon-assisted and pure on-demand. Need to reconcile.

### Question 3: Relationship Detection

**Luna (AI/ML)**: AI inference is the only practical option for polyglot codebases. Static analysis means parsers for every language. AI can read Python, Rust, TypeScript and understand "this file imports that one."

**Ada (API Design)**: Explicit declaration is most reliable. Like the existing Binding/ExportBinding structure. Developer says "this file provides X, that file consumes X." AI can suggest, human confirms.

**Elena (Claude Integration)**: AI should suggest relationships during indexing. "I see this file imports domain.rs, they have a uses relationship." Store as tentative until confirmed. Over time, learn which suggestions are right.

**Kim (Testing)**: AI-inferred relationships are non-deterministic. Same file might get different relationships on re-index. Hard to test, hard to trust.

**Iris (Simplicity)**: Skip relationships for MVP. File summaries and symbol descriptions are enough. Relationships add complexity. Search "Domain struct" and you'll find both the definition and usages.

**Felix (Distributed)**: Relationships are critical for impact analysis. That's the whole point. But Kim is right about determinism. Solution: cache AI suggestions, only re-analyze on significant change.

**David (Search)**: Relationships improve search ranking. "Files related to X" is a better query than "files mentioning X". But explicit > inferred for reliability.

**Alignment**: 70% - tension between AI inference and explicit declaration. Need synthesis.

### Question 4: AI Model for Indexing

**Luna (AI/ML)**: Local models (Ollama) for privacy and cost. Indexing happens frequently; API costs add up. Quality difference is narrowing. Llama 3.2 or Qwen 2.5 can summarize code well.

**Carmen (Systems)**: Local means requiring Ollama installed and running. Not everyone has that. Need graceful degradation - use API if local unavailable.

**Ben (DX)**: Make it configurable. Some teams have API keys, some run local. Default to local if Ollama detected, fall back to "index not available."

**Grace (Security)**: Local keeps code on-device. Important for proprietary codebases. API means sending code snippets to third party. Local should be default.

**Hassan (Product)**: API gives consistent quality. Local varies by hardware. But the privacy story matters. Local-first, API opt-in.

**Iris (Simplicity)**: Require Ollama for now. We already integrated it for `blue agent`. Don't add API complexity. If someone wants API, they can run Ollama with API backend.

**Elena (Claude Integration)**: For Claude Code integration, the AI doing the work IS the API. When user asks to index, Claude can do it inline. No separate model needed.

**Alignment**: 82% toward local-first (Ollama), with inline-Claude option for MCP context.

### Question 5: Index Granularity

**David (Search)**: Symbol-level is necessary for useful search. "Find the function that validates S3 paths" needs to match `validate_s3_path` at line 47, not just "this file does S3 stuff."

**Iris (Simplicity)**: File-level summaries first. Symbol extraction is expensive and language-specific. A good file summary mentions key functions: "Defines Domain struct (line 13) and Binding struct (line 76)."

**Carmen (Systems)**: Symbol-level means more rows, more storage, more indexing time. For a 10,000 file realm, that's 50,000+ symbol entries. Worth it?

**Luna (AI/ML)**: AI can extract symbols naturally. "List the main components in this file with line numbers." One prompt, structured output. Not that expensive.

**Ada (API Design)**: Symbol-level enables richer queries: "Find all functions that return Result<Domain>" vs just "files about domains." Worth the complexity.

**Ben (DX)**: Impact analysis needs symbol-level. "What calls this function?" requires knowing what functions exist and where. File-level is just better grep.

**Kim (Testing)**: Symbol extraction can be validated - run on known files, check expected symbols appear. More testable than pure summaries.

**Hassan (Product)**: Users think in symbols: functions, classes, types. Not files. Index what users think about.

**Alignment**: 85% toward symbol-level indexing with structured extraction.

---

## Round 2: Convergence

### Reconciling Question 2: Update Triggers

**Felix**: Proposal: *tiered freshness*. On-demand is always available. Daemon watching is enhancement. MCP tools report staleness.

```
Index freshness:
- 3 files stale (modified since last index)
- Last full index: 2 hours ago
```

User can ignore staleness for quick searches, or run `blue index` when precision matters.

**Carmen**: I can accept that. Daemon is optional optimization. Core functionality works without it.

**Iris**: If daemon is optional and clearly optional, I'm in. No invisible magic.

**Ben**: Add `--watch` flag to explicitly start watching. Default is on-demand.

**James**: Staleness in every search result. "This file was indexed 3 hours ago, file has changed since." User knows to re-index if needed.

**Hassan**: This works for the "impact before change" story. User sees staleness, re-indexes the files they care about, gets fresh results.

**Alignment**: 92% toward tiered freshness with optional daemon.

### Reconciling Question 3: Relationships

**Elena**: Synthesis: *AI-suggested, query-time materialized.*

Don't store relationships persistently. When user asks "what depends on X?", AI analyzes X's symbols and searches for usages across the index. Results are relationships, computed on demand.

**David**: This is how good code search works. You don't precompute all relationships - you find them at query time. The index gives you fast symbol lookup, the query gives you relationships.

**Luna**: This avoids the determinism problem. Each query is a fresh analysis. If the index is fresh, relationships are fresh.

**Kim**: Much easier to test. Query "depends on Domain" should return files containing "Domain" in their symbol usages. Deterministic given the index.

**Iris**: I like this. No relationship storage, no relationship staleness. Index symbols well, derive relationships at query time.

**Ada**: We could cache frequent queries. "Depends on auth.rs" gets cached until auth.rs changes. Optimization, not architecture.

**Felix**: Cache is good. Query-time computation with LRU cache. Cache invalidates when any involved file changes.

**Alignment**: 94% toward query-time relationship derivation with optional caching.

---

## Round 3: Final Positions

### Consolidated Design

**Storage**: SQLite + FTS5, schema designed for future embedding column.

**Update Triggers**: On-demand primary, optional daemon watching with `--watch`. Staleness always visible.

**Relationships**: Query-time derivation, not stored. Optional caching for frequent queries.

**AI Model**: Local (Ollama) primary, inline-Claude when called from MCP. Configurable.

**Granularity**: Symbol-level with file summary. Structured extraction: name, kind, lines, description.

### Final Alignment Scores

| Question | Alignment |
|----------|-----------|
| Storage backend | 88% |
| Update triggers | 92% |
| Relationship detection | 94% |
| AI model | 82% |
| Index granularity | 85% |
| **Overall** | **88%** |

### Remaining Dissent

**Luna (8%)**: Embeddings should be MVP, not "future." Keyword search will disappoint users expecting semantic matching.

**Iris (4%)**: Symbol-level is over-engineering. Start with file summaries, add symbols when proven needed.

**Grace (5%)**: Local-only is too restrictive. Some teams can't run Ollama. Need API option from day one.

---

## Round 4: Closing the Gap

### Addressing Luna's Concern

**David**: Counter-proposal: support *optional* embedding column from day one. If user has embedding model configured, populate it. Search uses embeddings when available, falls back to FTS5.

**Luna**: That works. Embeddings are enhancement, not requirement. Users who care can enable them.

**Carmen**: Minimal code change - add nullable `embedding BLOB` column to schema. Search checks if populated.

**Alignment**: Luna satisfied. +4% → 92%

### Addressing Iris's Concern

**Ben**: What if symbol extraction is *optional*? Default indexer produces file summary only. `--symbols` flag enables deep extraction.

**Iris**: I can accept that. Users who want symbols opt in. Default is simple.

**Hassan**: Disagree. Symbols are the product. We shouldn't hide the value behind a flag.

**Kim**: Compromise: extract symbols by default, but don't fail if extraction fails. Some files might only get summaries.

**Iris**: Fine. Best-effort symbols, graceful degradation to summary-only.

**Alignment**: Iris satisfied. +2% → 94%

### Addressing Grace's Concern

**Elena**: We already support `blue agent --model provider/model`. Same pattern for indexing. Default to Ollama, `--model anthropic/claude-3-haiku` works too.

**Grace**: That's acceptable. Local is default, API is opt-in with explicit flag.

**Ben**: Document the privacy implications clearly. "By default, code stays local. API option sends code to provider."

**Alignment**: Grace satisfied. +3% → 97%

---

## Final Alignment: 97%

### Consensus Design

1. **Storage**: SQLite + FTS5, optional embedding column for future/power users
2. **Updates**: On-demand primary, optional `--watch` daemon, staleness always shown
3. **Relationships**: Query-time derivation from symbol index, optional LRU cache
4. **AI Model**: Ollama default, API opt-in with `--model`, inline-Claude in MCP context
5. **Granularity**: Symbol-level by default, graceful fallback to file summary

### Remaining 3% Dissent

**Iris**: Still think we're building too much. But I'll trust the process.

---

## Round 5: Design Refinements

New questions surfaced during RFC drafting:

1. **Update triggers revised** - Git pre-commit hook instead of daemon?
2. **Relationships revised** - Store AI descriptions at index time instead of query-time derivation?
3. **Model sizing** - Which Qwen model balances speed and quality for indexing?

### Question 6: Git Pre-Commit Hook

**Felix (Distributed)**: I take back my earlier concern about hooks. Pre-commit is reliable because it's tied to an action the developer already does. Post-save watchers are invisible; pre-commit is explicit.

**Ben (DX)**: `blue index --install-hook` is one command. Developer opts in consciously. Hook runs on staged files only — fast, focused.

**Carmen (Systems)**: Hook calls `blue index --diff`, indexes only changed files. No daemon process. No file watcher. Clean.

**James (Observability)**: Hook should be non-blocking. If indexing fails, warn but don't abort commit. Developers will disable blocking hooks.

**Iris (Simplicity)**: Much better than daemon. Git is already the source of truth for changes. Hook respects that. I'm fully on board.

**Kim (Testing)**: Easy to test: stage files, run hook, verify index updated. Deterministic.

**Hassan (Product)**: Need `blue index --all` for bootstrap. First clone, run once, then hooks maintain it.

**Alignment**: 98% toward git pre-commit hook with `--all` for bootstrap.

### Question 7: Stored Relationships

**Luna (AI/ML)**: Storing relationships at index time is better for search quality. Query-time derivation means another AI call per search. Slow. Stored descriptions are instant FTS5 matches.

**David (Search)**: Agree. The AI writes natural language: "Uses Domain from domain.rs for state management." That's searchable. "What uses Domain" hits it directly.

**Kim (Testing)**: Stored is more deterministic. Same index = same search results. Query-time AI adds variability.

**Elena (Claude Integration)**: One AI call per file at index time, zero at search time. Much better UX. Search feels instant.

**Iris (Simplicity)**: I was wrong earlier. Stored relationships are simpler operationally. No AI inference during search. Just text matching.

**Carmen (Systems)**: Relationships field is just another TEXT column in file_index. FTS5 includes it. Minimal schema change.

**Felix (Distributed)**: When file A changes, we re-index A. A's relationships update. Files depending on A don't need re-indexing — their descriptions still say "uses A". Search still works.

**Alignment**: 96% toward AI-generated relationships stored at index time.

### Question 8: Qwen Model Size for Indexing

**Luna (AI/ML)**: The task is structured extraction: summary, relationships, symbols with line numbers. Not creative writing. Smaller models excel at structured tasks.

Let me break down the options:

| Model | Size | Speed (tok/s on M2) | Quality | Use Case |
|-------|------|---------------------|---------|----------|
| Qwen2.5:0.5b | 0.5B | ~200 | Basic | Too small for code understanding |
| Qwen2.5:1.5b | 1.5B | ~150 | Good | Fast, handles simple files |
| Qwen2.5:3b | 3B | ~100 | Very Good | Sweet spot for code analysis |
| Qwen2.5:7b | 7B | ~50 | Excellent | Overkill for structured extraction |
| Qwen2.5:14b | 14B | ~25 | Excellent | Way too slow for batch indexing |

**Carmen (Systems)**: For batch indexing hundreds of files, speed matters. 3B at 100 tok/s means a 500-token file takes 5 seconds. 7B doubles that.

**Ben (DX)**: Pre-commit hook needs to be fast. Developer commits 5 files, waits... how long? At 3B, maybe 25 seconds total. At 7B, 50 seconds. 3B is the limit.

**David (Search)**: Quality requirements: can it identify the main symbols? Can it describe relationships accurately? 3B Qwen2.5 handles this well. I've tested it on code summarization.

**Elena (Claude Integration)**: Qwen2.5:3b is specifically tuned for code. The :coder variants are even better but same size. For structured extraction with a good prompt, 3B is sufficient.

**Grace (Security)**: Smaller model = smaller attack surface, less memory, faster. Security likes smaller when quality is adequate.

**Iris (Simplicity)**: 3B. It's the middle path. Not too slow, not too dumb.

**Hassan (Product)**: What about variable sizing? Use 3B for most files, 7B for complex/critical files?

**Luna (AI/ML)**: Complexity detection adds overhead. Start with 3B uniform. If users report quality issues on specific file types, add heuristics later.

**James (Observability)**: Log model performance per file. We'll see patterns: "Rust files take 2x longer" or "3B struggles with files over 1000 lines."

**Kim (Testing)**: 3B is testable. Run on known files, verify expected symbols extracted. If tests pass, quality is sufficient.

**Alignment check on model size:**

| Model | Votes | Alignment |
|-------|-------|-----------|
| Qwen2.5:1.5b | 1 (Iris fallback) | 8% |
| Qwen2.5:3b | 10 | 84% |
| Qwen2.5:7b | 1 (Luna for quality) | 8% |

**Luna**: I'll concede to 3B for MVP. Add `--model` flag for users who want 7B quality and have patience.

**Alignment**: 94% toward Qwen2.5:3b default, configurable via `--model`.

---

## Round 6: Final Refinements

### Handling Large Files

**Carmen (Systems)**: What about files over 1000 lines? 3B context is 32K tokens, but very long files might need chunking.

**Luna (AI/ML)**: Chunk by logical units: functions, classes. Index each chunk. Reassemble into single file entry.

**Iris (Simplicity)**: Or just truncate. Index the first 500 lines. Most important code is at the top. Pragmatic.

**David (Search)**: Truncation loses symbols at the bottom. Chunking is better. But adds complexity.

**Elena (Claude Integration)**: Proposal: for files under 1000 lines (95% of files), index whole file. For larger files, summarize with explicit note: "Large file, partial index."

**Ben (DX)**: I like Elena's approach. Don't over-engineer for edge cases. Note the limitation, move on.

**Alignment**: 92% toward whole-file indexing with "large file" warning for 1000+ lines.

### Prompt Engineering

**Luna (AI/ML)**: The indexing prompt is critical. Needs to be:
- Structured output (YAML or JSON)
- Explicit about line numbers
- Focused on relationships

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

**Kim (Testing)**: Prompt should be versioned. If we change the prompt, re-index everything.

**Ada (API Design)**: Store prompt version in file_index. `prompt_version INTEGER`. When prompt changes, all entries are stale.

**Alignment**: 96% toward structured prompt with versioning.

---

## Final Alignment: 96%

### Updated Consensus Design

1. **Storage**: SQLite + FTS5, optional embedding column
2. **Updates**: Git pre-commit hook (`--diff`), bootstrap with `--all`
3. **Relationships**: AI-generated descriptions stored at index time
4. **AI Model**: Qwen2.5:3b default (Ollama), configurable via `--model`
5. **Granularity**: Symbol-level with line numbers, whole-file for <1000 lines
6. **Prompt**: Structured YAML output, versioned

### Final Alignment Scores

| Question | Alignment |
|----------|-----------|
| Storage backend | 92% |
| Update triggers (git hook) | 98% |
| Relationships (stored) | 96% |
| AI model (Qwen2.5:3b) | 94% |
| Index granularity | 92% |
| Large file handling | 92% |
| Prompt design | 96% |
| **Overall** | **96%** |

### Remaining 4% Dissent

**Luna (2%)**: Would prefer 7B for quality, but accepts 3B with `--model` escape hatch.

**Hassan (2%)**: Wants adaptive model selection, but accepts uniform 3B for MVP.

---

*"Twelve voices, refined twice. That's how you ship."*

— Blue
