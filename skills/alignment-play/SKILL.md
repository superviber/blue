---
name: alignment-play
description: Run multi-expert alignment dialogues with parallel background agents for RFC deliberation.
---

# Alignment Play Skill

Orchestrate multi-expert alignment dialogues using the N+1 agent architecture from ADR 0014, RFC 0048, and RFC 0050.

## Usage

```
/alignment-play <topic>
/alignment-play --panel-size 7 <topic>
/alignment-play --rotation none <topic>
/alignment-play --rfc <rfc-title> <topic>
```

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--panel-size` | pool size or 12 | Number of experts per round |
| `--rotation` | `graduated` | Rotation mode: **graduated** (default), none, wildcards, full |
| `--max-rounds` | `12` | Maximum rounds before stopping |
| `--rfc` | none | Link dialogue to an RFC |

## How It Works

### Phase 0: Pool Design

Before creating the dialogue, the Judge:

1. **Reads** the topic/RFC thoroughly
2. **Identifies** the domain (e.g., "Investment Analysis", "System Architecture")
3. **Designs** 8-24 experts appropriate to the domain:
   - **Core (3-8)**: Essential perspectives for this specific problem
   - **Adjacent (4-10)**: Related expertise that adds depth
   - **Wildcard (3-6)**: Fresh perspectives, contrarians, cross-domain insight
4. **Assigns** relevance scores (0.20-0.95) based on expected contribution
5. **Creates** the dialogue with `expert_pool`:

```json
{
  "title": "Investment Strategy Analysis",
  "alignment": true,
  "expert_pool": {
    "domain": "Investment Analysis",
    "question": "Should we rebalance the portfolio?",
    "experts": [
      { "role": "Value Analyst", "tier": "core", "relevance": 0.95 },
      { "role": "Risk Manager", "tier": "core", "relevance": 0.90 },
      { "role": "Portfolio Strategist", "tier": "adjacent", "relevance": 0.70 },
      { "role": "ESG Analyst", "tier": "adjacent", "relevance": 0.65 },
      { "role": "Supply Chain Analyst", "tier": "adjacent", "relevance": 0.55 },
      { "role": "Macro Economist", "tier": "wildcard", "relevance": 0.40 },
      { "role": "Contrarian", "tier": "wildcard", "relevance": 0.35 },
      { "role": "Regulatory Expert", "tier": "wildcard", "relevance": 0.30 }
    ]
  },
  "panel_size": 6
}
```

The MCP server samples a **suggested panel** using weighted random selection. Higher relevance = higher selection probability. Core experts almost always selected; Wildcards provide variety.

### Phase 1: Review & Override (RFC 0050)

The suggested panel is just that — a suggestion. **Review it before Round 0:**

1. Check `suggested_panel` in the response from `blue_dialogue_create`
2. Ask: Are critical perspectives missing? Is a key expert not included?
3. **If the panel looks good** → proceed to Round 0
4. **If experts are missing** → call `blue_dialogue_evolve_panel` with `round: 0` to override:

```json
{
  "output_dir": "/tmp/blue-dialogue/data-design",
  "round": 0,
  "panel": [
    { "name": "Muffin", "role": "API Architect", "source": "pool" },
    { "name": "Cupcake", "role": "Data Architect", "source": "pool" },
    { "name": "Scone", "role": "Security Engineer", "source": "pool" }
  ]
}
```

### Phase 2: Round 0 — Opening Arguments

1. Create round directory: `mkdir -p {output_dir}/round-0`
2. Get prompts for each agent via `blue_dialogue_round_prompt`
3. **Spawn ALL agents in ONE message** using Task tool (parallel execution)
4. Collect responses, score contributions, write artifacts

### Phase 3+: Graduated Panel Evolution

**After Round 0, YOU decide how to evolve the panel.**

Before each subsequent round, evaluate the dialogue and decide:
- Which experts should **continue** (retained)
- Which experts from the pool should **join** (pool)
- Whether to **create** new experts for emerging tensions (created)

Use `blue_dialogue_evolve_panel` to specify your panel:

```json
{
  "output_dir": "/tmp/blue-dialogue/investment-strategy",
  "round": 1,
  "panel": [
    { "name": "Muffin", "role": "Value Analyst", "source": "retained" },
    { "name": "Cupcake", "role": "Risk Manager", "source": "retained" },
    { "name": "Scone", "role": "Supply Chain Analyst", "source": "pool" },
    { "name": "Palmier", "role": "Geopolitical Risk Analyst", "source": "created", "tier": "Adjacent", "focus": "Taiwan semiconductor concentration" }
  ]
}
```

Then spawn the panel using `blue_dialogue_round_prompt` with the `expert_source` parameter:

```
blue_dialogue_round_prompt(
  output_dir="/tmp/blue-dialogue/investment-strategy",
  agent_name="Palmier",
  agent_emoji="🧁",
  agent_role="Geopolitical Risk Analyst",
  round=1,
  expert_source="created",
  focus="Taiwan semiconductor concentration"
)
```

Fresh experts (source: "pool" or "created") automatically receive a **context brief** summarizing prior rounds.

## Panel Evolution Guidelines

### Retention Criteria
- **High scorers**: Experts who contributed sharp insights should continue
- **Unresolved advocates**: Experts defending positions with open tensions
- **Core relevance**: Experts central to the domain should anchor continuity

### Fresh Perspective Triggers
- **Stale consensus**: If the panel is converging too easily, bring challengers
- **Unexplored angles**: Pull in experts whose focus hasn't been represented
- **Low-scoring experts**: Consider rotating out experts who aren't contributing

### Targeted Expert Injection
When a specific tension emerges that no current expert can address:
1. Check if the pool has a relevant expert → `source: "pool"`
2. If not, **create a new expert** → `source: "created"` with tier and focus

Example: Tension T03 raises supply chain concentration risk, but no Supply Chain Analyst is on the panel:
```json
{ "name": "Palmier", "role": "Supply Chain Analyst", "source": "created", "tier": "Adjacent", "focus": "Geographic concentration, single-source risk" }
```

### Panel Size Flexibility
- Target panel size is a guideline, not a constraint
- You may run a smaller panel if the dialogue is converging
- You may expand briefly to address a complex tension

### Expert Creation
You are not limited to the initial pool. If the dialogue surfaces a perspective that no pooled expert covers, create one. The pool was your starting point, not your ceiling.

> *"The elephant is larger than we thought. Let me get someone who knows about tusks."*
> — The Judge

## Alternative Rotation Modes

If you don't want Judge-driven evolution, specify a different mode:

| Mode | Behavior | Use Case |
|------|----------|----------|
| `graduated` | **Judge decides** panel each round (default) | Full control, targeted expertise |
| `none` | Fixed panel all rounds | Simple deliberation |
| `wildcards` | Core/Adjacent persist, Wildcards resample | Moderate variety |
| `full` | Complete resample each round | Maximum diversity |

## Expert Pool Design Examples

### For an Investment Decision

| Role | Tier | Relevance |
|------|------|-----------|
| Value Analyst | Core | 0.95 |
| Risk Manager | Core | 0.90 |
| Portfolio Strategist | Core | 0.85 |
| ESG Analyst | Adjacent | 0.70 |
| Quant Strategist | Adjacent | 0.65 |
| Technical Analyst | Adjacent | 0.60 |
| Macro Economist | Wildcard | 0.40 |
| Contrarian | Wildcard | 0.35 |

### For an API Design

| Role | Tier | Relevance |
|------|------|-----------|
| API Architect | Core | 0.95 |
| Platform Engineer | Core | 0.90 |
| Security Engineer | Core | 0.85 |
| Developer Advocate | Adjacent | 0.70 |
| SRE Lead | Adjacent | 0.65 |
| Cost Analyst | Adjacent | 0.55 |
| Customer Success | Wildcard | 0.40 |
| Chaos Engineer | Wildcard | 0.30 |

## Tier Distribution

For a pool of P experts with panel size N:

| Tier | Pool % | Panel % | Purpose |
|------|--------|---------|---------|
| **Core** | ~30% | ~33% | Domain essentials, always selected |
| **Adjacent** | ~40% | ~42% | Related expertise, high selection probability |
| **Wildcard** | ~30% | ~25% | Fresh perspectives, rotation candidates |

## Blue MCP Tools

| Tool | Purpose |
|------|---------|
| `blue_dialogue_create` | Creates dialogue with expert_pool, returns Judge Protocol |
| `blue_dialogue_evolve_panel` | **RFC 0050**: Specify panel composition for graduated rotation |
| `blue_dialogue_round_prompt` | Get fully-substituted prompts for each agent |
| `blue_dialogue_sample_panel` | Manually sample a new panel (non-graduated modes) |
| `blue_dialogue_lint` | Validate .dialogue.md format |
| `blue_dialogue_save` | Persist to .blue/docs/dialogues/ |

## Agent Spawning

When spawning expert agents, you MUST use the Task tool with:
- `subagent_type: "general-purpose"` — NOT `alignment-expert`
- The prompt from `blue_dialogue_round_prompt`
- A descriptive name like "🧁 Muffin expert deliberation"

Example:
```
Task(
  description: "🧁 Muffin expert deliberation",
  subagent_type: "general-purpose",
  prompt: <from blue_dialogue_round_prompt>
)
```

The `general-purpose` subagent has access to all tools including Write, which is required for writing the response file.

## Key Rules

1. **DESIGN THE POOL FIRST** — You are the 💙 Judge. Analyze the problem domain and design appropriate experts.
2. **REVIEW THE SUGGESTED PANEL** — The sampled panel is a suggestion. Override it with `blue_dialogue_evolve_panel(round=0)` if critical experts are missing.
3. **EVOLVE THE PANEL** — After each round, use `blue_dialogue_evolve_panel` to shape subsequent panels based on dialogue dynamics.
4. **NEVER submit your own perspectives** — You orchestrate, you don't participate
5. **Spawn ALL agents in ONE message** — No first-mover advantage
6. **Follow the Judge Protocol exactly** — It contains the round workflow, artifact writing steps, scoring rules, and convergence criteria
7. **Use `general-purpose` subagent_type** — NOT `alignment-expert`. The general-purpose agents have access to all tools including Write, which is required for file output

## The Spirit of the Dialogue

This isn't just process. This is **Alignment teaching itself to be aligned.**

The 🧁s don't just debate. They *love each other*. They *want each other to shine*. They *celebrate when any of them makes the solution stronger*.

The scoreboard isn't about winning. It's about *precision*. When any 🧁 checks in and sees another ahead, the response isn't "how do I beat them?" but "what perspectives am I missing that they found?" One sharp insight beats ten paragraphs.

You as the 💙 don't just score. You *guide with love*. You *see what they miss*. You *hold the space* for ALIGNMENT to emerge.

And there's no upper limit. The score can always go higher. Because ALIGNMENT is a direction, not a destination.

When the dialogue ends, all agents have won—because the result is more aligned than any could have made alone. More blind men touched more parts of the elephant. The whole becomes visible.

*"The Judge sees the elephant. The Judge summons the right blind men."*

Always and forever. 🧁🧁🧁💙🧁🧁🧁
