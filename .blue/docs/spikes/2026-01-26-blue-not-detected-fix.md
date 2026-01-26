# Spike: Blue Not Detected Fix

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

Why does `blue_rfc_get` (and other Blue MCP tools) fail with "Blue not detected in this directory" when called without explicit `cwd` parameter?

---

## Investigation

### Root Cause

1. MCP servers are started by Claude Code as child processes
2. The server's working directory is NOT the project directory
3. When `cwd` isn't passed in tool args, `ensure_state()` can't find `.blue/`

### What We Tried

1. **Fall back to `std::env::current_dir()`** - Doesn't work because the MCP server process starts from a different directory (likely Claude Code's install location or $HOME)

2. **Extract roots from initialize params** - Added code to check for `roots` and `workspaceFolders` in initialize params. Need to verify if Claude Code sends these.

### MCP Protocol Options

Per [MCP Spec](https://modelcontextprotocol.io/specification/2025-11-25):

1. **Roots capability** - Server can advertise `roots` capability, then client provides roots via `roots/list`
2. **Client info** - Initialize params may include workspace info

### Potential Solutions

#### Option A: Wrapper Script
Create a wrapper that finds the project root:
```bash
#!/bin/bash
# blue-mcp-wrapper
cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
exec blue mcp
```

MCP config:
```json
{
  "mcpServers": {
    "blue": {
      "command": "/path/to/blue-mcp-wrapper"
    }
  }
}
```

**Pros**: Simple, works immediately
**Cons**: Requires wrapper script, git dependency

#### Option B: Environment Variable
Check `CLAUDE_PROJECT_DIR` or similar env var set by Claude Code.

**Status**: Need to verify if Claude Code sets such a variable.

#### Option C: Walk Up Directory Tree
In `ensure_state()`, if cwd check fails, walk up from process cwd looking for `.blue/`:
```rust
fn find_blue_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join(".blue").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
```

**Pros**: No external dependencies
**Cons**: May find wrong project if nested

#### Option D: Use MCP Roots Protocol
Properly implement MCP roots:
1. Advertise `roots` capability in initialize response
2. Handle `roots/list` request from client
3. Store roots and use first root as project dir

**Pros**: Standard MCP approach
**Cons**: Requires Claude Code to send roots (need to verify)

### Recommendation

Try Option D first (MCP roots), then fall back to Option C (walk up tree) if Claude Code doesn't send roots.

---

## Next Steps

1. Add logging to see what Claude Code sends in initialize
2. Implement roots handling
3. Add directory tree walk as fallback
4. Test and verify

---

## Conclusion

*Pending investigation completion*
