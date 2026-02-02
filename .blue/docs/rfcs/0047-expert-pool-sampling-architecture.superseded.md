# RFC 0047: Expert Pool Sampling Architecture

| | |
|---|---|
| **Status** | Superseded |
| **Superseded By** | RFC 0048 (Alignment Expert Pools) |
| **Date** | 2026-02-01 |
| **ADRs** | 0014 (Alignment Dialogue Agents) |
| **Extends** | RFC 0046 (Judge Defined Expert Panels) |

---

## Summary

Extend the alignment dialogue system to support **larger expert pools** from which panels are sampled. Currently, `blue_dialogue_create(agents=12)` creates exactly 12 fixed agents. This RFC introduces a two-phase architecture: (1) Judge defines a domain-appropriate pool of 15-30 experts, (2) MCP server samples N experts from this pool, with optional per-round rotation.

## Problem

RFC 0046 addresses the issue of inappropriate auto-selected roles. However, it still creates a **fixed panel** of exactly N agents who participate in all rounds. This misses opportunities:

1. **Perspective diversity**: A larger pool enables rotation, bringing fresh perspectives each round
2. **Stochastic exploration**: Weighted random sampling may surface unexpected insights
3. **Tiered expertise**: Core experts can be retained while Wildcards rotate
4. **Demonstrable reasoning**: Users see the Judge's domain analysis reflected in pool design

Current architecture:
```
blue_dialogue_create(agents=12, expert_panel=[...12 roles...])
  → Creates 12 fixed agents
  → Same 12 participate in all rounds
```

Proposed architecture:
```
blue_dialogue_create(expert_pool=[...24 roles...], panel_size=12, rotation="wildcards")
  → Creates 24-expert domain pool
  → Samples 12 for Round 0
  → Rotates Wildcards each round (retains Core/Adjacent)
```

## Design

### Expert Pool Structure

The Judge creates a pool with tiered relevance:

```json
{
  "expert_pool": {
    "domain": "Investment Analysis",
    "question": "Should Acme Trust add NVIDIA by trimming NVAI?",
    "experts": [
      { "role": "Value Analyst", "tier": "Core", "relevance": 0.95 },
      { "role": "Growth Analyst", "tier": "Core", "relevance": 0.90 },
      { "role": "Risk Manager", "tier": "Core", "relevance": 0.85 },
      { "role": "Portfolio Strategist", "tier": "Core", "relevance": 0.80 },
      { "role": "ESG Analyst", "tier": "Adjacent", "relevance": 0.70 },
      { "role": "Quant Strategist", "tier": "Adjacent", "relevance": 0.65 },
      { "role": "Technical Analyst", "tier": "Adjacent", "relevance": 0.60 },
      { "role": "Behavioral Analyst", "tier": "Adjacent", "relevance": 0.55 },
      { "role": "Income Analyst", "tier": "Adjacent", "relevance": 0.50 },
      { "role": "Macro Economist", "tier": "Wildcard", "relevance": 0.40 },
      { "role": "Credit Analyst", "tier": "Wildcard", "relevance": 0.35 },
      { "role": "Contrarian", "tier": "Wildcard", "relevance": 0.30 },
      { "role": "Geopolitical Analyst", "tier": "Wildcard", "relevance": 0.25 },
      { "role": "Market Historian", "tier": "Wildcard", "relevance": 0.22 },
      { "role": "Options Strategist", "tier": "Wildcard", "relevance": 0.20 }
    ]
  },
  "panel_size": 12,
  "rotation": "wildcards"
}
```

### Tier Distribution

For a pool of P experts with panel size N:

| Tier | Pool % | Panel % | Purpose |
|------|--------|---------|---------|
| **Core** | ~25% | ~33% | Domain essentials, always selected |
| **Adjacent** | ~40% | ~42% | Related expertise, high selection probability |
| **Wildcard** | ~35% | ~25% | Fresh perspectives, rotation candidates |

### Sampling Algorithm

```rust
/// Sample N experts from pool for a round
fn sample_panel(pool: &ExpertPool, panel_size: usize, round: usize, rotation: RotationMode) -> Vec<PastryAgent> {
    let (core_n, adj_n, wc_n) = tier_split(panel_size);

    match rotation {
        RotationMode::None => {
            // Round 0 selection persists all rounds (current behavior)
            if round == 0 {
                weighted_sample(&pool.core, core_n)
                    .chain(weighted_sample(&pool.adjacent, adj_n))
                    .chain(weighted_sample(&pool.wildcard, wc_n))
            } else {
                // Return same panel as round 0
                load_round_0_panel()
            }
        }
        RotationMode::Wildcards => {
            // Core/Adjacent persist, Wildcards resample each round
            let core = if round == 0 { weighted_sample(&pool.core, core_n) } else { load_core_panel() };
            let adjacent = if round == 0 { weighted_sample(&pool.adjacent, adj_n) } else { load_adjacent_panel() };
            let wc_remaining = pool.wildcard.iter()
                .filter(|e| !used_wildcards.contains(&e.role))
                .collect();
            let wildcards = weighted_sample(&wc_remaining, wc_n);

            core.chain(adjacent).chain(wildcards)
        }
        RotationMode::Full => {
            // Complete resample each round (respects relevance weights)
            weighted_sample(&pool.all, panel_size)
        }
    }
}

/// Weighted random sampling without replacement
fn weighted_sample(experts: &[Expert], n: usize) -> Vec<Expert> {
    let total_weight: f64 = experts.iter().map(|e| e.relevance).sum();
    let probs: Vec<f64> = experts.iter().map(|e| e.relevance / total_weight).collect();

    // Reservoir sampling with weights
    weighted_reservoir_sample(experts, probs, n)
}
```

### API Changes

#### `blue_dialogue_create` Extended Parameters

```json
{
  "title": "NVIDIA Investment Decision",
  "alignment": true,
  "expert_pool": {
    "domain": "Investment Analysis",
    "experts": [
      { "role": "Value Analyst", "tier": "Core", "relevance": 0.95, "focus": "Intrinsic value, margin of safety" },
      // ... 15-30 experts
    ]
  },
  "panel_size": 12,
  "rotation": "wildcards"  // "none" | "wildcards" | "full"
}
```

#### Backward Compatibility

- `expert_panel` (RFC 0046) still works → creates fixed panel, no pool
- `expert_pool` (this RFC) → creates pool with sampling

```rust
if let Some(pool) = args.get("expert_pool") {
    // New pool-based architecture
    create_with_pool(pool, panel_size, rotation)
} else if let Some(panel) = args.get("expert_panel") {
    // RFC 0046 behavior - fixed panel
    create_with_fixed_panel(panel)
} else {
    // Error - alignment requires either pool or panel
    Err(ServerError::InvalidParams)
}
```

### New Tool: `blue_dialogue_sample_panel`

For manual round-by-round control:

```json
{
  "name": "blue_dialogue_sample_panel",
  "description": "Sample a new panel from the expert pool for the next round",
  "params": {
    "dialogue_id": "nvidia-investment-decision",
    "round": 1,
    "retain_experts": ["muffin", "cupcake", "scone"],  // Optional: keep specific experts
    "exclude_experts": ["beignet"]  // Optional: exclude specific experts
  }
}
```

### Pool Persistence

Expert pools are stored per-dialogue:

```
{output_dir}/
├── expert-pool.json      ← Full pool definition (Judge writes)
├── round-0/
│   ├── panel.json        ← Sampled panel for this round
│   └── *.md              ← Agent responses
├── round-1/
│   ├── panel.json        ← May differ if rotation enabled
│   └── *.md
└── scoreboard.md
```

### Judge Workflow

1. **Analyze problem**: Read RFC/topic, identify required expertise domains
2. **Design pool**: Create 15-30 experts across Core/Adjacent/Wildcard tiers
3. **Create dialogue**: Call `blue_dialogue_create` with `expert_pool`
4. **Run rounds**: MCP server handles sampling automatically
5. **Review selections**: Pool and panel visible in output files

## ADR 0014 Amendment

Add to ADR 0014:

```markdown
### Expert Pools (RFC 0047)

The Judge may create a **larger expert pool** from which panels are sampled:

| Concept | Description |
|---------|-------------|
| **Pool** | 15-30 domain-appropriate experts defined by Judge |
| **Panel** | N experts sampled from pool for a given round |
| **Sampling** | Weighted random selection respecting relevance scores |
| **Rotation** | Optional: Wildcards may rotate between rounds |

Pool design is a Judge responsibility. The Judge understands the problem domain after reading the RFC/topic and designs experts accordingly.

**Tier Distribution**:
- **Core** (~25% of pool, 33% of panel): Essential domain experts, always selected
- **Adjacent** (~40% of pool, 42% of panel): Related expertise, high probability
- **Wildcard** (~35% of pool, 25% of panel): Fresh perspectives, rotation candidates

**Rotation Modes**:
- `none`: Fixed panel (current behavior)
- `wildcards`: Core/Adjacent persist, Wildcards resample each round
- `full`: Complete resample each round (experimental)
```

## Skill Update: alignment-play

```markdown
## Phase 1: Pool Design

Before creating the dialogue, the Judge:
1. Reads the topic/RFC thoroughly
2. Identifies the **domain** (e.g., "Investment Analysis", "System Architecture")
3. Designs **15-30 experts** appropriate to the domain:
   - **Core (4-8)**: Essential perspectives for this specific problem
   - **Adjacent (6-12)**: Related expertise that adds depth
   - **Wildcard (5-10)**: Fresh perspectives, contrarians, cross-domain insight
4. Assigns **relevance scores** (0.20-0.95) based on expected contribution
5. Calls `blue_dialogue_create` with the `expert_pool`

## Phase 2: Round Execution

The MCP server:
1. Samples `panel_size` experts from pool using weighted random selection
2. Higher relevance = higher selection probability
3. Core experts almost always selected; Wildcards provide variety
4. If rotation enabled, Wildcards resample each round

## Phase 3: Convergence

Same as current: velocity → 0 or tensions resolved
```

## Implementation

### Changes to `dialogue.rs`

```rust
/// Expert pool with tiered structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertPool {
    pub domain: String,
    pub experts: Vec<PoolExpert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolExpert {
    pub role: String,
    pub tier: ExpertTier,
    pub relevance: f64,
    pub focus: Option<String>,
    pub bias: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpertTier {
    Core,
    Adjacent,
    Wildcard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationMode {
    None,       // Fixed panel all rounds
    Wildcards,  // Core/Adjacent fixed, Wildcards rotate
    Full,       // Complete resample each round
}

/// Handle blue_dialogue_create with pool support
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    // ... existing validation ...

    if let Some(pool_json) = args.get("expert_pool") {
        let pool: ExpertPool = serde_json::from_value(pool_json.clone())?;
        let panel_size = args.get("panel_size").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
        let rotation: RotationMode = args.get("rotation")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "wildcards" => RotationMode::Wildcards,
                "full" => RotationMode::Full,
                _ => RotationMode::None,
            })
            .unwrap_or(RotationMode::None);

        // Sample initial panel
        let agents = sample_panel_from_pool(&pool, panel_size);

        // Persist pool to output directory
        let pool_path = format!("{}/expert-pool.json", output_dir);
        fs::write(&pool_path, serde_json::to_string_pretty(&pool)?)?;

        // ... rest of dialogue creation ...
    }
}
```

### New Handler: `handle_sample_panel`

```rust
pub fn handle_sample_panel(state: &ProjectState, args: &Value) -> Result<Value, ServerError> {
    let dialogue_id = args.get("dialogue_id").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;
    let round = args.get("round").and_then(|v| v.as_u64())
        .ok_or(ServerError::InvalidParams)? as usize;

    // Load pool from dialogue directory
    let pool_path = format!("/tmp/blue-dialogue/{}/expert-pool.json", dialogue_id);
    let pool: ExpertPool = serde_json::from_str(&fs::read_to_string(&pool_path)?)?;

    // Parse retain/exclude lists
    let retain: Vec<String> = args.get("retain_experts")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    // Sample new panel
    let panel = sample_panel_with_constraints(&pool, 12, round, &retain);

    // Persist panel for this round
    let panel_path = format!("/tmp/blue-dialogue/{}/round-{}/panel.json", dialogue_id, round);
    fs::write(&panel_path, serde_json::to_string_pretty(&panel)?)?;

    Ok(json!({
        "status": "success",
        "round": round,
        "panel": panel,
    }))
}
```

## Test Plan

- [ ] `blue_dialogue_create` with `expert_pool` creates pool file
- [ ] Initial panel respects tier distribution (33/42/25)
- [ ] Weighted sampling: higher relevance = higher selection probability
- [ ] `rotation: "wildcards"` keeps Core/Adjacent, rotates Wildcards
- [ ] `rotation: "none"` uses same panel all rounds
- [ ] `blue_dialogue_sample_panel` respects `retain_experts`
- [ ] Pool persists across rounds in output directory
- [ ] Backward compatibility: `expert_panel` (RFC 0046) still works

## Visualization (for demo)

The demo page can show:

```
┌─────────────────────────────────────────────────────────────┐
│  INVESTMENT EXPERT POOL                            24 total │
├─────────────────────────────────────────────────────────────┤
│  CORE (6)          ████████████████████  rel: 0.80-0.95     │
│  ✓ Value Analyst   ✓ Growth Analyst   ✓ Risk Manager        │
│  ✓ Portfolio Strat ○ Fundamental      ○ Tax Specialist      │
├─────────────────────────────────────────────────────────────┤
│  ADJACENT (10)     ████████████████      rel: 0.50-0.70     │
│  ✓ ESG Analyst     ✓ Quant Strategist ✓ Technical Analyst   │
│  ✓ Behavioral      ✓ Income Analyst   ○ Credit Analyst      │
│  ○ Governance      ○ Competitive      ○ Regulatory          │
│  ○ Momentum Trader                                          │
├─────────────────────────────────────────────────────────────┤
│  WILDCARD (8)      ████████              rel: 0.20-0.40     │
│  ✓ Macro Economist ✓ Contrarian       ✓ Geopolitical        │
│  ○ Market Historian ○ Options Strat   ○ Ethicist            │
│  ○ Retail Sentiment ○ Academic                              │
└─────────────────────────────────────────────────────────────┘
  ✓ = Selected for Round 0    ○ = Available in pool

  [🎲 Resample Panel] ← Click to see stochastic selection
```

---

## Philosophy

> "The Judge sees the elephant. The Judge summons the right blind men."

The alignment dialogue system embodies the parable of the blind men and the elephant. Each expert touches a different part. Wisdom emerges from integration.

With expert pools:
- The Judge **designs** the population of potential perspectives
- The MCP server **samples** fairly from that population
- Rotation **refreshes** the conversation with new viewpoints
- The final verdict reflects **multiple samplings** of the elephant

This is ALIGNMENT by design: more blind men, more parts touched, more wisdom integrated.

---

*Blue*
