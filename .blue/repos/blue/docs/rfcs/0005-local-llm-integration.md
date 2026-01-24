# RFC 0005: Local Llm Integration

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-24 |
| **Source Spike** | local-llm-integration, agentic-cli-integration |

---

## Summary

Blue needs local LLM capabilities for:
1. **Semantic tasks** - ADR relevance, runbook matching, dialogue summarization (lightweight, fast)
2. **Agentic coding** - Full code generation via Goose integration (heavyweight, powerful)

Unified approach: **Ollama as shared backend** + **Goose for agentic tasks** + **Blue's LlmProvider for semantic tasks**.

Must support CUDA > MPS > CPU backend priority.

## Background

### Two Use Cases

| Use Case | Latency | Complexity | Tool |
|----------|---------|------------|------|
| **Semantic tasks** | <500ms | Short prompts, structured output | Blue internal |
| **Agentic coding** | Minutes | Multi-turn, code generation | Goose |

### Blue's Semantic Tasks

| Feature | RFC | Need |
|---------|-----|------|
| ADR Relevance | 0004 | Match context to philosophical ADRs |
| Runbook Lookup | 0002 | Semantic action matching |
| Dialogue Summary | 0001 | Extract key decisions |

### Why Local LLM?

- **Privacy**: No data leaves the machine
- **Cost**: Zero per-query cost after model download
- **Speed**: Sub-second latency for short tasks
- **Offline**: Works without internet

### Why Embed Ollama?

| Approach | Pros | Cons |
|----------|------|------|
| llama-cpp-rs | Rust-native | Build complexity, no model management |
| Ollama (external) | Easy setup | User must install separately |
| **Ollama (embedded)** | Single install, full features | Larger binary, Go dependency |

**Embedded Ollama wins because:**
1. **Single install** - `cargo install blue` gives you everything
2. **Model management built-in** - pull, list, remove models
3. **Goose compatibility** - Goose connects to Blue's embedded Ollama
4. **Battle-tested** - Ollama handles CUDA/MPS/CPU, quantization, context
5. **One model, all uses** - Semantic tasks + agentic coding share model

### Ollama Version

Blue embeds a specific, tested Ollama version:

| Blue Version | Ollama Version | Release Date |
|--------------|----------------|--------------|
| 0.1.x | 0.5.4 | 2026-01 |

Version pinned in `build.rs`. Updated via Blue releases, not automatically.

## Proposal

### 1. LlmProvider Trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str, options: &CompletionOptions) -> Result<String>;
    fn name(&self) -> &str;
}

pub struct CompletionOptions {
    pub max_tokens: usize,
    pub temperature: f32,
    pub stop_sequences: Vec<String>,
}
```

### 2. Implementations

```rust
pub enum LlmBackend {
    Ollama(OllamaLlm),   // Embedded Ollama server
    Api(ApiLlm),          // External API fallback
    Mock(MockLlm),        // Testing
}
```

**OllamaLlm**: Embedded Ollama server managed by Blue
**ApiLlm**: Uses Anthropic/OpenAI APIs (fallback)
**MockLlm**: Returns predefined responses (testing)

### 2.1 Embedded Ollama Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Blue CLI                                                │
├─────────────────────────────────────────────────────────┤
│  blue-ollama (embedded)                                  │
│  ├── Ollama server (Go, compiled to lib)                │
│  ├── Model management (pull, list, remove)              │
│  └── HTTP API on localhost:11434                        │
├─────────────────────────────────────────────────────────┤
│  Consumers:                                              │
│  ├── Blue semantic tasks (ADR relevance, etc.)          │
│  ├── Goose (connects to localhost:11434)                │
│  └── Any Ollama-compatible client                       │
└─────────────────────────────────────────────────────────┘
```

**Embedding Strategy:**

```rust
// blue-ollama crate
pub struct EmbeddedOllama {
    process: Option<Child>,
    port: u16,
    models_dir: PathBuf,
}

impl EmbeddedOllama {
    /// Start embedded Ollama server
    pub async fn start(&mut self) -> Result<()> {
        // Ollama binary bundled in Blue release
        let ollama_bin = Self::bundled_binary_path();

        self.process = Some(
            Command::new(ollama_bin)
                .env("OLLAMA_MODELS", &self.models_dir)
                .env("OLLAMA_HOST", format!("127.0.0.1:{}", self.port))
                .spawn()?
        );

        self.wait_for_ready().await
    }

    /// Stop embedded server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut proc) = self.process.take() {
            proc.kill()?;
        }
        Ok(())
    }
}
```

### 3. Backend Priority (CUDA > MPS > CPU)

**Ollama handles this automatically.** Ollama detects GPU at runtime:

| Platform | Backend | Detection |
|----------|---------|-----------|
| NVIDIA GPU | CUDA | Auto-detected via driver |
| Apple Silicon | **Metal (MPS)** | Auto-detected on M1/M2/M3/M4 |
| AMD GPU | ROCm | Auto-detected on Linux |
| No GPU | CPU | Fallback |

```bash
# Ollama auto-detects best backend
ollama run qwen2.5:7b  # Uses CUDA → Metal → ROCm → CPU
```

**Apple Silicon (M1/M2/M3/M4):**
- Ollama uses Metal Performance Shaders (MPS) automatically
- No configuration needed - just works
- Full GPU acceleration on unified memory

**Blue just starts Ollama and lets it choose:**

```rust
impl EmbeddedOllama {
    pub async fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(Self::bundled_binary_path());

        // Force specific backend if configured
        match self.config.backend {
            BackendChoice::Cuda => {
                cmd.env("CUDA_VISIBLE_DEVICES", "0");
                cmd.env("OLLAMA_NO_METAL", "1");  // Prefer CUDA over Metal
            }
            BackendChoice::Mps => {
                // Metal/MPS on Apple Silicon (default on macOS)
                cmd.env("CUDA_VISIBLE_DEVICES", "");  // Disable CUDA
            }
            BackendChoice::Cpu => {
                cmd.env("CUDA_VISIBLE_DEVICES", "");  // Disable CUDA
                cmd.env("OLLAMA_NO_METAL", "1");      // Disable Metal/MPS
            }
            BackendChoice::Auto => {
                // Let Ollama decide: CUDA → MPS → ROCm → CPU
            }
        }

        self.process = Some(cmd.spawn()?);
        self.wait_for_ready().await
    }
}
```

**Backend verification:**

```rust
impl EmbeddedOllama {
    pub async fn detected_backend(&self) -> Result<String> {
        // Query Ollama for what it's using
        let resp = self.client.get("/api/version").await?;
        // Returns: {"version": "0.5.1", "gpu": "cuda"} or "metal" or "cpu"
        Ok(resp.gpu)
    }
}
```

### 4. Configuration

**Default: API (easier setup)**

New users get API by default - just set an env var:

```bash
export ANTHROPIC_API_KEY=sk-...
# That's it. Blue works.
```

**Opt-in: Local (better privacy/cost)**

```bash
blue_model_download name="qwen2.5-7b"
# Edit .blue/config.yaml to prefer local
```

**Full Configuration:**

```yaml
# .blue/config.yaml
llm:
  provider: auto  # auto | local | api | none

  # auto (default): Use local if model exists, else API, else keywords
  # local: Only use local, fail if unavailable
  # api: Only use API, fail if unavailable
  # none: Disable AI features entirely

  local:
    model: qwen2.5-7b  # Shorthand, resolves to full path
    # Or explicit: model_path: ~/.blue/models/qwen2.5-7b-instruct-q4_k_m.gguf
    backend: auto  # cuda | mps | cpu | auto
    context_size: 8192
    threads: 8  # for CPU backend

  api:
    provider: anthropic  # anthropic | openai
    model: claude-3-haiku-20240307
    api_key_env: ANTHROPIC_API_KEY  # Read from env var
```

**Zero-Config Experience:**

| User State | Behavior |
|------------|----------|
| No config, no env var | Keywords only (works offline) |
| `ANTHROPIC_API_KEY` set | API (easiest) |
| Model downloaded | Local (best) |
| Both available | Local preferred |

### 5. Model Management (via Embedded Ollama)

Blue wraps Ollama's model commands:

```
blue_model_list          # ollama list
blue_model_pull          # ollama pull
blue_model_remove        # ollama rm
blue_model_info          # ollama show
```

Model storage: `~/.ollama/models/` (Ollama default, shared with external Ollama)

**Recommended Models:**

| Model | Size | Use Case |
|-------|------|----------|
| `qwen2.5:7b` | ~4.4GB | Fast, good quality |
| `qwen2.5:32b` | ~19GB | Best quality |
| `qwen2.5-coder:7b` | ~4.4GB | Code-focused |
| `qwen2.5-coder:32b` | ~19GB | Best for agentic coding |

**Pull Example:**

```
blue_model_pull name="qwen2.5:7b"

→ Pulling qwen2.5:7b...
→ [████████████████████] 100% (4.4 GB)
→ Model ready. Run: blue_model_info name="qwen2.5:7b"
```

**Licensing:** Qwen2.5 models are Apache 2.0 - commercial use permitted.

### 5.1 Goose Integration

Blue's embedded Ollama serves Goose for agentic coding:

```
┌─────────────────────────────────────────────────────────┐
│  User runs: goose                                        │
│       ↓                                                  │
│  Goose connects to localhost:11434 (Blue's Ollama)      │
│       ↓                                                  │
│  Uses same model Blue uses for semantic tasks           │
└─────────────────────────────────────────────────────────┘
```

**Setup:**

```bash
# 1. Start Blue (starts embedded Ollama)
blue daemon start

# 2. Configure Goose to use Blue's Ollama
# ~/.config/goose/config.yaml
provider: ollama
model: qwen2.5-coder:32b
host: http://localhost:11434

# 3. Run Goose with Blue's MCP tools
goose --extension "blue mcp"
```

**Convenience command:**

```bash
# Start Goose with Blue pre-configured
blue agent

# Equivalent to:
# 1. Ensure Blue daemon running (Ollama ready)
# 2. Launch Goose with Blue extension
# 3. Model auto-pulled if missing
```

**Shared Model Benefits:**

| Without Blue | With Blue |
|--------------|-----------|
| Install Ollama separately | Blue bundles Ollama |
| Configure Goose manually | `blue agent` just works |
| Model loaded twice (Ollama + Goose) | One model instance |
| 40GB RAM for two 32B models | 20GB for shared model |

### 6. Graceful Degradation

```rust
impl BlueState {
    pub async fn get_llm(&self) -> Option<&dyn LlmProvider> {
        // Try local first
        if let Some(local) = &self.local_llm {
            if local.is_ready() {
                return Some(local);
            }
        }

        // Fall back to API
        if let Some(api) = &self.api_llm {
            return Some(api);
        }

        // No LLM available
        None
    }
}
```

| Condition | Behavior |
|-----------|----------|
| Local model loaded | Use local (default) |
| Local unavailable, API configured | Fall back to API + warning |
| Neither available | Keyword matching only |
| `--no-ai` flag | Skip AI entirely |

### 7. Model Loading Strategy

**Problem:** Model load takes 5-10 seconds. Can't block MCP calls.

**Solution:** Daemon preloads model on startup.

```rust
impl EmbeddedOllama {
    pub async fn warmup(&self, model: &str) -> Result<()> {
        // Send a dummy request to load model into memory
        let resp = self.client
            .post("/api/generate")
            .json(&json!({
                "model": model,
                "prompt": "Hi",
                "options": { "num_predict": 1 }
            }))
            .send()
            .await?;

        // Model now loaded and warm
        Ok(())
    }
}
```

**Daemon Startup:**

```bash
blue daemon start

→ Starting embedded Ollama...
→ Ollama ready on localhost:11434
→ Warming up qwen2.5:7b... (5-10 seconds)
→ Model ready.
```

**MCP Tool Response During Load:**

```json
{
  "status": "loading",
  "message": "Model loading... Try again in a few seconds.",
  "retry_after_ms": 2000
}
```

**Auto-Warmup:** Daemon warms up configured model on start. First MCP request is fast.

**Manual Warmup:**

```
blue_model_warmup model="qwen2.5:32b"  # Load specific model
```

### 8. Multi-Session Model Handling

**Question:** What if user has multiple Blue MCP sessions (multiple IDE windows)?

**Answer:** All sessions share one Ollama instance via `blue daemon`.

```
┌─────────────────────────────────────────────────────────┐
│  blue daemon (singleton)                                 │
│  └── Embedded Ollama (localhost:11434)                  │
│      └── Model loaded once (~20GB for 32B)              │
├─────────────────────────────────────────────────────────┤
│  Blue MCP Session 1  ──┐                                │
│  Blue MCP Session 2  ──┼──→ HTTP to localhost:11434    │
│  Goose              ──┘                                │
└─────────────────────────────────────────────────────────┘
```

**Benefits:**
- One model in memory, not per-session
- Goose shares same model instance
- Daemon manages Ollama lifecycle
- Sessions can come and go

**Daemon Lifecycle:**

```bash
blue daemon start   # Start Ollama, keep running
blue daemon stop    # Stop Ollama
blue daemon status  # Check health and GPU info

# Auto-start: first MCP connection starts daemon if not running
```

**Status Output:**

```
$ blue daemon status

Blue Daemon: running
├── Ollama: healthy (v0.5.4)
├── Backend: Metal (MPS) - Apple M4 Max
├── Port: 11434
├── Models loaded: qwen2.5:32b (19GB)
├── Uptime: 2h 34m
└── Requests served: 1,247
```

### Daemon Health & Recovery

**Health checks:**

```rust
impl EmbeddedOllama {
    pub async fn health_check(&self) -> Result<HealthStatus> {
        match self.client.get("/api/version").await {
            Ok(resp) => Ok(HealthStatus::Healthy {
                version: resp.version,
                gpu: resp.gpu,
            }),
            Err(e) => Ok(HealthStatus::Unhealthy { error: e.to_string() }),
        }
    }

    pub fn start_health_monitor(&self) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;

                if let Ok(HealthStatus::Unhealthy { .. }) = self.health_check().await {
                    log::warn!("Ollama unhealthy, attempting restart...");
                    self.restart().await;
                }
            }
        });
    }
}
```

**Crash recovery:**

| Scenario | Behavior |
|----------|----------|
| Ollama crashes | Auto-restart within 5 seconds |
| Restart fails 3x | Mark as failed, fall back to API |
| User calls `daemon restart` | Force restart, reset failure count |

**Graceful shutdown:**

```rust
impl EmbeddedOllama {
    pub async fn stop(&mut self) -> Result<()> {
        // Signal Ollama to finish current requests
        self.client.post("/api/shutdown").await.ok();

        // Wait up to 10 seconds for graceful shutdown
        tokio::time::timeout(
            Duration::from_secs(10),
            self.wait_for_exit()
        ).await.ok();

        // Force kill if still running
        if let Some(proc) = self.process.take() {
            proc.kill().ok();
        }

        Ok(())
    }
}
```

### 8. Integration Points

**ADR Relevance (RFC 0004):**
```rust
pub async fn find_relevant_adrs(
    llm: &dyn LlmProvider,
    context: &str,
    adrs: &[AdrSummary],
) -> Result<Vec<RelevanceResult>> {
    let prompt = format_relevance_prompt(context, adrs);
    let response = llm.complete(&prompt, &RELEVANCE_OPTIONS).await?;
    parse_relevance_response(&response)
}
```

**Runbook Matching (RFC 0002):**
```rust
pub async fn match_action_semantic(
    llm: &dyn LlmProvider,
    query: &str,
    actions: &[String],
) -> Result<Option<String>> {
    // Use LLM to find best semantic match
}
```

### 9. Cargo Features & Build

```toml
[features]
default = ["ollama"]
ollama = []  # Embeds Ollama binary

[dependencies]
reqwest = { version = "0.12", features = ["json"] }  # Ollama HTTP client
tokio = { version = "1", features = ["process"] }     # Process management

[build-dependencies]
# Download Ollama binary at build time
```

**Build Process:**

```rust
// build.rs
const OLLAMA_VERSION: &str = "0.5.4";

fn main() {
    let target = std::env::var("TARGET").unwrap();

    let (ollama_url, sha256) = match target.as_str() {
        // macOS (Universal - works on Intel and Apple Silicon)
        t if t.contains("darwin") =>
            (format!("https://github.com/ollama/ollama/releases/download/v{}/ollama-darwin", OLLAMA_VERSION),
             "abc123..."),

        // Linux x86_64
        t if t.contains("x86_64") && t.contains("linux") =>
            (format!("https://github.com/ollama/ollama/releases/download/v{}/ollama-linux-amd64", OLLAMA_VERSION),
             "def456..."),

        // Linux ARM64 (Raspberry Pi 4/5, AWS Graviton, etc.)
        t if t.contains("aarch64") && t.contains("linux") =>
            (format!("https://github.com/ollama/ollama/releases/download/v{}/ollama-linux-arm64", OLLAMA_VERSION),
             "ghi789..."),

        // Windows x86_64
        t if t.contains("windows") =>
            (format!("https://github.com/ollama/ollama/releases/download/v{}/ollama-windows-amd64.exe", OLLAMA_VERSION),
             "jkl012..."),

        _ => panic!("Unsupported target: {}", target),
    };

    download_and_verify(&ollama_url, sha256);
    println!("cargo:rerun-if-changed=build.rs");
}
```

**Supported Platforms:**

| Platform | Architecture | Ollama Binary |
|----------|--------------|---------------|
| macOS | x86_64 + ARM64 | ollama-darwin (universal) |
| Linux | x86_64 | ollama-linux-amd64 |
| Linux | ARM64 | ollama-linux-arm64 |
| Windows | x86_64 | ollama-windows-amd64.exe |

**ARM64 Linux Use Cases:**
- Raspberry Pi 4/5 (8GB+ recommended)
- AWS Graviton instances
- NVIDIA Jetson
- Apple Silicon Linux VMs

**Binary Size:**

| Component | Size |
|-----------|------|
| Blue CLI | ~5 MB |
| Ollama binary | ~50 MB |
| **Total** | ~55 MB |

Models downloaded separately on first use.

### 10. Performance Expectations

**Apple Silicon (M4 Max, 128GB, Metal/MPS):**

| Metric | Qwen2.5-7B | Qwen2.5-32B |
|--------|------------|-------------|
| Model load | 2-3 sec | 5-10 sec |
| Prompt processing | ~150 tok/s | ~100 tok/s |
| Generation | ~80 tok/s | ~50 tok/s |
| ADR relevance | 100-200ms | 200-400ms |

**NVIDIA GPU (RTX 4090, CUDA):**

| Metric | Qwen2.5-7B | Qwen2.5-32B |
|--------|------------|-------------|
| Model load | 1-2 sec | 3-5 sec |
| Prompt processing | ~200 tok/s | ~120 tok/s |
| Generation | ~100 tok/s | ~60 tok/s |
| ADR relevance | 80-150ms | 150-300ms |

**CPU Only (fallback):**

| Metric | Qwen2.5-7B | Qwen2.5-32B |
|--------|------------|-------------|
| Generation | ~10 tok/s | ~3 tok/s |
| ADR relevance | 1-2 sec | 5-10 sec |

Metal/MPS on Apple Silicon is first-class - not a fallback.

### 11. Memory Validation

Ollama handles memory management, but Blue validates before pull:

```rust
impl EmbeddedOllama {
    pub async fn validate_can_pull(&self, model: &str) -> Result<()> {
        let model_size = self.get_model_size(model).await?;
        let available = sys_info::mem_info()?.avail * 1024;
        let buffer = model_size / 5;  // 20% buffer

        if available < model_size + buffer {
            return Err(LlmError::InsufficientMemory {
                model: model.to_string(),
                required: model_size + buffer,
                available,
                suggestion: format!(
                    "Close some applications or use a smaller model. \
                     Try: blue_model_pull name=\"qwen2.5:7b\""
                ),
            });
        }
        Ok(())
    }
}
```

**Ollama's Own Handling:**

Ollama gracefully handles memory pressure by unloading models. Blue's validation is advisory.

### 12. Build Requirements

**Blue Build (all platforms):**
```bash
# Just Rust toolchain
cargo build --release
```

Blue's build.rs downloads the pre-built Ollama binary for the target platform. No C++ compiler needed.

**Runtime GPU Support:**

Ollama bundles GPU support. User just needs drivers:

**macOS (Metal):**
- Works out of box on Apple Silicon (M1/M2/M3/M4)
- No additional setup needed

**Linux (CUDA):**
```bash
# NVIDIA drivers (CUDA Toolkit not needed for inference)
nvidia-smi  # Verify driver installed
```

**Linux (ROCm):**
```bash
# AMD GPU support
rocminfo  # Verify ROCm installed
```

**Windows:**
- NVIDIA: Just need GPU drivers
- Works on CPU if no GPU

**Ollama handles everything else** - users don't need to install CUDA Toolkit, cuDNN, etc.

## Security Considerations

1. **Ollama binary integrity**: Verify SHA256 of bundled Ollama binary at build time
2. **Model provenance**: Ollama registry handles model verification
3. **Local only by default**: Ollama binds to localhost:11434, not exposed
4. **Prompt injection**: Sanitize user input before prompts
5. **Memory**: Ollama handles memory management
6. **No secrets in prompts**: ADR relevance only sends context strings
7. **Process isolation**: Ollama runs as subprocess, not linked

**Network Binding:**

```rust
impl EmbeddedOllama {
    pub async fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(Self::bundled_binary_path());

        // Bind to localhost only - not accessible from network
        cmd.env("OLLAMA_HOST", "127.0.0.1:11434");

        // ...
    }
}
```

**Goose Access:**

Goose connects to `localhost:11434` - works because it's on the same machine. Remote access requires explicit `OLLAMA_HOST=0.0.0.0:11434` override.

### Port Conflict Handling

**Scenario:** User already has Ollama running on port 11434.

```rust
impl EmbeddedOllama {
    pub async fn start(&mut self) -> Result<()> {
        // Check if port 11434 is in use
        if Self::port_in_use(11434) {
            // Check if it's Ollama
            if Self::is_ollama_running().await? {
                // Use existing Ollama instance
                self.mode = OllamaMode::External;
                return Ok(());
            } else {
                // Something else on port - use alternate
                self.port = Self::find_free_port(11435..11500)?;
            }
        }

        // Start embedded Ollama on chosen port
        self.start_embedded().await
    }
}
```

| Situation | Behavior |
|-----------|----------|
| Port 11434 free | Start embedded Ollama |
| Ollama already running | Use existing (no duplicate) |
| Other service on port | Use alternate port (11435+) |

**Config override:**

```yaml
# .blue/config.yaml
llm:
  local:
    ollama_port: 11500  # Force specific port
    use_external: true   # Never start embedded, use existing
```

### Binary Verification

**Build-time verification:**

```rust
// build.rs
const OLLAMA_SHA256: &str = "abc123...";  // Per-platform hashes

fn download_ollama() {
    let bytes = download(OLLAMA_URL)?;
    let hash = sha256(&bytes);

    if hash != OLLAMA_SHA256 {
        panic!("Ollama binary hash mismatch! Expected {}, got {}", OLLAMA_SHA256, hash);
    }

    write_binary(bytes)?;
}
```

**Runtime verification:**

```rust
impl EmbeddedOllama {
    fn verify_binary(&self) -> Result<()> {
        let expected = include_str!("ollama.sha256");
        let actual = sha256_file(Self::bundled_binary_path())?;

        if actual != expected {
            return Err(LlmError::BinaryTampered {
                expected: expected.to_string(),
                actual,
            });
        }
        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        self.verify_binary()?;  // Check before every start
        // ...
    }
}
```

### Air-Gapped Builds

For environments without internet during build:

```bash
# 1. Download Ollama binary manually
curl -L https://github.com/ollama/ollama/releases/download/v0.5.4/ollama-darwin \
  -o vendor/ollama-darwin

# 2. Build with BLUE_OLLAMA_PATH
BLUE_OLLAMA_PATH=vendor/ollama-darwin cargo build --release
```

```rust
// build.rs
fn get_ollama_binary() -> Vec<u8> {
    if let Ok(path) = std::env::var("BLUE_OLLAMA_PATH") {
        // Use pre-downloaded binary
        std::fs::read(path).expect("Failed to read BLUE_OLLAMA_PATH")
    } else {
        // Download from GitHub
        download_ollama()
    }
}
```

## Implementation Phases

**Phase 1: Embedded Ollama**
1. Add build.rs to download Ollama binary per platform
2. Create `blue-ollama` crate for embedded server management
3. Implement `EmbeddedOllama::start()` and `stop()`
4. Add `blue daemon start/stop` commands

**Phase 2: LLM Provider**
5. Add `LlmProvider` trait to blue-core
6. Implement `OllamaLlm` using HTTP client
7. Add `blue_model_pull`, `blue_model_list` tools
8. Implement auto-pull on first use

**Phase 3: Semantic Integration**
9. Integrate with ADR relevance (RFC 0004)
10. Add semantic runbook matching (RFC 0002)
11. Add fallback chain: Ollama → API → keywords

**Phase 4: Goose Integration**
12. Add `blue agent` command to launch Goose
13. Document Goose + Blue setup
14. Ship example configs

## CI/CD Matrix

Test embedded Ollama on all platforms:

```yaml
# .github/workflows/ci.yml
jobs:
  test-ollama:
    strategy:
      matrix:
        include:
          - os: macos-latest
            ollama_binary: ollama-darwin
          - os: ubuntu-latest
            ollama_binary: ollama-linux-amd64
          - os: windows-latest
            ollama_binary: ollama-windows-amd64.exe

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - name: Build Blue (downloads Ollama binary)
        run: cargo build --release

      - name: Verify Ollama binary embedded
        run: |
          # Check binary exists in expected location
          ls -la target/release/ollama*

      - name: Test daemon start/stop
        run: |
          cargo run -- daemon start
          sleep 5
          curl -s http://localhost:11434/api/version
          cargo run -- daemon stop

      - name: Test with mock model (no download)
        run: cargo test ollama::mock

  # GPU tests run on self-hosted runners
  test-gpu:
    runs-on: [self-hosted, gpu]
    steps:
      - uses: actions/checkout@v4
      - name: Test CUDA detection
        run: |
          cargo build --release
          cargo run -- daemon start
          # Verify GPU detected
          curl -s http://localhost:11434/api/version | jq .gpu
          cargo run -- daemon stop
```

**Note:** Full model integration tests run nightly (large downloads).

## Test Plan

**Embedded Ollama:**
- [ ] `blue daemon start` launches embedded Ollama
- [ ] `blue daemon stop` cleanly shuts down
- [ ] Ollama detects CUDA when available
- [ ] Ollama detects Metal on macOS
- [ ] Falls back to CPU when no GPU
- [ ] Health check returns backend type

**Model Management:**
- [ ] `blue_model_pull` downloads from Ollama registry
- [ ] `blue_model_list` shows pulled models
- [ ] `blue_model_remove` deletes model
- [ ] Auto-pull on first completion if model missing
- [ ] Progress indicator during pull

**LLM Provider:**
- [ ] `OllamaLlm::complete()` returns valid response
- [ ] Fallback chain: Ollama → API → keywords
- [ ] `--no-ai` flag skips LLM entirely
- [ ] Configuration parsing from .blue/config.yaml

**Semantic Integration:**
- [ ] ADR relevance uses embedded Ollama
- [ ] Runbook matching uses semantic search
- [ ] Response includes method used (ollama/api/keywords)

**Goose Integration:**
- [ ] `blue agent` starts Goose with Blue extension
- [ ] Goose connects to Blue's embedded Ollama
- [ ] Goose can use Blue MCP tools
- [ ] Model shared between Blue tasks and Goose

**Multi-Session:**
- [ ] Multiple Blue MCP sessions share one Ollama
- [ ] Concurrent completions handled correctly
- [ ] Daemon persists across shell sessions

**Port Conflict:**
- [ ] Detects existing Ollama on port 11434
- [ ] Uses existing Ollama instead of starting new
- [ ] Uses alternate port if non-Ollama on 11434
- [ ] `use_external: true` config works

**Health & Recovery:**
- [ ] Health check detects unhealthy Ollama
- [ ] Auto-restart on crash
- [ ] Falls back to API after 3 restart failures
- [ ] Graceful shutdown waits for requests

**Binary Verification:**
- [ ] Build fails if Ollama hash mismatch
- [ ] Runtime verification before start
- [ ] Tampered binary: clear error message
- [ ] Air-gapped build with BLUE_OLLAMA_PATH works

**CI Matrix:**
- [ ] macOS build includes darwin Ollama binary
- [ ] Linux x86_64 build includes amd64 binary
- [ ] Linux ARM64 build includes arm64 binary
- [ ] Windows build includes windows binary
- [ ] Integration tests with mock Ollama server

---

*"Right then. Let's get to it."*

— Blue
