# RFC 0072: Remove Blue MCP Server

| | |
|---|---|
| **Status** | Approved |
| **Date** | 2026-03-16 |
| **Story** | — |
| **Deprecates** | RFC 0005, RFC 0020, RFC 0041 |
| **Depends On** | RFC 0071 (Remove Embedded LLM) |

---

## Summary

Remove the Blue MCP server entirely. Claude Code will invoke `blue` CLI commands via Bash instead of MCP tool calls. The MCP transport layer is 6,000+ lines of glue code wrapping the same handlers the CLI already uses. Removing it cuts build complexity, binary size, and maintenance surface with no loss of capability.

## Problem

1. **Redundant transport.** The MCP server (`crates/blue-mcp/`) and CLI (`apps/blue-cli/`) both dispatch to the same 27 handler modules. The MCP layer adds JSON-RPC 2.0 framing, tool registration, schema generation, and dispatch logic — all of which duplicates what Clap already does for the CLI.

2. **95 tools pollute Claude Code's context.** Every MCP tool definition is injected into Claude's system prompt. 95 tools × ~50 tokens each ≈ 4,750 tokens of tool schemas loaded on every conversation. This crowds out user context and slows tool selection. CLI commands are discovered on-demand via `blue --help`.

3. **MCP adds latency.** Each MCP call requires JSON-RPC serialization, stdin/stdout pipe communication, and MCP protocol overhead. A direct `blue status` via Bash is faster and produces human-readable output.

4. **Maintenance burden.** Every new feature requires: (a) handler implementation, (b) MCP tool registration with schema, (c) CLI command with Clap derive, (d) dispatch arm in `server.rs`. Removing MCP cuts this to two steps: handler + CLI command.

5. **Installation complexity.** `blue install` writes MCP server config to `~/.claude.json`, copies skills, and manages hooks. Without MCP, installation simplifies to ensuring `blue` is on PATH and skills reference CLI commands.

6. **Debugging opacity.** MCP tool calls are harder to debug than CLI commands. Users can't easily reproduce MCP interactions, but they can copy-paste CLI commands.

## What Gets Removed

### Crate: `blue-mcp` (MCP transport layer only)

| Component | Lines | Purpose |
|-----------|-------|---------|
| `crates/blue-mcp/src/lib.rs` | ~100 | MCP server entry point, JSON-RPC loop |
| `crates/blue-mcp/src/server.rs` | ~2,900 | Tool registration, schema definitions, dispatch |
| `crates/blue-mcp/src/types.rs` | ~200 | MCP protocol types (JsonRpcRequest, etc.) |
| `crates/blue-mcp/src/resources.rs` | ~580 | MCP resource definitions |

**Total MCP glue removed:** ~3,780 lines

### Handler modules: relocated, not deleted

The 27 handler modules in `crates/blue-mcp/src/handlers/` (~17,000 lines) contain the actual business logic. These move to `blue-core` or a new `blue-handlers` crate. No logic is lost.

### CLI changes

| File | Change |
|------|--------|
| `apps/blue-cli/src/main.rs` | Remove `blue mcp` subcommand (~20 lines) |
| `apps/blue-cli/src/main.rs` | Add CLI commands for any MCP-only features worth keeping |

### Installation changes

| File | Change |
|------|--------|
| `blue install` | Stop writing MCP server config to `~/.claude.json` |
| `blue install` | Update skills to reference `blue <command>` instead of MCP tools |
| `~/.claude.json` | Remove `mcpServers.blue` entry |

### Skills/Prompts

All `.claude/skills/` files that reference MCP tool names (`blue_status`, `blue_rfc_create`, etc.) must be updated to use CLI invocations (`blue status`, `blue rfc create`, etc.).

## MCP-Only Features That Need CLI Commands

These features exist only as MCP tools today and need CLI equivalents before removal:

| MCP Tool | Proposed CLI Command | Priority |
|----------|---------------------|----------|
| `blue_dialogue_round_context` | `blue dialogue round-context` | High (alignment dialogues) |
| `blue_dialogue_expert_create` | `blue dialogue expert create` | High |
| `blue_dialogue_round_register` | `blue dialogue round register` | High |
| `blue_dialogue_verdict_register` | `blue dialogue verdict register` | High |
| `blue_dialogue_export` | `blue dialogue export` | Medium |
| `blue_dialogue_evolve_panel` | `blue dialogue panel evolve` | Medium |
| `blue_dialogue_sample_panel` | `blue dialogue panel sample` | Medium |
| `blue_dialogue_lint` | `blue dialogue lint` | Medium |
| `blue_playwright_verify` | Drop — can be handled by skills | Low |
| `blue_env_detect` | `blue env detect` | Low |
| `blue_env_mock` | `blue env mock` | Low |
| `blue_health_check` | Merge into `blue doctor` | Low |
| `blue_guide` | `blue guide` | Low |
| `blue_index_*` (5 tools) | Already have `blue index` CLI commands | Already done |
| `blue_postmortem_*` (2 tools) | `blue postmortem create`, `blue postmortem action-to-rfc` | Medium |
| `blue_release_create` | `blue release create` | Medium |
| `blue_staging_*` (7 tools) | `blue staging *` | Medium |
| `blue_contract_get` | `blue contract get` | Low |
| `blue_decision_create` | `blue decision create` | Medium |
| `blue_runbook_*` (4 tools) | `blue runbook *` | Medium |
| `blue_notifications_list` | `blue notifications list` | Low |

## What Does NOT Change

- All business logic in handler modules (relocated, not rewritten)
- Blue's document model, ADR system, realm system, alignment dialogues
- Claude Code skills (updated to use CLI commands)
- Hook system (already uses CLI: `blue session-heartbeat`, `blue guard`)
- `blue init`, `blue doctor`, `blue install` (simplified)

## How Claude Code Uses Blue After This

**Before (MCP):**
```
Claude Code → MCP JSON-RPC → blue mcp subprocess → handler → JSON response → Claude Code
```

**After (CLI):**
```
Claude Code → Bash tool → blue <command> → handler → stdout → Claude Code
```

Skills guide Claude to use the right `blue` commands. Example skill snippet:

```markdown
# Before (MCP)
Use `blue_rfc_create` with title and content parameters.

# After (CLI)
Run: `blue rfc create --title "..." --content "..."`
```

CLI output is already structured (JSON by default, human-readable with `--format text`). Claude Code parses it the same way it parses any command output.

## Implementation Phases

### Phase 1: Add missing CLI commands

Add CLI equivalents for all MCP-only features listed above. Since handlers already exist, each CLI command is just a Clap struct + dispatch to existing handler. Estimated: ~500 lines of new CLI code.

### Phase 2: Update skills and prompts

Update all `.claude/skills/` files to reference CLI commands instead of MCP tools. Update `CLAUDE.md` and any documentation referencing MCP tool names.

### Phase 3: Relocate handlers

Move `crates/blue-mcp/src/handlers/` to `crates/blue-core/src/handlers/` (or a new `blue-handlers` crate). Update imports in CLI. This is a mechanical move — no logic changes.

### Phase 4: Remove MCP transport

- Delete MCP server entry point, dispatch, schema generation, protocol types
- Remove `blue mcp` subcommand
- Remove MCP server config from `blue install`
- Remove `blue-mcp` from workspace (or repurpose as thin re-export during transition)

### Phase 5: Simplify installation

- `blue install` no longer writes to `~/.claude.json` MCP config
- `blue install` ensures `blue` binary is on PATH
- `blue install` copies/updates skills only
- `blue uninstall` removes skills and hooks (no MCP config to clean up)

### Phase 6: Clean up

- Remove MCP-related dependencies (`serde_json` MCP types, JSON-RPC helpers)
- Run full test suite
- Update README, INSTALL docs
- Mark RFC 0020 (MCP Project Detection) as deprecated

## Open Questions

- [x] ~~Should CLI output default to JSON (machine-readable) or text (human-readable)?~~ **JSON when piped, text when interactive.** Claude Code always gets JSON.
- [x] ~~Should we keep `blue-mcp` as a crate name for the handlers, or rename to `blue-handlers`?~~ **Rename to `blue-handlers`.**
- [x] ~~Should RFC 0020 (MCP Project Detection) be deprecated or superseded?~~ **Deprecated.** Project detection logic stays but moves to CLI; MCP delivery mechanism goes away.
- [x] ~~How do we handle the transition for users who have `blue` in their `~/.claude.json` MCP config?~~ **`blue install` detects and removes stale MCP config.**

## Risks

- **Skill coverage gap.** If skills don't adequately guide Claude to the right CLI commands, usage quality drops. Mitigation: comprehensive skill updates in Phase 2 before removing MCP.
- **Output format mismatch.** MCP returns structured JSON by contract. CLI output format needs to be reliable enough for Claude to parse. Mitigation: `--format json` flag on all commands.
- **Bash tool permissions.** Users with restrictive Claude Code permission settings may need to approve more Bash calls. MCP tools were auto-approved. Mitigation: document recommended permission settings.

## Test Plan

- [ ] All existing handler tests pass after relocation
- [ ] Every former MCP tool has a working CLI equivalent
- [ ] Skills reference CLI commands and produce correct results
- [ ] `blue install` no longer writes MCP config
- [ ] `blue install` on a system with old MCP config cleans it up
- [ ] `blue doctor` reports healthy state without MCP
- [ ] Binary size decreases
- [ ] Build time decreases (no MCP schema generation)
- [ ] Claude Code can complete a full workflow (create RFC, create worktree, work, deliver) using only CLI commands

---

*"Fewer moving parts. Same kitchen."*

— Blue
