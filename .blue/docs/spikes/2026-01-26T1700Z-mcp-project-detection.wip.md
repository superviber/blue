# Spike: MCP Project Detection

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 30 minutes |

---

## Question

Why does the Blue MCP server fail to detect the project directory, and what's the correct solution?

---

## Investigation

### Current Behavior

1. Claude Code starts Blue MCP server via `blue mcp`
2. Server starts with unknown working directory
3. First tool call without `cwd` param fails: "Blue not detected in this directory"
4. Tool call with explicit `cwd` works

### Root Cause Analysis

**Hypothesis 1: Process working directory**
- MCP servers are child processes of Claude Code
- Working directory may be Claude Code's install location, not project

**Hypothesis 2: MCP protocol provides workspace info**
- MCP spec has `roots` capability for filesystem boundaries
- Claude Code may send roots during initialize or via `roots/list`

**Hypothesis 3: Environment variables**
- Claude Code sets `CLAUDECODE=1`, `CLAUDE_CODE_SSE_PORT`, `CLAUDE_CODE_ENTRYPOINT`
- No project path variable observed

### MCP Protocol Research

Per [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25):

1. **Roots** - Servers can request roots from clients
2. **Initialize params** - May contain client capabilities but not workspace path
3. **No standard workspace field** - Unlike LSP's `rootUri`/`workspaceFolders`

### Attempted Fixes

| Attempt | Result |
|---------|--------|
| Fall back to `current_dir()` | Failed - wrong directory |
| Parse `roots` from initialize | Unknown - needs testing |
| Walk up directory tree | Implemented but untested |

### Key Finding

**The MCP protocol does not provide a standard way for servers to know the user's project directory.** This is a gap compared to LSP.

### Options Discovered

1. **Require `cwd` in every tool call** - Current workaround, verbose
2. **Walk up directory tree** - Find `.blue/` from process cwd
3. **Use MCP roots** - Request roots from client, use first root
4. **Wrapper script** - Shell wrapper that sets cwd before starting server
5. **Config file** - Store project path in `~/.blue/projects.json`

---

## Alignment Dialogue Result

6 experts, 98% convergence on **Option 1: C → B → fail**

| Round | Result |
|-------|--------|
| R1 | B (3), C (2), Combined (1) |
| R2 | Option 1 (3), Option 2 (2), Option 3 (1) |
| R3 | Option 1 unanimous |

**Consensus**: MCP roots first → Walk tree fallback → Fail with guidance

The current code already implements this. The remaining issue is that:
1. Claude Code may not send roots during initialize
2. Process cwd starts outside project tree (e.g., $HOME)

---

## Conclusion

**Root cause**: Claude Code starts MCP servers from a non-project directory and doesn't send MCP roots.

**Solution**: The C → B fallback chain is correct. Need to verify Claude Code sends roots, or ensure walk-tree can find project from wherever the process starts.

**Recommended next step**: Draft RFC to formalize the detection algorithm and add better error messaging.
