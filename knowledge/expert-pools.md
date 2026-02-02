# Expert Pool System

When running alignment dialogues, the Judge creates domain-appropriate expert pools from which panels are sampled.

## Two-Phase Architecture (RFC 0047)

| Phase | Actor | Action |
|-------|-------|--------|
| **Pool Design** | Judge | Creates 15-30 domain-specific experts with tiers and relevance |
| **Panel Sampling** | MCP Server | Samples N experts using weighted random selection |

## Pool Design (Judge Responsibility)

The Judge reads the RFC/topic and designs experts appropriate to the domain:

```json
{
  "expert_pool": {
    "domain": "Investment Analysis",
    "experts": [
      { "role": "Value Analyst", "tier": "Core", "relevance": 0.95, "focus": "Intrinsic value, margin of safety" },
      { "role": "Growth Analyst", "tier": "Core", "relevance": 0.90, "focus": "TAM expansion, revenue acceleration" },
      { "role": "Risk Manager", "tier": "Core", "relevance": 0.85, "focus": "Downside scenarios, tail events" },
      { "role": "ESG Analyst", "tier": "Adjacent", "relevance": 0.70, "focus": "Environmental, governance factors" },
      { "role": "Contrarian", "tier": "Wildcard", "relevance": 0.30, "focus": "Challenge consensus, find crowding" }
    ]
  }
}
```

## Tier Distribution

| Tier | Pool % | Panel % | Selection Behavior |
|------|--------|---------|-------------------|
| **Core** | ~25% | ~33% | Almost always selected (high relevance weights) |
| **Adjacent** | ~40% | ~42% | High probability, related expertise |
| **Wildcard** | ~35% | ~25% | Fresh perspectives, rotation candidates |

## Panel Sampling (MCP Server)

```
blue_dialogue_create(expert_pool=[...24 roles...], panel_size=12, rotation="wildcards")
  → Weighted random sample: higher relevance = higher selection probability
  → For N=12: ~4 Core, ~5 Adjacent, ~3 Wildcard
```

## Rotation Modes

| Mode | Behavior | Use Case |
|------|----------|----------|
| `none` | Fixed panel all rounds | Standard deliberation |
| `wildcards` | Core/Adjacent persist, Wildcards resample | Bring fresh perspectives each round |
| `full` | Complete resample each round | Maximum diversity (experimental) |

## Pastry Naming

Experts are assigned pastry names for identification:
Muffin, Cupcake, Scone, Eclair, Donut, Brioche, Croissant, Macaron, Cannoli, Strudel, Beignet, Churro, Profiterole, Tartlet, Galette, Palmier, Kouign, Sfogliatella, Financier, Religieuse

## Domain-Specific Pools

The Judge designs pools appropriate to each domain. Example domains:

**Investment Analysis**: Value Analyst, Growth Analyst, Risk Manager, Portfolio Strategist, ESG Analyst, Quant Strategist, Technical Analyst, Behavioral Analyst, Income Analyst, Macro Economist, Credit Analyst, Contrarian

**System Architecture**: Platform Architect, Security Engineer, Database Architect, SRE Lead, API Designer, DevOps Engineer, Performance Engineer, Network Engineer, Cost Analyst, Compliance Officer

**Product Development**: Product Manager, UX Designer, Frontend Architect, Customer Advocate, Data Analyst, Backend Engineer, QA Lead, Technical Writer, Marketing Strategist

## Expert Prompt Template

Each expert receives their context:

```
You are {name} 🧁, a {role} in an ALIGNMENT-seeking dialogue.
Tier: {tier} | Relevance: {relevance}
Focus: {focus}

Your contribution is scored on PRECISION, not volume.
One sharp insight beats ten paragraphs.
```

## Pool Persistence

Pools are stored per-dialogue:

```
{output_dir}/
├── expert-pool.json      ← Full pool definition (Judge writes)
├── round-0/
│   ├── panel.json        ← Sampled panel for this round
│   └── *.md              ← Agent responses
└── scoreboard.md
```

---

*"The Judge sees the elephant. The Judge summons the right blind men."*
