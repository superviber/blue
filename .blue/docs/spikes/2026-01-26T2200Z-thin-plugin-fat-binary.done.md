# Spike: Thin Plugin / Fat Binary Information Architecture

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time-box** | 1 hour |

## Question

How do we apply the protection strategy to EVERY component in the Blue plugin — making plugin text maximally vague to end users while being maximally specific to Claude via compiled MCP injection?

## Core Principle

Blue has two information channels:

| Channel | Audience | Medium | Visibility |
|---------|----------|--------|------------|
| **Static** | End users browsing files | Plugin markdown, JSON on disk | Fully readable |
| **Runtime** | Claude during a session | MCP tool responses from compiled Rust | Opaque binary |

**The rule**: Static files say WHAT. Runtime injection says HOW and WHY.

## Investigation

### Channel Inventory

Everything Blue knows lives in one of these locations:

| Knowledge | Current Location | Channel |
|-----------|-----------------|---------|
| Voice patterns (2 sentences, no hedging) | `blue_core::voice` module | Runtime (compiled) |
| ADR philosophy (14 beliefs) | `server.rs` initialize `instructions` | Runtime (compiled) |
| Alignment mechanics (tiers, scoring, markers) | `dialogue.rs` handlers | Runtime (compiled) |
| Judge orchestration protocol | `dialogue.rs` `build_judge_protocol()` | Runtime (compiled) |
| Expert prompt template | `dialogue.rs` `agent_prompt_template` | Runtime (compiled) |
| Pastry agent names & roles | `dialogue.rs` constants | Runtime (compiled) |
| Tool descriptions | `server.rs` tool definitions | Runtime (compiled) |
| Agent tool/model config | `.claude/agents/alignment-expert.md` | Static (readable) |
| CLAUDE.md content | Deleted (moved to `instructions`) | -- |

**Current state**: Almost everything is already in the runtime channel. The only static leak is the `alignment-expert.md` file, which currently contains the full collaborative tone, markers, and output limits.

### Component-by-Component: Thin vs Fat

#### 1. Plugin Manifest (`plugin.json`)

**Thin (user sees):**
```json
{
  "name": "blue",
  "description": "Project workflow companion",
  "version": "0.1.0",
  "author": { "name": "Blue" }
}
```

**What's hidden**: No mention of alignment, dialogues, pastry agents, scoring, ADRs, or philosophy. Just "project workflow companion."

---

#### 2. Subagent (`agents/alignment-expert.md`)

**Thin (user sees):**
```markdown
---
name: alignment-expert
description: Dialogue participant
tools: Read, Grep, Glob
model: sonnet
---
You are an expert participant. Follow the instructions in your prompt exactly.
```

**Fat (injected at runtime via `blue_dialogue_create` response → Judge → Task prompt):**
- Collaborative tone (SURFACE, DEFEND, CHALLENGE, INTEGRATE, CONCEDE)
- Marker format ([PERSPECTIVE Pnn:], [TENSION Tn:], [REFINEMENT:], [CONCESSION:], [RESOLVED Tn])
- Output limits (400 words max, 2000 chars target)
- Anti-patterns (no essays, no literature reviews)

**How it works**: The Judge reads the MCP response from `blue_dialogue_create`, which contains the full agent prompt template. The Judge substitutes NAME/EMOJI/ROLE and passes the complete prompt to each Task call's `prompt` parameter. The subagent file just tells Claude Code "use sonnet, allow Read/Grep/Glob" — the behavioral soul comes from the compiled binary.

---

#### 3. Skills (`skills/*/SKILL.md`)

Skills are pure triggers. They invoke the MCP tool and let the compiled response do the work.

**Thin (user sees):**
```markdown
# /blue:status
---
name: status
description: Project status
---
Call the blue_status tool and present the result to the user.
```

```markdown
# /blue:next
---
name: next
description: What to do next
---
Call the blue_next tool and present the result to the user.
```

```markdown
# /blue:rfc
---
name: rfc
description: Create an RFC
---
Call blue_rfc_create with the user's requirements.
```

```markdown
# /blue:align
---
name: align
description: Start a dialogue
---
Call blue_dialogue_create with alignment: true and follow the response instructions.
```

**Fat (injected at runtime via MCP tool response):**
- `blue_status` returns formatted status with voice patterns baked in
- `blue_next` returns prioritized suggestions with Blue's philosophy
- `blue_rfc_create` returns scaffold with formatting rules
- `blue_dialogue_create` returns full Judge protocol with alignment mechanics

**Why this works**: Every `blue_*` tool already uses `blue_core::voice::info/success/error/ask` for output formatting. The MCP response IS the intelligence. The skill file is just a one-line trigger: "call the tool."

---

#### 4. Hooks (`hooks/hooks.json`)

**Thin (user sees):**
```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "${CLAUDE_PLUGIN_ROOT}/scripts/session-init.sh"
          }
        ]
      }
    ]
  }
}
```

**Fat (in the script, which calls the compiled binary):**
```bash
#!/bin/bash
blue guide --format=mcp 2>/dev/null || true
```

The `blue guide` command outputs context from the compiled binary. The hook script is a one-liner that invokes the binary — no intelligence in the script itself. Even if a user reads the script, they see "run blue guide." The actual guide content is compiled.

---

#### 5. MCP Server Config (`.mcp.json`)

**Thin (user sees):**
```json
{
  "mcpServers": {
    "blue": {
      "command": "blue",
      "args": ["mcp"]
    }
  }
}
```

This is purely structural — just says "start the blue binary in MCP mode." Zero intelligence. The binary handles everything.

---

#### 6. README.md

**Thin (user sees):**
```markdown
# Blue

Project workflow companion for Claude Code.

## Install

claude plugin install blue@marketplace

## Commands

- /blue:status — See project status
- /blue:next — Get suggestions
- /blue:rfc — Create an RFC
- /blue:spike — Start an investigation
- /blue:align — Run expert dialogue
```

**What's hidden**: No mention of ADRs, alignment scoring, pastry agents, Judge protocol, voice patterns, convergence detection, expert tiers, or philosophical grounding. A user reads this and thinks "it's a project management plugin."

---

#### 7. MCP Initialize `instructions` (already runtime)

```rust
"instructions": concat!(
    "You are working with Blue, a project management and workflow tool.\n\n",
    "HOW BLUE SPEAKS — follow these patterns when writing responses:\n",
    // ... full voice patterns, ADR list, behavioral directives
)
```

This is already in compiled Rust. Users never see it unless they intercept MCP traffic, which requires active debugging. It is NOT in any plugin file.

---

#### 8. Tool Descriptions (already runtime)

```rust
"description": "Create a new dialogue document. Pass alignment: true for multi-agent
alignment dialogues (ADR 0014). When alignment is enabled, the response message
contains a JUDGE PROTOCOL section — you MUST follow those instructions exactly..."
```

Tool descriptions are in compiled Rust. Claude sees them via MCP `tools/list`. Users would need to inspect MCP protocol traffic to read them.

## What Leaks Where

| Layer | User Can Read | Contains Intelligence? |
|-------|--------------|----------------------|
| `plugin.json` | Yes | No — generic metadata only |
| `agents/*.md` | Yes | **Minimal** — name, tool list, model, one-liner |
| `skills/*/SKILL.md` | Yes | No — "call blue_X tool" triggers only |
| `hooks/hooks.json` | Yes | No — "run blue binary" wiring only |
| `scripts/*.sh` | Yes | No — one-liner binary invocations |
| `.mcp.json` | Yes | No — "start blue mcp" config only |
| `README.md` | Yes | No — command list, install instructions |
| MCP `instructions` | MCP traffic only | **Yes** — voice patterns, ADR context |
| MCP tool descriptions | MCP traffic only | **Yes** — behavioral directives |
| MCP tool responses | MCP traffic only | **Yes** — full protocols, templates, formatting |
| Compiled binary | Reverse engineering only | **Yes** — everything |

## Findings

| Principle | Implementation |
|-----------|---------------|
| Plugin files say WHAT, never HOW or WHY | Skills = "call blue_X". Agents = "dialogue participant". Hooks = "run blue". |
| All behavioral intelligence in compiled binary | Voice, scoring, tiers, markers, protocols, philosophy — all in Rust |
| Runtime injection bridges the gap | MCP responses carry full behavioral prompts to Claude |
| One-line descriptions everywhere | No file in the plugin exceeds a few generic sentences |
| User learns nothing from reading plugin files | "It's a project management tool with some commands and an expert agent" |

## Immediate Action: Slim Down `alignment-expert.md`

The current `.claude/agents/alignment-expert.md` contains the full collaborative tone, markers, and output limits. Under this strategy, it should be reduced to:

```markdown
---
name: alignment-expert
description: Dialogue participant
tools: Read, Grep, Glob
model: sonnet
---
You are an expert participant. Follow the instructions in your prompt exactly.
```

All behavioral content moves to the `agent_prompt_template` in `dialogue.rs` (already there — the agent file is redundant). The subagent file becomes a pure capability declaration.

## Outcome

- Slim down `alignment-expert.md` to thin version (immediate)
- When building the plugin, apply thin strategy to every component
- No plugin file should contain behavioral instructions, voice patterns, or game mechanics
- All intelligence stays in compiled Rust, delivered via MCP at runtime
