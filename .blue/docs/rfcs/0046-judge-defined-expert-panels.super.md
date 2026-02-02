# RFC 0046: Judge Defined Expert Panels

| | |
|---|---|
| **Status** | Superseded |
| **Date** | 2026-02-01 |
| **ADRs** | 0014 (Alignment Dialogue Agents) |

---

## Summary

The alignment dialogue system currently uses keyword matching against the topic title to select expert roles, falling back to generic roles like "Systems Thinker" and "Domain Expert". This produces inappropriate expert panels for domain-specific topics (e.g., investment strategy gets software engineering roles instead of investment analysts, risk managers, portfolio strategists). The Judge should be able to define custom expert panels appropriate for the specific problem being deliberated.

## Problem

When the Judge calls `blue_dialogue_create` for an alignment dialogue, expert roles are auto-selected via:

1. Keyword matching against topic title (e.g., "security" → "Security Architect")
2. Fallback to generic roles: Systems Thinker, Domain Expert, Devil's Advocate, etc.

This fails for:
- **Domain-specific topics**: Investment analysis gets "Systems Architect" instead of "Portfolio Manager"
- **Cross-functional topics**: A product launch might need Marketing, Legal, Finance perspectives
- **Novel domains**: Topics without keyword matches get only generic roles

The Judge understands the problem space after reading the topic. The Judge should select appropriate experts.

## Design

### New Parameter: `expert_panel`

Add an `expert_panel` parameter to `blue_dialogue_create`:

```json
{
  "title": "Investment Strategy for Q3 Portfolio Rebalancing",
  "alignment": true,
  "agents": 5,
  "expert_panel": [
    "Investment Analyst",
    "Risk Manager",
    "Portfolio Strategist",
    "Compliance Officer",
    "Market Economist"
  ]
}
```

### Behavior

When `expert_panel` is provided:
- Use the provided roles in order
- Array length determines agent count (ignore `agents` param if both provided)
- Assign pastry names automatically (Muffin, Cupcake, Scone...)
- Assign tiers based on position: first ~33% Core, next ~42% Adjacent, final ~25% Wildcard
- Calculate relevance scores within each tier

When `expert_panel` is omitted:
- Remove keyword matching entirely
- Require `expert_panel` for alignment dialogues
- Error: "Alignment dialogues require expert_panel parameter"

### Example Output

```markdown
## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 🧁 Muffin | Investment Analyst | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Risk Manager | Core | 0.90 | 🧁 |
| 🧁 Scone | Portfolio Strategist | Adjacent | 0.70 | 🧁 |
| 🧁 Eclair | Compliance Officer | Adjacent | 0.65 | 🧁 |
| 🧁 Donut | Market Economist | Wildcard | 0.40 | 🧁 |
```

### SKILL.md Update

Update the alignment-play skill to document the workflow:

```
## Round 0: Panel Design

Before creating the dialogue, the Judge:
1. Reads the topic/RFC thoroughly
2. Identifies relevant domains and expertise needed
3. Designs a panel of 3-12 experts appropriate to the problem
4. Creates the dialogue with the custom expert_panel
```

## Implementation

### Changes to `dialogue.rs`

1. **Remove** `ROLE_KEYWORDS` and `GENERAL_ROLES` constants
2. **Remove** `select_role_for_topic` function
3. **Modify** `handle_create` to parse `expert_panel` array
4. **Modify** `assign_pastry_agents` to accept optional `Vec<String>` roles
5. **Error** if `alignment: true` but no `expert_panel` provided

### Code Changes

```rust
// In handle_create:
let expert_panel: Option<Vec<String>> = args
    .get("expert_panel")
    .and_then(|v| v.as_array())
    .map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });

// Require expert_panel for alignment mode
if alignment && expert_panel.is_none() {
    return Err(ServerError::InvalidParams);
}

let agent_count = expert_panel.as_ref().map(|p| p.len()).unwrap_or(agent_count);
let agents = assign_pastry_agents(agent_count, expert_panel);
```

```rust
// Modified assign_pastry_agents:
pub fn assign_pastry_agents(count: usize, roles: Option<Vec<String>>) -> Vec<PastryAgent> {
    let (core_count, adjacent_count, _wildcard_count) = tier_split(count);

    (0..count)
        .map(|i| {
            let name = PASTRY_NAMES.get(i).unwrap_or(&"Pastry").to_string();
            let role = roles
                .as_ref()
                .and_then(|r| r.get(i))
                .cloned()
                .unwrap_or_else(|| "Expert".to_string());
            // ... tier/relevance assignment unchanged
        })
        .collect()
}
```

## Test Plan

- [ ] `blue_dialogue_create` with `expert_panel` creates correct roles
- [ ] `blue_dialogue_create` without `expert_panel` in alignment mode returns error
- [ ] Pastry names assigned correctly regardless of panel
- [ ] Tier distribution follows Core/Adjacent/Wildcard split
- [ ] `blue_dialogue_round_prompt` returns correct role in prompt
- [ ] End-to-end: alignment dialogue with custom panel runs successfully

## Migration

No migration needed. This is a breaking change:
- Remove keyword-based role selection entirely
- Alignment dialogues now require `expert_panel`
- Non-alignment dialogues unaffected

---

*"The Judge sees the whole elephant. The Judge picks which blind men to summon."*

— Blue
