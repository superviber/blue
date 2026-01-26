# Spike: Agentic Cli Integration

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-24 |
| **Time Box** | 2 hours |

---

## Question

Which commercial-compatible local agentic coding CLI (Aider, Goose, OpenCode) can be integrated into Blue CLI, and what's the best integration pattern?

---

## Findings

### Candidates Evaluated

| Tool | License | Language | MCP Support | Integration Pattern |
|------|---------|----------|-------------|---------------------|
| **Goose** | Apache-2.0 | Rust | Native | MCP client/server, subprocess |
| **Aider** | Apache-2.0 | Python | Via extensions | Subprocess, CLI flags |
| **OpenCode** | MIT | Go | Native | Go SDK, subprocess |

### Goose (Recommended)

**Why Goose wins:**

1. **Same language as Blue** - Rust-based, can share types and potentially link as library
2. **Native MCP support** - Goose is built on MCP (co-developed with Anthropic). Blue already speaks MCP.
3. **Apache-2.0** - Commercial-compatible with patent grant
4. **Block backing** - Maintained by Block (Square/Cash App), contributed to Linux Foundation's Agentic AI Foundation in Dec 2025
5. **25+ LLM providers** - Works with Ollama, OpenAI, Anthropic, local models

**Integration patterns:**

```
Option A: MCP Extension (Lowest friction)
┌─────────────────────────────────────────────┐
│  Goose CLI                                  │
│    ↓ (MCP client)                           │
│  Blue MCP Server (existing blue-mcp)        │
│    ↓                                        │
│  Blue tools: rfc_create, worktree, etc.     │
└─────────────────────────────────────────────┘

Option B: Blue as Goose Extension
┌─────────────────────────────────────────────┐
│  Blue CLI                                   │
│    ↓ (spawns)                               │
│  Goose (subprocess)                         │
│    ↓ (MCP client)                           │
│  Blue MCP Server                            │
└─────────────────────────────────────────────┘

Option C: Embedded (Future)
┌─────────────────────────────────────────────┐
│  Blue CLI                                   │
│    ↓ (links)                                │
│  goose-core (Rust crate)                    │
│    ↓                                        │
│  Local LLM / API                            │
└─────────────────────────────────────────────┘
```

**Recommendation: Option A first**

Goose already works as an MCP client. Blue already has an MCP server (`blue mcp`). The integration is:

```bash
# User installs goose
brew install block/tap/goose

# User configures Blue as Goose extension
# In ~/.config/goose/config.yaml:
extensions:
  blue:
    type: stdio
    command: blue mcp
```

This requires **zero code changes** to Blue. Users get agentic coding with Blue's workflow tools immediately.

### Aider

**Pros:**
- Mature, battle-tested (Apache-2.0)
- Git-native with smart commits
- Strong local model support via Ollama

**Cons:**
- Python-based (foreign to Rust codebase)
- CLI scripting API is "not officially supported"
- No native MCP (would need wrapper)

**Integration pattern:** Subprocess with `--message` flag for non-interactive use.

```rust
// Hypothetical
let output = Command::new("aider")
    .args(["--message", "implement the function", "--yes-always"])
    .output()?;
```

**Verdict:** Viable but more friction than Goose.

### OpenCode

**Pros:**
- MIT license (most permissive)
- Go SDK available
- Native MCP support
- Growing fast (45K+ GitHub stars)

**Cons:**
- Go-based (FFI overhead to call from Rust)
- Newer, less mature than Aider
- SDK is for Go clients, not embedding

**Integration pattern:** Go SDK or subprocess.

**Verdict:** Good option if Goose doesn't work out.

### Local LLM Backend

All three support Ollama for local models:

```bash
# Install Ollama
brew install ollama

# Pull a coding model (Apache-2.0 licensed)
ollama pull qwen2.5-coder:32b    # 19GB, best quality
ollama pull qwen2.5-coder:7b     # 4.4GB, faster
ollama pull deepseek-coder-v2    # Alternative
```

Goose config for local:
```yaml
# ~/.config/goose/config.yaml
provider: ollama
model: qwen2.5-coder:32b
```

## Outcome

**Recommends implementation** with Goose as the integration target.

### Immediate (Zero code):
1. Document Blue + Goose setup in docs/
2. Ship example `goose-extension.yaml` config

### Short-term (Minimal code):
1. Add `blue agent` subcommand that launches Goose with Blue extension pre-configured
2. Add Blue-specific prompts/instructions for Goose

### Medium-term (More code):
1. Investigate goose-core Rust crate for tighter integration
2. Consider Blue daemon serving as persistent MCP host

## Sources

- [Goose GitHub](https://github.com/block/goose)
- [Goose Architecture](https://block.github.io/goose/docs/goose-architecture/)
- [Aider Scripting](https://aider.chat/docs/scripting.html)
- [OpenCode Go SDK](https://pkg.go.dev/github.com/sst/opencode-sdk-go)
- [Goose MCP Deep Dive](https://dev.to/lymah/deep-dive-into-gooses-extension-system-and-model-context-protocol-mcp-3ehl)
