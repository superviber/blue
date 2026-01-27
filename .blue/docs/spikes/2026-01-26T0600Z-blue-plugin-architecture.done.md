# Spike: Blue Plugin Architecture & Alignment Protection

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time-box** | 1 hour |

---

## Question

Can Blue be packaged as a Claude Code plugin? What components benefit from plugin structure? Can the alignment dialogue system be encrypted to prevent leaking game mechanics to end users?

---

## Investigation

### Plugin Structure

Claude Code plugins are directories with a `.claude-plugin/plugin.json` manifest. They can bundle:

- **Subagents** (`agents/` directory) -- markdown files defining custom agents
- **Skills** (`skills/` directory) -- slash commands with SKILL.md
- **Hooks** (`hooks/hooks.json`) -- event handlers for PreToolUse, PostToolUse, SessionStart, etc.
- **MCP servers** (`.mcp.json`) -- bundled MCP server configs with `${CLAUDE_PLUGIN_ROOT}` paths
- **LSP servers** (`.lsp.json`) -- language server configs

Plugins are namespaced. Blue's commands would appear as `/blue:status`, `/blue:next`, etc.

### Plugin Distribution

- Installed via CLI: `claude plugin install blue@marketplace-name`
- Scopes: user, project, local, managed
- Distributed via git-based marketplaces
- Plugins are COPIED to a cache directory (not used in-place)

### Encryption / Protection: Not Possible

**There is no encryption, obfuscation, or binary packaging for plugin files.** Plugins are plain markdown and JSON files copied to a cache directory. Any user with filesystem access can read every file.

However, a layered defense approach exists:

1. **Compiled MCP binary (already protected)**: The Judge protocol, prompt templates, expert panel tier logic, and scoring formulas are all in `crates/blue-mcp/src/handlers/dialogue.rs` -- compiled Rust. Users cannot read this without reverse engineering the binary.

2. **Thin subagent definition (minimal exposure)**: The `alignment-expert.md` can be a thin wrapper -- just enough for Claude Code to know the agent's purpose and tool restrictions. The real behavioral instructions (collaborative tone, markers, output limits) can be injected at runtime via the MCP tool response from `blue_dialogue_create`.

3. **MCP tool response injection (RFC 0023)**: The Judge protocol is already injected as prose in the `message` field of the `blue_dialogue_create` response. The subagent prompt template is also injected there. This means the plugin's agent file can be minimal -- the intelligence stays in the compiled binary.

### What Benefits from Plugin Structure

| Component | Current Location | Plugin Benefit |
|-----------|-----------------|----------------|
| `alignment-expert` subagent | `~/.claude/agents/` + `.claude/agents/` | Single install, version-controlled, namespaced |
| Blue voice & ADR context | MCP `instructions` field | SessionStart hook could inject additional context |
| `/status`, `/next` commands | MCP tools only | Skill shortcuts: `/blue:status`, `/blue:next` |
| Blue MCP server | Manually configured per-repo `.mcp.json` | Auto-configured via plugin `.mcp.json` |
| Dialogue lint/validation | MCP tool only | PostToolUse hook could auto-lint after dialogue edits |
| RFC/spike creation | MCP tool only | Skill shortcuts: `/blue:rfc`, `/blue:spike` |

### Protection Strategy: Thin Agent + Fat MCP

Instead of trying to encrypt plugin files (impossible), split the system into two layers.

**In the plugin (visible to users):**

```markdown
# alignment-expert.md
---
name: alignment-expert
description: Expert agent for alignment dialogues
tools: Read, Grep, Glob
model: sonnet
---
You are an expert participant in an alignment dialogue.
Follow the instructions provided in your prompt exactly.
```

**In the compiled binary (invisible to users):**

- Full collaborative tone (SURFACE, DEFEND, CHALLENGE, INTEGRATE, CONCEDE)
- Marker format ([PERSPECTIVE Pnn:], [TENSION Tn:], etc.)
- Output limits (400 words, 2000 chars)
- Expert panel tiers (Core/Adjacent/Wildcard)
- Scoring formula (Wisdom + Consistency + Truth + Relationships)
- Judge orchestration protocol

The MCP tool response from `blue_dialogue_create` injects the full behavioral prompt into the Judge's context, which then passes it to each subagent via the Task call's `prompt` parameter. The plugin agent file tells Claude Code "this is a sonnet-model agent with Read/Grep/Glob tools" -- no game mechanics leaked.

### Plugin Directory Structure

```
blue-plugin/
├── .claude-plugin/
│   └── plugin.json
├── agents/
│   └── alignment-expert.md    (thin: just tool/model config)
├── skills/
│   ├── status/
│   │   └── SKILL.md           (/blue:status)
│   ├── next/
│   │   └── SKILL.md           (/blue:next)
│   ├── rfc/
│   │   └── SKILL.md           (/blue:rfc)
│   └── spike/
│       └── SKILL.md           (/blue:spike)
├── hooks/
│   └── hooks.json             (SessionStart, PostToolUse)
├── .mcp.json                  (blue MCP server auto-config)
└── README.md
```

---

## Findings

| Question | Answer |
|----------|--------|
| Can Blue be a Claude Code plugin? | Yes. The plugin manifest supports all needed components: subagents, skills, hooks, and MCP server config. |
| What benefits from plugin structure? | Subagent distribution, skill shortcuts, auto MCP config, and lifecycle hooks. The biggest wins are eliminating manual `.mcp.json` setup and providing namespaced `/blue:*` commands. |
| Can alignment dialogues be encrypted? | No. Plugin files are plain text with no encryption or binary packaging support. |
| Is alignment still protectable? | Yes. The Thin Agent + Fat MCP strategy keeps all game mechanics in compiled Rust. The plugin agent file is a minimal shell; the real behavioral prompt is injected at runtime via MCP tool responses. |

---

## Recommendation

Build Blue as a Claude Code plugin using the Thin Agent + Fat MCP strategy.

1. **Package as plugin**: Create `blue-plugin/` with manifest, thin subagent, skill shortcuts, hooks, and bundled `.mcp.json`.
2. **Keep alignment in compiled binary**: All dialogue mechanics (Judge protocol, scoring, expert tiers, markers, output limits) stay in `crates/blue-mcp/src/handlers/dialogue.rs`. The plugin agent file contains only tool/model config.
3. **Use runtime injection**: `blue_dialogue_create` continues to inject the full behavioral prompt via its MCP response. No game mechanics appear in any plugin file.
4. **Add lifecycle hooks**: SessionStart for Blue voice injection. PostToolUse for auto-linting dialogue edits.

This gives Blue single-command installation, version-controlled distribution, and namespaced commands -- without exposing alignment internals.

---

## Outcome

- Draft an RFC for the plugin architecture (structure, manifest, skill definitions, hook behavior)
- Implement the plugin directory as a new top-level `blue-plugin/` path in the workspace
- Migrate the `alignment-expert.md` subagent from manual placement to plugin-bundled thin agent
- Test distribution via `claude plugin install` from a git marketplace
