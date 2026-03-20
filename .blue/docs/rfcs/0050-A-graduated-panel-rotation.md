# RFC 0050: Graduated Panel Rotation

| | |
|---|---|
| **Status** | Approved |
| **Date** | 2026-02-01 |
| **ADRs** | 0014 (Alignment Dialogue Agents) |
| **Extends** | RFC 0048 (Alignment Expert Pools) |

---

## Summary

The current alignment dialogue system samples a fixed panel from the expert pool for Round 0 and uses **the same panel for all rounds**. This wastes the larger pool and misses opportunities for fresh perspectives. This RFC introduces **graduated panel rotation**: the Judge evolves the panel each round based on dialogue dynamics, with freedom to retain high performers, bring in fresh perspectives, and even create new experts to address emerging tensions.

## Problem

In the NVIDIA Investment Decision dialogue:
- **Pool size**: 22 experts across Core/Adjacent/Wildcard tiers
- **Panel size**: 12 experts
- **Actual behavior**: Same 12 experts for all 3 rounds
- **Expected behavior**: Panel evolves based on dialogue needs

The dialogue converged with contributions from Strudel (Automotive Tech Analyst) and Brioche (Options Strategist). But 10 experts in the pool **never participated**: Value Analyst, Data Center Specialist, Supply Chain Analyst, ESG Analyst, Quant Strategist, Behavioral Finance Expert, Energy Sector Analyst, Retail Investor Advocate, Regulatory Expert, and Gaming Industry Analyst.

Worse: when a tension emerged around regulatory risk, there was no mechanism to pull in the Regulatory Expert specifically to address it.

## Design

### Judge-Driven Panel Evolution

Instead of algorithmic rotation with fixed parameters, the **Judge decides** how to evolve the panel each round. The MCP server provides infrastructure; the Judge provides judgment.

### Rotation Mode: `graduated` (Default)

```json
{
  "rotation": "graduated"
}
```

This is now the **default** rotation mode. No `rotation_config` needed. The Judge receives guidelines in the skill prompt.

### Judge Guidelines (in alignment-play skill)

The skill prompt instructs the Judge on panel evolution principles:

```markdown
## Panel Evolution Guidelines

Between rounds, you decide how to evolve the panel. Consider:

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
1. Check if the pool has a relevant expert → pull them in
2. If not, **create a new expert** with the needed focus

Example: Tension T03 raises supply chain concentration risk, but no Supply Chain
Analyst is on the panel. Either pull from pool or create:
```json
{ "role": "Supply Chain Analyst", "tier": "adjacent", "focus": "Geographic concentration, single-source risk" }
```

### Panel Size Flexibility
- Target panel size is a guideline, not a constraint
- You may run a smaller panel if the dialogue is converging
- You may expand briefly to address a complex tension

### Expert Creation
You are not limited to the initial pool. If the dialogue surfaces a perspective
that no pooled expert covers, create one. The pool was your starting point,
not your ceiling.
```

### MCP Server Role

The server provides:
1. **Panel tracking**: Record which experts participated in which rounds
2. **Context briefs**: Generate summaries for fresh experts joining mid-dialogue
3. **Expert registry**: Accept new experts created by the Judge
4. **History persistence**: Store panel evolution for post-hoc analysis

The server does **not**:
- Decide which experts to retain
- Calculate overlap ratios
- Enforce tier-based rules

### API

#### `blue_dialogue_round_prompt`

When the Judge requests the next round, they specify the panel:

```json
{
  "round": 1,
  "panel": [
    { "name": "Muffin", "role": "Value Analyst", "retained": true },
    { "name": "Scone", "role": "Data Center Specialist", "source": "pool" },
    { "name": "Palmier", "role": "Supply Chain Risk Analyst", "source": "created", "focus": "Geographic concentration" }
  ]
}
```

The server:
- Validates expert names are unique
- Generates context briefs for non-retained experts
- Records the panel composition
- Returns prompts for each expert

#### Response

```json
{
  "round": 1,
  "panel_size": 12,
  "retained": 6,
  "from_pool": 5,
  "created": 1,
  "context_brief": "## Round 0 Summary\n...",
  "expert_prompts": [...]
}
```

### Persistence

```
{output_dir}/
├── expert-pool.json          # Initial pool (Judge's starting point)
├── round-0/
│   └── panel.json            # { "experts": [...] }
├── round-1/
│   └── panel.json            # { "experts": [...], "retained": [...], "fresh": [...], "created": [...] }
└── round-2/
    └── panel.json
```

## Dialogue Continuity

Fresh experts (from pool or created) receive a context brief:

```markdown
## Context for Round 1

You are joining this dialogue in Round 1. Here's what happened:

### Key Tensions Raised (Round 0)
- T01: Growth mandate vs. valuation discipline
- T02: Hedging income vs. conviction allocation

### Current Panel Position (Round 0)
- 10 experts: Don't Add
- 1 expert (Brioche): Options Reframe
- 1 expert (Strudel): Automotive Differentiation

### Your Task
Review these positions and contribute your perspective as {role}.
```

## Example: 3-Round Dialogue with Targeted Injection

**Initial Pool**: 22 experts
**Round 0 Panel**: 12 sampled experts

```
Round 0:
├── Panel deliberates
├── Tension T03 emerges: "What about Taiwan concentration risk?"
└── No Supply Chain expert on panel

Judge decision for Round 1:
├── Retain: 7 experts (high scorers + tension advocates)
├── Rotate out: 5 experts (low contribution)
├── Pull from pool: 4 experts including Supply Chain Analyst
├── Create: 1 new expert "Geopolitical Risk Analyst" (not in original pool)
└── New panel size: 12

Round 1:
├── Supply Chain Analyst addresses T03 directly
├── Geopolitical Risk Analyst adds Taiwan Strait context
├── T03 marked [RESOLVED] with synthesis
└── New tension T04 emerges around AI chip export controls

Judge decision for Round 2:
├── Retain: 8 experts (T04 is complex, needs continuity)
├── Pull from pool: 2 experts
├── Create: "Export Control Specialist" for T04
└── Smaller panel: 11 (dialogue converging)
```

**Result**:
- 18 of 22 pool experts participated
- 2 experts created on-demand
- All tensions addressed by relevant expertise

## Comparison to Current Modes

| Aspect | `none` | `wildcards` | `full` | `graduated` (new) |
|--------|--------|-------------|--------|-------------------|
| Pool utilization | ~50% | ~65% | 100% | High (Judge discretion) |
| Dialogue continuity | High | High | Low | High (retained experts) |
| Fresh perspectives | None | Some | All | As needed |
| Targeted expertise | No | No | No | **Yes** |
| Expert creation | No | No | No | **Yes** |
| Configurable | No | No | No | Via guidelines |

## Implementation

### Changes to `dialogue.rs`

1. Accept `panel` specification in `round_prompt` request
2. Track expert sources: `retained`, `pool`, `created`
3. Generate context briefs for non-retained experts
4. Persist panel history per round

### Changes to `alignment-play` skill

Add Judge guidelines for panel evolution (see above).

### No New Config Structs

The Judge's judgment replaces configuration. The server just records what the Judge decides.

## Test Plan

- [ ] Judge can specify panel composition in round prompt
- [ ] Fresh experts receive context briefs
- [ ] Created experts are registered and tracked
- [ ] Panel history persists across rounds
- [ ] Backward compatibility: `rotation: "none"` still works

## Philosophy

> "The Judge sees the elephant. The Judge summons the right blind men. And when a new part of the elephant emerges, the Judge can summon someone who wasn't in the original room."

The pool is a starting point, not a constraint. The Judge's job is to ensure every relevant perspective touches the elephant. Sometimes that means pulling from the pool. Sometimes that means creating a new expert on the spot.

This is ALIGNMENT by design: **responsive expertise** rather than **fixed sampling**.

---

*"The elephant is larger than we thought. Let me get someone who knows about tusks."*

— The Judge

