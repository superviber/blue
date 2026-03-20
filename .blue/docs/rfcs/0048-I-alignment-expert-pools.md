# RFC 0048: Alignment Expert Pools

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-02-01 |
| **ADRs** | 0014 (Alignment Dialogue Agents) |
| **Supersedes** | RFC 0046, RFC 0047 |

---

## Summary

The alignment dialogue system uses keyword matching to auto-select generic expert roles, producing inappropriate panels for domain-specific topics. This RFC introduces **Judge-defined expert pools**: the Judge designs a tiered pool of domain-appropriate experts, and the MCP server samples panels from this pool with optional per-round rotation.

## Problem

When `blue_dialogue_create` is called for an alignment dialogue, expert roles are auto-selected via:

1. Keyword matching against topic title (e.g., "security" → "Security Architect")
2. Fallback to generic roles: Systems Thinker, Domain Expert, Devil's Advocate

This fails for:
- **Domain-specific topics**: Investment analysis gets "Systems Architect" instead of "Portfolio Manager"
- **Cross-functional topics**: A product launch might need Marketing, Legal, Finance perspectives
- **Novel domains**: Topics without keyword matches get only generic roles
- **Perspective diversity**: Fixed panels miss opportunities for rotation and fresh viewpoints

The Judge understands the problem space after reading the topic. The Judge should design the expert pool.

## Design

### Expert Pool Structure

The Judge creates a pool with three tiers:

```json
{
  "title": "NVIDIA Investment Decision",
  "alignment": true,
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
      { "role": "Contrarian", "tier": "Wildcard", "relevance": 0.35 },
      { "role": "Geopolitical Analyst", "tier": "Wildcard", "relevance": 0.30 },
      { "role": "Market Historian", "tier": "Wildcard", "relevance": 0.25 }
    ]
  },
  "panel_size": 7,
  "rotation": "wildcards"
}
```

### Tier Distribution

| Tier | Pool % | Panel % | Purpose |
|------|--------|---------|---------|
| **Core** | ~30% | ~33% | Domain essentials, always selected |
| **Adjacent** | ~40% | ~42% | Related expertise, high selection probability |
| **Wildcard** | ~30% | ~25% | Fresh perspectives, rotation candidates |

### Rotation Modes

| Mode | Behavior |
|------|----------|
| `none` | Fixed panel for all rounds (default) |
| `wildcards` | Core/Adjacent persist, Wildcards resample each round |
| `full` | Complete resample each round |

### Sampling Algorithm

```rust
fn sample_panel(pool: &ExpertPool, panel_size: usize, round: usize, rotation: RotationMode) -> Vec<PastryAgent> {
    let (core_n, adj_n, wc_n) = tier_split(panel_size);

    match rotation {
        RotationMode::None => {
            // Round 0 selection persists all rounds
            if round == 0 {
                weighted_sample(&pool.core, core_n)
                    .chain(weighted_sample(&pool.adjacent, adj_n))
                    .chain(weighted_sample(&pool.wildcard, wc_n))
            } else {
                load_round_0_panel()
            }
        }
        RotationMode::Wildcards => {
            // Core/Adjacent persist, Wildcards resample
            let core = if round == 0 { weighted_sample(&pool.core, core_n) } else { load_core() };
            let adjacent = if round == 0 { weighted_sample(&pool.adjacent, adj_n) } else { load_adjacent() };
            let unused_wc = pool.wildcard.iter().filter(|e| !used_in_previous_rounds(e));
            let wildcards = weighted_sample(&unused_wc, wc_n);
            core.chain(adjacent).chain(wildcards)
        }
        RotationMode::Full => {
            weighted_sample(&pool.all, panel_size)
        }
    }
}

fn weighted_sample(experts: &[Expert], n: usize) -> Vec<Expert> {
    // Higher relevance = higher selection probability
    let total: f64 = experts.iter().map(|e| e.relevance).sum();
    let probs: Vec<f64> = experts.iter().map(|e| e.relevance / total).collect();
    weighted_reservoir_sample(experts, probs, n)
}
```

### API

#### `blue_dialogue_create`

```json
{
  "title": "...",
  "alignment": true,
  "expert_pool": {
    "domain": "string",
    "question": "string (optional)",
    "experts": [
      { "role": "string", "tier": "Core|Adjacent|Wildcard", "relevance": 0.0-1.0 }
    ]
  },
  "panel_size": 12,
  "rotation": "none|wildcards|full"
}
```

**Required for alignment mode**: `expert_pool` with at least 3 experts.

**Validation**:
- Error if `alignment: true` but no `expert_pool`
- Error if `panel_size` > total experts in pool
- Error if relevance not in 0.0-1.0 range
- Warning if no Wildcard tier experts (groupthink risk)

#### `blue_dialogue_sample_panel` (New)

Manual round-by-round control:

```json
{
  "dialogue_title": "nvidia-investment-decision",
  "round": 1,
  "retain": ["Muffin", "Cupcake"],
  "exclude": ["Beignet"]
}
```

### Output Structure

```
{output_dir}/
├── expert-pool.json      # Full pool (Judge's design)
├── round-0/
│   ├── panel.json        # Sampled panel for this round
│   └── *.md              # Agent responses
├── round-1/
│   ├── panel.json        # May differ if rotation enabled
│   └── *.md
└── scoreboard.md
```

### Dialogue Markdown

```markdown
## Expert Pool

**Domain**: Investment Analysis
**Question**: Should Acme Trust add NVIDIA by trimming NVAI?

| Tier | Experts |
|------|---------|
| Core | Value Analyst, Growth Analyst, Risk Manager, Portfolio Strategist |
| Adjacent | ESG Analyst, Quant Strategist, Technical Analyst, Behavioral Analyst, Income Analyst |
| Wildcard | Macro Economist, Contrarian, Geopolitical Analyst, Market Historian |

## Round 0 Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 🧁 Muffin | Value Analyst | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Risk Manager | Core | 0.85 | 🧁 |
| 🧁 Scone | ESG Analyst | Adjacent | 0.70 | 🧁 |
| 🧁 Eclair | Technical Analyst | Adjacent | 0.60 | 🧁 |
| 🧁 Donut | Behavioral Analyst | Adjacent | 0.55 | 🧁 |
| 🧁 Brioche | Contrarian | Wildcard | 0.35 | 🧁 |
| 🧁 Croissant | Market Historian | Wildcard | 0.25 | 🧁 |
```

## Implementation

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertPool {
    pub domain: String,
    pub question: Option<String>,
    pub experts: Vec<PoolExpert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolExpert {
    pub role: String,
    pub tier: ExpertTier,
    pub relevance: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExpertTier {
    Core,
    Adjacent,
    Wildcard,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum RotationMode {
    #[default]
    None,
    Wildcards,
    Full,
}
```

### Changes to `dialogue.rs`

1. **Remove**: `ROLE_KEYWORDS`, `GENERAL_ROLES`, `select_role_for_topic`
2. **Add**: `ExpertPool`, `PoolExpert`, `ExpertTier`, `RotationMode` structs
3. **Add**: `sample_panel_from_pool`, `weighted_sample` functions
4. **Modify**: `handle_create` to parse `expert_pool` and require it for alignment mode
5. **Modify**: `assign_pastry_agents` to accept sampled experts instead of generating roles
6. **Add**: `handle_sample_panel` for manual round control

### handle_create Changes

```rust
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    // ... existing validation ...

    if alignment {
        let pool: ExpertPool = args.get("expert_pool")
            .ok_or_else(|| ServerError::InvalidParams("alignment requires expert_pool".into()))?
            .try_into()?;

        let panel_size = args.get("panel_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(pool.experts.len().min(12) as u64) as usize;

        let rotation: RotationMode = args.get("rotation")
            .and_then(|v| v.as_str())
            .map(|s| match s {
                "wildcards" => RotationMode::Wildcards,
                "full" => RotationMode::Full,
                _ => RotationMode::None,
            })
            .unwrap_or_default();

        // Sample initial panel
        let sampled = sample_panel_from_pool(&pool, panel_size, 0, rotation);
        let agents = assign_pastry_names(sampled);

        // Persist pool
        let pool_path = format!("{}/expert-pool.json", output_dir);
        fs::write(&pool_path, serde_json::to_string_pretty(&pool)?)?;

        // ... continue with dialogue creation ...
    }
}
```

## Judge Workflow

### Phase 0: Pool Design

Before creating the dialogue, the Judge:

1. **Reads** the topic/RFC thoroughly
2. **Identifies** the domain (e.g., "Investment Analysis", "System Architecture")
3. **Designs** 8-24 experts appropriate to the domain:
   - **Core (3-8)**: Essential perspectives for this specific problem
   - **Adjacent (4-10)**: Related expertise that adds depth
   - **Wildcard (3-6)**: Fresh perspectives, contrarians, cross-domain insight
4. **Assigns** relevance scores (0.20-0.95) based on expected contribution
5. **Creates** the dialogue with `expert_pool`

### Phase 1+: Round Execution

The MCP server:

1. Samples `panel_size` experts using weighted random selection
2. Higher relevance = higher selection probability
3. Core experts almost always selected; Wildcards provide variety
4. If rotation enabled, Wildcards resample each round

### Convergence

Same as ADR 0014: velocity → 0 for 3 consecutive rounds, or tensions resolved.

## Skill Update: alignment-play

```markdown
## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `--panel-size` | pool size or 12 | Number of experts per round |
| `--rotation` | `none` | Rotation mode: none, wildcards, full |
| `--max-rounds` | `12` | Maximum rounds before stopping |
| `--rfc` | none | Link dialogue to an RFC |

## Expert Pool Design

The Judge designs the pool before creating the dialogue:

1. Analyze the problem domain
2. Identify 8-24 relevant expert roles
3. Assign tiers (Core/Adjacent/Wildcard)
4. Assign relevance scores (0.20-0.95)
5. Call blue_dialogue_create with expert_pool

Example pool for "API Rate Limiting Strategy":

| Role | Tier | Relevance |
|------|------|-----------|
| API Architect | Core | 0.95 |
| Platform Engineer | Core | 0.90 |
| Security Engineer | Core | 0.85 |
| SRE Lead | Adjacent | 0.70 |
| Developer Advocate | Adjacent | 0.65 |
| Cost Analyst | Adjacent | 0.55 |
| Customer Success | Wildcard | 0.40 |
| Chaos Engineer | Wildcard | 0.30 |
```

## Test Plan

- [ ] `blue_dialogue_create` requires `expert_pool` for alignment mode
- [ ] Error returned when `expert_pool` missing in alignment mode
- [ ] Pool persisted to `expert-pool.json` in output directory
- [ ] Weighted sampling: higher relevance = higher selection probability
- [ ] Tier distribution respected: ~33% Core, ~42% Adjacent, ~25% Wildcard
- [ ] `rotation: "none"` uses same panel all rounds
- [ ] `rotation: "wildcards"` keeps Core/Adjacent, rotates Wildcards
- [ ] `rotation: "full"` resamples completely each round
- [ ] `blue_dialogue_sample_panel` respects retain/exclude
- [ ] `blue_dialogue_round_prompt` returns correct role from sampled panel
- [ ] Pastry names assigned correctly to sampled experts
- [ ] End-to-end: alignment dialogue with custom pool runs successfully

## Migration

**Breaking change**: Remove all keyword-based role selection.

- Delete `ROLE_KEYWORDS` constant
- Delete `GENERAL_ROLES` constant
- Delete `select_role_for_topic` function
- Alignment dialogues require `expert_pool` parameter
- Non-alignment dialogues unaffected

---

*"The Judge sees the elephant. The Judge summons the right blind men. The sampling ensures no single perspective dominates."*

— Blue
