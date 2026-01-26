# RFC 0020: MCP Project Detection

| | |
|---|---|
| **Status** | Accepted |
| **Created** | 2026-01-26 |
| **Source** | Spike: MCP Project Detection |
| **Dialogue** | 6-expert alignment, 98% convergence |

---

## Problem

Blue MCP server fails with "Blue not detected in this directory" when:
1. Tool calls don't include explicit `cwd` parameter
2. Claude Code doesn't send MCP roots during initialize
3. Server process starts from non-project directory

This creates poor UX - users must pass `cwd` to every tool call.

## Proposal

Implement robust project detection with clear fallback chain:

```
Explicit cwd → MCP Roots → Walk Tree → Fail with Guidance
```

### Detection Algorithm

```rust
fn detect_project() -> Result<PathBuf, Error> {
    // 1. Use explicit cwd if provided in tool args
    if let Some(cwd) = self.cwd {
        return Ok(cwd);
    }

    // 2. Use MCP roots from initialize (set during handshake)
    if let Some(root) = self.mcp_root {
        return Ok(root);
    }

    // 3. Walk up from process cwd looking for .blue/
    if let Some(found) = walk_up_for_blue() {
        return Ok(found);
    }

    // 4. Fail with actionable guidance
    Err("Blue project not found. Either:
         - Pass 'cwd' parameter to tool calls
         - Run from within a Blue project directory
         - Initialize Blue with 'blue init'")
}
```

### MCP Roots Handling

During `initialize`, extract roots from client:

```rust
fn handle_initialize(params: Value) {
    // Check for MCP roots
    if let Some(roots) = params.get("roots") {
        if let Some(uri) = roots[0].get("uri") {
            self.mcp_root = Some(uri_to_path(uri));
        }
    }

    // Also check workspaceFolders (some clients use this)
    if let Some(folders) = params.get("workspaceFolders") {
        // ...
    }
}
```

### Walk Tree Implementation

```rust
fn walk_up_for_blue() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    for _ in 0..20 {  // Limit depth
        if dir.join(".blue").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
    None
}
```

### Error Messages

Clear, actionable errors:

| Scenario | Message |
|----------|---------|
| No project found | "Blue project not found. Run 'blue init' or pass 'cwd' parameter." |
| Wrong directory | "Blue not detected in: /path. Expected .blue/ directory." |

## Implementation

### Phase 1: Improve Detection (Done)

- [x] Add `find_blue_root()` walk-up function
- [x] Extract roots from initialize params into `mcp_root` field
- [x] Use fallback chain in `ensure_state()`: `cwd` → `mcp_root` → walk tree → fail
- [x] Separate `cwd` (tool-arg override) from `mcp_root` (session-level from initialize)
- [x] Unit tests: 17 tests covering construction, roots extraction, field isolation, fallback chain, and integration

### Phase 2: Verify Claude Code Behavior (Done)

- [x] Log what Claude Code sends in initialize
- [x] Confirm if roots are provided
- [x] Document client requirements

**Findings (2026-01-25, Claude Code v2.1.19):**

Claude Code does **not** send MCP roots. It declares the capability but provides no roots array:

```json
{
  "capabilities": { "roots": {} },
  "clientInfo": { "name": "claude-code", "version": "2.1.19" },
  "protocolVersion": "2025-11-25"
}
```

Detection succeeds via step 3 (walk-up from process cwd):

| Step | Source | Result |
|------|--------|--------|
| 1. `cwd` | Tool arg | None (no call yet) |
| 2. `mcp_root` | Initialize | null (not sent by client) |
| 3. Walk tree | `find_blue_root()` | Found project root |

**Implication:** Walk-up is the primary detection path for Claude Code. The `mcp_root` path exists for future clients or if Claude Code adds roots support.

### Phase 3: Improve Error Messages (Done)

- [x] Show attempted paths in error — `BlueNotDetected` now carries context (process cwd, mcp_root, attempted path)
- [x] Suggest fixes based on context — errors end with "Run 'blue init' or pass 'cwd' parameter."
- [x] Add `--debug` flag to MCP server — `blue mcp --debug` logs DEBUG-level output to `/tmp/blue-mcp-debug.log`

## ADR Alignment

| ADR | How Honored |
|-----|-------------|
| ADR 3 (Home) | "You are never lost" - detection finds home |
| ADR 5 (Single Source) | `.blue/` directory is the marker |
| ADR 8 (Honor) | Clear errors explain what happened |

## Open Questions

1. ~~Does Claude Code send MCP roots?~~ **No.** Declares `capabilities.roots: {}` but sends no roots array. (Verified 2026-01-25, v2.1.19)
2. Should we support multiple projects in one session?
3. Should detection be cached or run per-call?

## References

- [Spike: MCP Project Detection](../spikes/2026-01-26-mcp-project-detection.md)
- [MCP Specification - Roots](https://spec.modelcontextprotocol.io/specification/client/roots/)
