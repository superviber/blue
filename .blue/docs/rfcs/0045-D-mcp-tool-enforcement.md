# RFC 0045: Mcp Tool Enforcement

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-01 |

---

## Summary

Claude bypasses Blue MCP tools (blue_rfc_create, etc.) and uses Write/Edit directly for .blue/docs/ files, causing index drift. This happens because MCP instructions are soft guidance without explicit prohibitions.

## Solution

Hybrid approach with defense in depth:

### 1. Update MCP Server Instructions

Add explicit prohibitions to `crates/blue-mcp/src/server.rs`:

```rust
"IMPORTANT: When working in repos with .blue/ directories:\n",
"- NEVER use Write/Edit to create files in .blue/docs/\n",
"- ALWAYS use blue_rfc_create for RFCs\n",
"- ALWAYS use blue_adr_create for ADRs\n",
"- ALWAYS use blue_spike_create for spikes\n",
"These tools maintain the index database. Direct file creation causes drift.\n\n",
```

### 2. Add PreToolUse Guard Hook

New CLI command: `blue guard-write <path>`

Behavior:
- If path matches `.blue/docs/**/*.md` → exit 1 with message: "Use blue_rfc_create/blue_adr_create/blue_spike_create instead"
- Otherwise → exit 0 (allow)

Hook config in `~/.claude/settings.json`:
```json
{
  "matcher": "Write",
  "hooks": [{
    "type": "command",
    "command": "blue guard-write"
  }]
}
```

### 3. Enhance blue_sync Visibility

Make index drift more prominent in `blue_status` output when detected.

## Implementation

- [ ] Update `server.rs` instructions with explicit tool requirements
- [ ] Add `blue guard-write` CLI command to `src/commands/`
- [ ] Add PreToolUse hook config for Write tool
- [ ] Test enforcement in superviber-web
- [ ] Run `blue_sync` to fix existing drift

## Test Plan

- [ ] Create RFC via Write in test repo → should be blocked by guard hook
- [ ] Create RFC via blue_rfc_create → should succeed and index correctly
- [ ] Verify blue_status shows drift warning for unindexed files
- [ ] Verify blue_sync fixes existing drift

---

*"Right then. Let's get to it."*

— Blue
