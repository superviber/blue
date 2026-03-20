# RFC 0062: Batch Dialogue Round Prompts

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-26 |
| **Relates To** | RFC 0048 (Expert Pools), RFC 0050 (Graduated Panel Rotation), RFC 0057/0061 (CLI-MCP Parity), ADR 0014 (Alignment Dialogue Agents) |

---

## Summary

Add a batch `round_prompts` handler that accepts an array of agents and returns all prompts in a single call. Implement once as a shared handler in `blue-mcp/handlers/dialogue.rs`, expose through both MCP tool and CLI subcommand.

## Problem Statement

### Current Flow (Serial)

```
Judge calls blue_dialogue_round_prompt("Muffin", ...)   → wait → response
Judge calls blue_dialogue_round_prompt("Cupcake", ...)  → wait → response
Judge calls blue_dialogue_round_prompt("Scone", ...)    → wait → response
...repeat 12 times...
Judge spawns 12 Task agents in parallel ← this part is already parallel
```

Each MCP tool call in Claude Code is a synchronous round-trip. For a 12-agent panel, that's 12 sequential calls before any expert work begins. The `handle_round_prompt` function is stateless and idempotent — each call reads the same shared context files (tensions.md, prior round summary) and substitutes agent-specific parameters. There is zero dependency between calls.

### Proposed Flow (Batch)

```
Judge calls round_prompts([Muffin, Cupcake, Scone, ...12 agents])  → one call → 12 prompts
Judge spawns 12 Task agents in parallel
```

One call replaces twelve. Works identically whether invoked via MCP or CLI.

## Design

### Implement Once, Expose Twice

Following the RFC 0057 shared handler pattern, the batch logic lives in the handler layer. Both MCP and CLI are thin wrappers that parse their respective inputs and call the same function.

```
┌──────────────┐     ┌──────────────┐
│  MCP Server  │     │   CLI App    │
│              │     │              │
│ tool call:   │     │ blue dialogue│
│ round_prompts│     │ round-prompts│
└──────┬───────┘     └──────┬───────┘
       │  &Value             │ clap args → json!({...})
       └──────────┬──────────┘
                  ▼
  ┌─────────────────────────────────┐
  │ handlers::dialogue::            │
  │ handle_round_prompts(args)      │
  │                                 │
  │ • parse agents array            │
  │ • generate_context_brief once   │
  │ • build_agent_prompt × N        │
  │ • return all prompts            │
  └─────────────────────────────────┘
```

The handler is stateless (no `ProjectState` needed) — it reads files and builds strings. This matches the existing `handle_round_prompt` signature.

### Refactor: Extract `build_agent_prompt`

The existing `handle_round_prompt` contains ~200 lines of prompt construction logic. Extract the core into `build_agent_prompt()` so both singular and batch endpoints share it:

```rust
/// Core prompt builder — shared by singular and batch endpoints.
fn build_agent_prompt(
    output_dir: &str,
    round: usize,
    agent_name: &str,
    agent_emoji: &str,
    agent_role: &str,
    sources: &[String],
    expert_source: Option<ExpertSource>,
    focus: Option<&str>,
    context_brief: Option<&str>,  // pre-computed, passed in
) -> Result<Value, ServerError> {
    // ... existing prompt construction logic, unchanged
}

/// Singular endpoint (backward compat)
pub fn handle_round_prompt(args: &Value) -> Result<Value, ServerError> {
    // parse single agent params from args
    let context_brief = if round > 0 {
        Some(generate_context_brief(output_dir, round)?)
    } else { None };
    build_agent_prompt(output_dir, round, name, emoji, role, &sources,
                       expert_source, focus, context_brief.as_deref())
}

/// Batch endpoint (new)
pub fn handle_round_prompts(args: &Value) -> Result<Value, ServerError> {
    let output_dir = get_required_str(args, "output_dir")?;
    let round = get_required_u64(args, "round")? as usize;
    let agents = get_required_array(args, "agents")?;
    let sources = get_optional_array_str(args, "sources");

    // Context brief generated ONCE, reused across all fresh experts
    let context_brief = if round > 0 {
        Some(generate_context_brief(output_dir, round)?)
    } else { None };

    let mut prompts = Vec::with_capacity(agents.len());
    for agent in agents {
        let name = agent.get("name").and_then(|v| v.as_str()).ok_or(...)?;
        let emoji = agent.get("emoji").and_then(|v| v.as_str()).ok_or(...)?;
        let role = agent.get("role").and_then(|v| v.as_str()).ok_or(...)?;
        let expert_src = agent.get("expert_source")...;
        let focus = agent.get("focus").and_then(|v| v.as_str());

        prompts.push(build_agent_prompt(
            output_dir, round, name, emoji, role, &sources,
            expert_src, focus, context_brief.as_deref()
        )?);
    }

    Ok(json!({
        "status": "success",
        "round": round,
        "agent_count": prompts.len(),
        "prompts": prompts
    }))
}
```

### MCP Tool Registration

```json
{
  "name": "blue_dialogue_round_prompts",
  "description": "Get fully-substituted prompts for ALL agents in a round in a single call. Batch version of blue_dialogue_round_prompt.",
  "inputSchema": {
    "type": "object",
    "required": ["output_dir", "round", "agents"],
    "properties": {
      "output_dir": { "type": "string" },
      "round": { "type": "integer" },
      "agents": {
        "type": "array",
        "items": {
          "type": "object",
          "required": ["name", "emoji", "role"],
          "properties": {
            "name": { "type": "string" },
            "emoji": { "type": "string" },
            "role": { "type": "string" },
            "expert_source": { "type": "string", "enum": ["retained", "pool", "created"] },
            "focus": { "type": "string" }
          }
        }
      },
      "sources": { "type": "array", "items": { "type": "string" } }
    }
  }
}
```

### CLI Subcommand

```rust
// In DialogueCommands enum:
/// Get prompts for all agents in a round (batch)
RoundPrompts {
    /// Dialogue output directory
    #[arg(long)]
    output_dir: String,

    /// Round number
    #[arg(long)]
    round: u64,

    /// Agents as JSON array: [{"name":"Muffin","emoji":"🧁","role":"..."},...]
    #[arg(long)]
    agents: String,

    /// Optional source files (comma-separated)
    #[arg(long)]
    sources: Option<String>,
}

// Handler:
DialogueCommands::RoundPrompts { output_dir, round, agents, sources } => {
    let agents_val: Value = serde_json::from_str(&agents)?;
    let args = json!({
        "output_dir": output_dir,
        "round": round,
        "agents": agents_val,
        "sources": sources.map(|s| s.split(',').collect::<Vec<_>>()),
    });
    match blue_mcp::handlers::dialogue::handle_round_prompts(&args) {
        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Response Format

```json
{
  "status": "success",
  "round": 0,
  "agent_count": 12,
  "prompts": [
    {
      "agent_name": "Muffin",
      "prompt": "You are Muffin ...",
      "output_file": ".blue/dialogues/.../round-0/muffin.md",
      "prompt_file": ".blue/dialogues/.../round-0/muffin.prompt.md",
      "task_params": {
        "subagent_type": "general-purpose",
        "description": "Muffin expert deliberation"
      },
      "expert_source": "pool",
      "has_context_brief": false
    }
  ]
}
```

### Backward Compatibility

The singular `blue_dialogue_round_prompt` remains unchanged. The batch endpoint is additive. The alignment-play SKILL.md will be updated to prefer the batch endpoint.

### SKILL.md Update

Replace the current "get prompts" step:

```diff
- For EACH agent, call blue_dialogue_round_prompt to get the fully-substituted prompt
+ Call blue_dialogue_round_prompts ONCE with all agents to get all prompts in a single call
```

## Test Plan

- [ ] Unit test: batch with 1 agent matches singular endpoint output
- [ ] Unit test: batch with 12 agents returns 12 prompts with correct agent names
- [ ] Unit test: context brief generated once, not N times (round > 0)
- [ ] Unit test: CLI subcommand parses JSON agents correctly
- [ ] Integration: MCP tool registered and dispatches to handler
- [ ] Integration: CLI `blue dialogue round-prompts` produces same output as MCP
- [ ] Integration: SKILL.md updated to use batch endpoint
- [ ] Integration: full dialogue round using batch endpoint

---

*"Right then. Let's get to it."*

— Blue
