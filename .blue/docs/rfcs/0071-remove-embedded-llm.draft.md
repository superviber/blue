# RFC 0071: Remove Embedded LLM Tooling

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-16 |
| **Story** | — |
| **Deprecates** | RFC 0005 |

---

## Summary

Remove all embedded LLM tooling from Blue — the `blue-ollama` crate, the `LlmProvider` abstraction in `blue-core`, and the 8 LLM-related MCP tools. Blue delegates AI work to Claude Code (which already runs Blue); bundling a separate LLM runtime adds build complexity, binary size, and maintenance burden with no unique value.

## Problem

1. **Build-time binary download.** `blue-ollama/build.rs` downloads the Ollama binary (~100MB) from GitHub during `cargo build`. This slows builds, breaks air-gapped environments, and requires platform-specific download logic for macOS/Linux/Windows.

2. **Dead weight.** Blue runs inside Claude Code. The user already has a frontier model. The embedded Ollama path was speculative — useful for offline semantic indexing — but in practice nobody runs `blue llm start` when they have Claude sitting right there.

3. **Maintenance surface.** The LLM abstraction (`LlmProvider` trait, `LlmManager` fallback chain, `KeywordLlm`, `MockLlm`, config types) is 584 lines of code that exists to support a capability nobody uses. The MCP handlers add another 440 lines. The ollama crate adds 849 lines plus a 131-line build script.

4. **Install size.** The Ollama binary and build artifacts add ~200MB to the target directory. Removing them meaningfully improves first-build experience.

## What Gets Removed

### Crate: `blue-ollama` (entire crate)

| File | Lines | Purpose |
|------|-------|---------|
| `crates/blue-ollama/src/lib.rs` | 849 | `EmbeddedOllama` lifecycle, `OllamaLlm` provider, model management |
| `crates/blue-ollama/build.rs` | 131 | Downloads Ollama binary at build time |
| `crates/blue-ollama/Cargo.toml` | ~20 | Crate manifest |

Removes: embedded Ollama server management, binary download, SHA256 verification, port conflict detection, health monitoring.

### Module: `blue-core/src/llm.rs` (entire file)

| Component | Lines | Purpose |
|-----------|-------|---------|
| `LlmProvider` trait | ~30 | Abstraction for LLM backends |
| `LlmManager` | ~80 | Fallback chain (Ollama → API → Keywords) |
| `KeywordLlm` | ~120 | Keyword-based fallback when no LLM available |
| `MockLlm` | ~40 | Test support |
| Config types | ~100 | `LlmConfig`, `LocalLlmConfig`, `ApiLlmConfig` |
| Error types | ~40 | `LlmError` enum |
| Tests | ~170 | Unit tests |

### MCP Tools: `blue-mcp/src/handlers/llm.rs` (entire file)

8 tools removed:

| Tool | Purpose |
|------|---------|
| `blue_llm_start` | Start embedded Ollama server |
| `blue_llm_stop` | Stop embedded Ollama server |
| `blue_llm_status` | Health check |
| `blue_llm_providers` | List provider fallback chain |
| `blue_model_list` | List downloaded models |
| `blue_model_pull` | Download model from registry |
| `blue_model_remove` | Delete model |
| `blue_model_warmup` | Load model into memory |

### Integration Points

| File | Change | Lines |
|------|--------|-------|
| `blue-core/src/lib.rs` | Remove `pub mod llm` and re-exports | ~5 |
| `blue-core/src/indexer.rs` | Remove `P: LlmProvider` generic, use keyword matching directly | ~20 |
| `blue-mcp/src/server.rs` | Remove 8 tool registrations and dispatch arms | ~28 |
| `blue-mcp/src/handlers/mod.rs` | Remove `pub mod llm` | 1 |
| `blue-cli/src/main.rs` | Remove `detect_ollama_model()`, LLM init in agent/index/pre-commit | ~45 |
| `Cargo.toml` (workspace) | Remove `blue-ollama` from members | 1 |

### Dependencies Potentially Removable

| Dependency | Used By | Safe to Remove? |
|------------|---------|----------------|
| `sha2` | Only `blue-ollama` binary verification | Check if used elsewhere first |
| `dirs` | `blue-ollama` model directory discovery | Check if used elsewhere first |

Note: `reqwest`, `tokio`, `serde` are used broadly and stay.

### Documentation

| File | Action |
|------|--------|
| `.blue/docs/rfcs/0005-local-llm-integration.impl.md` | Mark as deprecated, add note pointing to this RFC |

## What Stays

- **`KeywordLlm` logic** (keyword matching for ADR relevance, runbook lookup) moves into the modules that use it directly. This isn't LLM — it's string matching. It doesn't need an `LlmProvider` trait wrapping it.
- **`blue index`** continues to work using keyword-based indexing (the `KeywordLlm` path). The LLM-enhanced indexing was never the default.
- **`blue agent`** (Goose integration) — removed entirely. Blue runs inside Claude Code; a separate agent runtime adds no value.

## What Does NOT Change

- Blue's MCP server and all non-LLM tools
- Jira integration, PM system, realm system
- Alignment dialogues
- Claude Code skill system

## Implementation Phases

### Phase 1: Remove `blue-ollama` crate

- Delete `crates/blue-ollama/` entirely
- Remove from workspace `Cargo.toml` members list
- Remove `blue-ollama` dependency from `blue-mcp/Cargo.toml` and `blue-cli/Cargo.toml`
- Fix compilation errors in dependent crates

### Phase 2: Remove MCP LLM tools

- Delete `crates/blue-mcp/src/handlers/llm.rs`
- Remove `pub mod llm` from `handlers/mod.rs`
- Remove 8 tool definitions and dispatch arms from `server.rs`
- Remove `OLLAMA` singleton

### Phase 3: Remove `blue-core/src/llm.rs`

- Delete `llm.rs`
- Remove `pub mod llm` and re-exports from `lib.rs`
- Inline keyword matching logic where it's actually used (indexer, ADR relevance)
- Remove `P: LlmProvider` generic from indexer — replace with direct keyword matching

### Phase 4: Clean up CLI

- Remove `detect_ollama_model()` from `main.rs`
- Remove LLM initialization from `blue index`, `blue pre-commit`
- Remove `blue agent` command entirely (Goose integration)

### Phase 5: Clean up dependencies

- Audit `sha2`, `dirs` for other usage — remove if orphaned
- Run `cargo build` to verify clean compilation
- Run full test suite

## Open Questions

- [x] ~~Should `blue agent` (Goose integration) also be removed?~~ **Yes.** Remove the entire `blue agent` command and Goose integration.
- [ ] Should RFC 0005 be marked as deprecated or deleted from the docs? (Recommendation: mark deprecated with pointer to this RFC.)

## Test Plan

- [ ] `cargo build --release` succeeds without downloading Ollama binary
- [ ] All remaining tests pass
- [ ] `blue install` succeeds and no LLM tools appear in MCP tool list
- [ ] `blue index` still works using keyword-based matching
- [ ] ADR relevance matching still works without LlmProvider
- [ ] Binary size decreases meaningfully
- [ ] Build time decreases (no network download step)

---

*"Right then. Let's get to it."*

— Blue
