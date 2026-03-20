# RFC 0051: Global Perspective & Tension Tracking

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-02-02 |
| **ADRs** | 0014 (Alignment Dialogue Agents), 0005 (Single Source), 0018 (DynamoDB-Portable Schema) |
| **Extends** | RFC 0048 (Expert Pools), RFC 0050 (Graduated Rotation) |
| **Extended By** | RFC 0054 (Calibrated Alignment — Ethos & Tenets) |
| **Dialogue** | 2047 (5 experts, 3 rounds, ALIGNMENT 427) |

---

## Summary

The alignment dialogue system currently tracks perspectives and tensions within round summaries, making cross-dialogue analysis difficult. This RFC introduces **global perspective and tension tracking** with:

1. **Two-phase ID assignment**: Agents write local IDs (`MUFFIN-P0001`), Judge registers with global IDs (`P0001`)
2. **Global IDs**: `P0001`, `R0001`, `T0001` — sequential per round, expert in data not ID
3. **DB-first architecture**: All data registered via MCP tools, agents get context from DB
4. **First-class entities**: Perspectives (P), recommendations (R), tensions (T), evidence (E), claims (C) with full lifecycle
5. **LLM-friendly transcripts**: MCP generates markdown context, writes to disk for debugging
6. **Dynamic expert creation**: Judge can create experts mid-dialogue to address emerging needs
7. **JSON export**: Full provenance from database queries (no file parsing)
8. **Calibration support**: Optional ethos integration for calibrated dialogues (see RFC 0054)

## Problem

In the NVIDIA Investment Decision dialogue:
- 24 perspectives surfaced across 3 rounds from 12 experts
- 12 tensions raised and resolved
- **No global numbering**: P01 in round 0 vs P01 in round 1 are ambiguous
- **No origin tracking**: "Who said this?" requires reading full transcripts
- **No lifecycle visibility**: When was T0003 resolved? By whom?

The reference export format (`dialogue.json`) lacks these fields, making it unsuitable for:
- Cross-dialogue analysis
- Visualization dashboards
- Audit and compliance review

## Design

### Core Principle: Two-Phase ID Assignment

**Phase 1 — Agents write with local IDs:** `{UPPER(expert)}-{TYPE}{round:02d}{seq:02d}`
```markdown
[MUFFIN-P0001: Income mandate mismatch]
[DONUT-R0001: Options collar]
[MUFFIN-T0001: Growth vs income]
[MUFFIN-E0001: Historical IV data]
[MUFFIN-C0001: Income gap is bridgeable]
```

**Phase 2 — Judge registers, MCP assigns global IDs:**
```
P0001, P0002, P0003...       (perspectives, sequential per round)
R0001, R0002...              (recommendations, sequential per round)
T0001, T0002...              (tensions, sequential per round)
E0001, E0002...              (evidence, sequential per round)
C0001, C0002...              (claims, sequential per round)
```

**Storage key:** `(dialogue_id, round, seq)` with `expert_slug` as attribution

**Display ID:** `P{round:02d}{seq:02d}` — first 2 digits = round, last 2 digits = seq
- `P0001` = round 0, seq 1
- `P0102` = round 1, seq 2
- `P0215` = round 2, seq 15

Expert attribution is stored in the record, not encoded in the ID. All downstream references use global IDs.

### Schema

```sql
-- ================================================================
-- DIALOGUES (root table - enforces unique dialogue_id)
-- ================================================================
CREATE TABLE dialogues (
  dialogue_id     TEXT PRIMARY KEY,  -- slug derived from title
  title           TEXT NOT NULL,
  question        TEXT,
  status          TEXT NOT NULL DEFAULT 'open',
  created_at      TEXT NOT NULL,
  converged_at    TEXT,
  total_rounds    INT DEFAULT 0,
  total_alignment INT DEFAULT 0,
  output_dir      TEXT,              -- /tmp/blue-dialogue/{slug}

  -- Calibration (RFC 0054)
  calibrated      BOOLEAN DEFAULT FALSE,
  domain_id       TEXT,              -- FK to domains table (RFC 0054)
  ethos_id        TEXT,              -- FK to ethos table (RFC 0054)

  CHECK (status IN ('open', 'converging', 'converged', 'abandoned'))
);

-- ================================================================
-- VERDICTS (first-class entities, supports multiple per dialogue)
-- ================================================================
CREATE TABLE verdicts (
  dialogue_id     TEXT NOT NULL,
  verdict_id      TEXT NOT NULL,     -- V01, V02, or slug like "final", "minority"
  verdict_type    TEXT NOT NULL,     -- interim | final | minority | dissent
  round           INT  NOT NULL,     -- round when verdict was issued
  author_expert   TEXT,              -- expert who authored (null = Judge)
  recommendation  TEXT NOT NULL,     -- one-line decision
  description     TEXT NOT NULL,     -- reasoning summary (2-3 sentences)
  conditions      JSON,              -- ["condition1", "condition2"]
  vote            TEXT,              -- "11-1", "unanimous"
  confidence      TEXT,              -- unanimous | strong | split | contested
  tensions_resolved JSON,            -- ["T0001", "T0002"]
  tensions_accepted JSON,            -- ["T0006"] - acknowledged but not blocking
  recommendations_adopted JSON,      -- ["R0001"]
  key_evidence    JSON,              -- ["E0001", "E0101"] - evidence supporting verdict
  key_claims      JSON,              -- ["C0001", "C0101"] - claims adopted in verdict
  supporting_experts JSON,           -- ["muffin", "cupcake", ...] for minority verdicts

  -- Ethos compliance (RFC 0054) — only for calibrated dialogues
  ethos_compliance JSON,             -- {"fully_compliant": bool, "exceptions": [...], "violations": [...]}

  created_at      TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, verdict_id),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (verdict_type IN ('interim', 'final', 'minority', 'dissent'))
);

-- Prevent exact duplicate titles at same timestamp
CREATE UNIQUE INDEX idx_dialogues_title_created
  ON dialogues(title, created_at);

-- ================================================================
-- EXPERTS (participation and scores per dialogue)
-- ================================================================
CREATE TABLE experts (
  dialogue_id   TEXT NOT NULL,
  expert_slug   TEXT NOT NULL,     -- muffin, cupcake, etc. (capitalize for display)
  role          TEXT NOT NULL,     -- Value Analyst, Risk Manager, etc.
  description   TEXT,              -- detailed role description
  focus         TEXT,              -- brief focus area
  tier          TEXT NOT NULL,     -- Core, Adjacent, Wildcard
  source        TEXT NOT NULL,     -- pool | created
  relevance     REAL,              -- 0.0-1.0 (from pool, or Judge-assigned for created)
  creation_reason TEXT,            -- why Judge created this expert (null if from pool)
  color         TEXT,              -- hex color for UI
  scores        JSON,              -- {"0": 12, "1": 8, "2": 15} - per-round scores
  raw_content   JSON,              -- {"0": "markdown...", "1": "markdown..."} - per-round responses
  total_score   INT DEFAULT 0,     -- computed: sum of scores (denormalized)
  first_round   INT,               -- round when expert first participated
  created_at    TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, expert_slug),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (tier IN ('Core', 'Adjacent', 'Wildcard')),
  CHECK (source IN ('pool', 'created'))
);

-- ================================================================
-- ROUNDS (metadata and scores per round)
-- ================================================================
CREATE TABLE rounds (
  dialogue_id   TEXT NOT NULL,
  round         INT  NOT NULL,
  title         TEXT,              -- "Opening Arguments", "Refinement", etc.
  score         INT  NOT NULL,     -- ALIGNMENT score for this round
  summary       TEXT,              -- Judge's synthesis
  status        TEXT NOT NULL DEFAULT 'open',
  created_at    TEXT NOT NULL,
  completed_at  TEXT,
  PRIMARY KEY (dialogue_id, round),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('open', 'in_progress', 'completed'))
);

-- ================================================================
-- PERSPECTIVES
-- ================================================================
CREATE TABLE perspectives (
  dialogue_id    TEXT NOT NULL,
  round          INT  NOT NULL,     -- origin round
  seq            INT  NOT NULL,     -- global per round (P0001, P0002...)
  label          TEXT NOT NULL,
  content        TEXT NOT NULL,     -- Judge-synthesized content
  contributors   JSON NOT NULL,     -- ["muffin", "cupcake"] - all who contributed
  status         TEXT NOT NULL DEFAULT 'open',
  references     JSON,              -- [{"type": "refine", "target": "P0001"}, ...]
  created_at     TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('open', 'refined', 'conceded', 'merged'))
);

-- Perspective events (audit trail)
CREATE TABLE perspective_events (
  dialogue_id       TEXT NOT NULL,
  perspective_round INT  NOT NULL,
  perspective_seq   INT  NOT NULL,
  event_type        TEXT NOT NULL,  -- created, refined, conceded, merged
  event_round       INT  NOT NULL,  -- round when event occurred
  actors            JSON NOT NULL,  -- ["muffin"] - who triggered this event
  result_id         TEXT,           -- e.g., "P0101" if refined into new perspective
  created_at        TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, perspective_round, perspective_seq, created_at),
  FOREIGN KEY (dialogue_id, perspective_round, perspective_seq)
    REFERENCES perspectives(dialogue_id, round, seq),
  CHECK (event_type IN ('created', 'refined', 'conceded', 'merged'))
);

CREATE INDEX idx_perspectives_dialogue_round
  ON perspectives(dialogue_id, round, created_at);

-- ================================================================
-- TENSIONS
-- ================================================================
CREATE TABLE tensions (
  dialogue_id      TEXT NOT NULL,
  round            INT  NOT NULL,    -- origin round
  seq              INT  NOT NULL,    -- global per round (T0001, T0002...)
  label            TEXT NOT NULL,
  description      TEXT NOT NULL,    -- Judge-synthesized description
  contributors     JSON NOT NULL,    -- ["muffin", "cupcake"] - all who surfaced this
  status           TEXT NOT NULL DEFAULT 'open',
  references       JSON,             -- [{"type": "depend", "target": "P0001"}, ...]
  created_at       TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('open', 'addressed', 'resolved', 'reopened'))
);

-- Display ID: T{round:02d}{seq:02d}
-- e.g., T0001, T0102

CREATE INDEX idx_tensions_status ON tensions(dialogue_id, status);

-- ================================================================
-- MOVES (dialogue moves: defend, challenge, bridge, etc.)
-- ================================================================
CREATE TABLE moves (
  dialogue_id   TEXT NOT NULL,
  round         INT  NOT NULL,
  seq           INT  NOT NULL,
  expert_slug   TEXT NOT NULL,
  move_type     TEXT NOT NULL,       -- defend, challenge, bridge, request, concede, converge
  targets       JSON NOT NULL,       -- ["P0001", "R0001"] - IDs this move references
  context       TEXT,                -- brief explanation
  created_at    TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, expert_slug, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (move_type IN ('defend', 'challenge', 'bridge', 'request', 'concede', 'converge'))
);

-- ================================================================
-- RECOMMENDATIONS (first-class, links to tensions and perspectives)
-- ================================================================
CREATE TABLE recommendations (
  dialogue_id        TEXT NOT NULL,
  round              INT  NOT NULL,   -- origin round
  seq                INT  NOT NULL,   -- global per round (R0001, R0002...)
  label              TEXT NOT NULL,
  content            TEXT NOT NULL,   -- Judge-synthesized content
  contributors       JSON NOT NULL,   -- ["donut", "muffin"] - all who contributed
  parameters         JSON,            -- structured data (e.g., options parameters)
  status             TEXT NOT NULL DEFAULT 'proposed',
  references         JSON,            -- [{"type": "refine", "target": "R0001"}, {"type": "address", "target": "T0001"}, ...]
  adopted_in_verdict TEXT,            -- verdict_id if adopted
  created_at         TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('proposed', 'amended', 'adopted', 'rejected'))
);

-- Recommendation events (audit trail)
CREATE TABLE recommendation_events (
  dialogue_id     TEXT NOT NULL,
  rec_round       INT  NOT NULL,
  rec_seq         INT  NOT NULL,
  event_type      TEXT NOT NULL,   -- created, amended, adopted, rejected
  event_round     INT  NOT NULL,
  actors          JSON NOT NULL,   -- who triggered this event
  result_id       TEXT,            -- new recommendation ID if amended
  created_at      TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, rec_round, rec_seq, created_at),
  FOREIGN KEY (dialogue_id, rec_round, rec_seq)
    REFERENCES recommendations(dialogue_id, round, seq),
  CHECK (event_type IN ('created', 'amended', 'adopted', 'rejected'))
);

-- Display ID: R{round:02d}{seq:02d}
-- e.g., R0001, R0102

CREATE INDEX idx_recommendations_status
  ON recommendations(dialogue_id, status);

-- ================================================================
-- EVIDENCE (first-class: data, precedents, facts)
-- ================================================================
CREATE TABLE evidence (
  dialogue_id    TEXT NOT NULL,
  round          INT  NOT NULL,
  seq            INT  NOT NULL,     -- global per round (E0001, E0002...)
  label          TEXT NOT NULL,
  content        TEXT NOT NULL,     -- the data/facts
  contributors   JSON NOT NULL,     -- ["muffin"]
  status         TEXT NOT NULL DEFAULT 'cited',
  references     JSON,              -- [{"type": "support", "target": "P0001"}]
  created_at     TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('cited', 'challenged', 'confirmed', 'refuted'))
);

-- Display ID: E{round:02d}{seq:02d}
-- e.g., E0001, E0103

CREATE INDEX idx_evidence_status
  ON evidence(dialogue_id, status);

-- ================================================================
-- CLAIMS (first-class: quotable position statements)
-- ================================================================
CREATE TABLE claims (
  dialogue_id    TEXT NOT NULL,
  round          INT  NOT NULL,
  seq            INT  NOT NULL,     -- global per round (C0001, C0002...)
  label          TEXT NOT NULL,
  content        TEXT NOT NULL,     -- the claim statement
  contributors   JSON NOT NULL,     -- ["muffin"]
  status         TEXT NOT NULL DEFAULT 'asserted',
  references     JSON,              -- [{"type": "depend", "target": "P0001"}]
  created_at     TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, round, seq),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('asserted', 'supported', 'opposed', 'adopted', 'withdrawn'))
);

-- Display ID: C{round:02d}{seq:02d}
-- e.g., C0001, C0205

CREATE INDEX idx_claims_status
  ON claims(dialogue_id, status);

-- ================================================================
-- REFS (explicit cross-references between entities)
-- ================================================================
CREATE TABLE refs (
  dialogue_id   TEXT NOT NULL,
  source_type   TEXT NOT NULL,     -- P, R, T, E, C
  source_id     TEXT NOT NULL,     -- P0101, R0001, etc.
  ref_type      TEXT NOT NULL,     -- support, oppose, refine, address, resolve, reopen, question, depend
  target_type   TEXT NOT NULL,     -- P, R, T, E, C
  target_id     TEXT NOT NULL,     -- P0001, T0001, etc.
  created_at    TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, source_id, ref_type, target_id),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),

  -- Type enum constraints
  CHECK (source_type IN ('P', 'R', 'T', 'E', 'C')),
  CHECK (target_type IN ('P', 'R', 'T', 'E', 'C')),
  CHECK (ref_type IN ('support', 'oppose', 'refine', 'address', 'resolve', 'reopen', 'question', 'depend')),

  -- Type consistency: source_type must match source_id prefix
  CHECK (
    (source_type = 'P' AND source_id LIKE 'P%') OR
    (source_type = 'R' AND source_id LIKE 'R%') OR
    (source_type = 'T' AND source_id LIKE 'T%') OR
    (source_type = 'E' AND source_id LIKE 'E%') OR
    (source_type = 'C' AND source_id LIKE 'C%')
  ),

  -- Type consistency: target_type must match target_id prefix
  CHECK (
    (target_type = 'P' AND target_id LIKE 'P%') OR
    (target_type = 'R' AND target_id LIKE 'R%') OR
    (target_type = 'T' AND target_id LIKE 'T%') OR
    (target_type = 'E' AND target_id LIKE 'E%') OR
    (target_type = 'C' AND target_id LIKE 'C%')
  ),

  -- Semantic constraints: which ref_types are valid for which targets
  CHECK (
    -- Tension-only targets: resolve, reopen, address must target T
    (ref_type IN ('resolve', 'reopen', 'address') AND target_type = 'T') OR
    -- Same-type only: refine must be source_type = target_type
    (ref_type = 'refine' AND source_type = target_type) OR
    -- Flexible: support, oppose, question, depend can target anything
    (ref_type IN ('support', 'oppose', 'question', 'depend'))
  )
);

-- Query: "What supports P0001?"
CREATE INDEX idx_refs_target
  ON refs(dialogue_id, target_id, ref_type);

-- Query: "What does P0101 reference?"
CREATE INDEX idx_refs_source
  ON refs(dialogue_id, source_id);

-- ================================================================
-- TENSION_EVENTS (lifecycle audit trail)
-- ================================================================
CREATE TABLE tension_events (
  dialogue_id    TEXT NOT NULL,
  tension_round  INT  NOT NULL,   -- round of the tension
  tension_seq    INT  NOT NULL,   -- seq of the tension
  event_type     TEXT NOT NULL,
  event_round    INT  NOT NULL,   -- round when event occurred
  actors         JSON NOT NULL,   -- ["muffin", "donut"] - who triggered this
  reason         TEXT,
  reference      TEXT,            -- global ID of resolving item (e.g., "P0005", "R0001")
  created_at     TEXT NOT NULL,
  PRIMARY KEY (dialogue_id, tension_round, tension_seq, created_at),
  FOREIGN KEY (dialogue_id, tension_round, tension_seq)
    REFERENCES tensions(dialogue_id, round, seq),
  CHECK (event_type IN ('created', 'addressed', 'resolved', 'reopened', 'commented'))
);
```

### Dialogue ID Collision Handling

The `dialogue_id` is derived from the title via slugification. Collisions are handled at creation time:

```
Algorithm: generate_dialogue_id(title)
  1. slug = slugify(title)  // "NVIDIA Analysis" → "nvidia-analysis"
  2. IF NOT EXISTS(SELECT 1 FROM dialogues WHERE dialogue_id = slug):
       RETURN slug
  3. FOR i IN 1..99:
       candidate = slug + "-" + i  // "nvidia-analysis-2"
       IF NOT EXISTS(SELECT 1 FROM dialogues WHERE dialogue_id = candidate):
         RETURN candidate
  4. FAIL "Too many dialogues with similar titles"
```

**Output directory** follows the same pattern:
- `/tmp/blue-dialogue/{dialogue_id}/`

This ensures:
- No file system collisions between concurrent dialogues
- Deterministic ID generation (same title → same base slug)
- Human-readable IDs (not UUIDs)

### Tension Lifecycle

```
                    ┌──────────────┐
                    │   created    │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
           ┌───────►│    open      │◄───────┐
           │        └──────┬───────┘        │
           │               │                │
           │               ▼                │
           │        ┌──────────────┐        │
           │        │  addressed   │────────┤
           │        └──────┬───────┘        │
           │               │                │
           │               ▼                │
           │        ┌──────────────┐        │
           └────────│   resolved   │────────┘
                    └──────────────┘
                           │
                           ▼ (if insufficient)
                    ┌──────────────┐
                    │   reopened   │
                    └──────────────┘
```

**Authority Rules:**
- Any expert can propose `addressed` with a resolution mechanism
- Original author or facilitator confirms `resolved`
- Reopening creates an event, preserving full history

### JSON Export Format

```json
{
  "id": "nvidia-investment-decision",
  "title": "NVIDIA Investment Analysis",
  "date": "2026-02-02",
  "status": "converged",
  "totalAlignment": 423,

  // Calibration (RFC 0054) — omitted if uncalibrated
  "calibration": {
    "calibrated": true,
    "domain": "fiduciary-investment",
    "ethos_id": "ethos-nvidia-001",
    "rules_count": 14,
    "compliance": {
      "fully_compliant": false,
      "exceptions": 1,
      "violations": 0
    }
  },

  "experts": [
    {
      "slug": "muffin",             // capitalize for display: "Muffin"
      "role": "Value Analyst",
      "tier": "Core",
      "scores": {
        "0": 12,
        "1": 8,
        "2": 15
      },
      "total": 35
    },
    {
      "slug": "palmier",
      "role": "Geopolitical Risk Analyst",
      "tier": "Adjacent",
      "source": "created",          // joined mid-dialogue
      "scores": {
        "2": 14                     // only present in round 2
      },
      "total": 14
    }
  ],

  "rounds_data": [
    {
      "round": 0,
      "score": 117,
      "experts": {
        "muffin": {
          "score": 12,
          "actions": [
            { "type": "perspective", "action": "created", "id": "P0001" },
            { "type": "tension", "action": "created", "id": "T0001" }
          ]
        },
        "cupcake": {
          "score": 10,
          "actions": [
            { "type": "perspective", "action": "created", "id": "P0002" },
            { "type": "tension", "action": "created", "id": "T0002" }
          ]
        },
        "donut": {
          "score": 15,
          "actions": [
            { "type": "perspective", "action": "created", "id": "P0003" },
            { "type": "recommendation", "action": "created", "id": "R0001" }
          ]
        }
      }
    },
    {
      "round": 1,
      "score": 45,
      "experts": {
        "muffin": {
          "score": 8,
          "actions": [
            { "type": "perspective", "action": "refined", "id": "P0101", "refines": "P0001" },
            { "type": "tension", "action": "addressed", "id": "T0001", "via": "R0001" }
          ]
        },
        "donut": {
          "score": 10,
          "actions": [
            { "type": "recommendation", "action": "amended", "id": "R0101", "amends": "R0001" },
            { "type": "tension", "action": "resolved", "id": "T0002", "via": "P0101" }
          ]
        }
      }
    }
  ],

  "perspectives": [
    {
      "id": "P0001",
      "label": "Income mandate mismatch",
      "content": "NVIDIA's zero dividend conflicts with 4% income requirement...",
      "status": "refined",
      "origin": {
        "round": 0,
        "contributors": ["muffin", "cupcake"]  // Judge synthesized from both
      },
      "events": [
        { "type": "created", "round": 0, "by": ["muffin", "cupcake"] },
        { "type": "refined", "round": 1, "by": ["muffin"], "result": "P0101" }
      ]
    }
  ],

  "tensions": [
    {
      "id": "T0001",
      "label": "Growth vs income obligation",
      "description": "NVIDIA's zero dividend conflicts with trust's 4% income requirement",
      "status": "resolved",
      "origin": {
        "round": 0,
        "contributors": ["muffin", "cupcake"]
      },
      "events": [
        { "type": "created", "round": 0, "by": ["muffin", "cupcake"] },
        { "type": "addressed", "round": 1, "by": ["donut"], "reference": "R0001" },
        { "type": "resolved", "round": 2, "by": ["muffin"], "reference": "P0201" }
      ],
      "relatedPerspectives": ["P0001", "P0003"]
    }
  ],

  "recommendations": [
    {
      "id": "R0001",
      "label": "Income Collar Structure",
      "content": "Strike Selection Framework with 30-delta covered calls...",
      "parameters": {
        "covered_call_delta": "0.20-0.25",
        "protective_put_delta": "-0.15",
        "dte": "30-45"
      },
      "status": "adopted",
      "origin": {
        "round": 0,
        "contributors": ["donut"]
      },
      "events": [
        { "type": "created", "round": 0, "by": ["donut"] },
        { "type": "amended", "round": 1, "by": ["donut", "muffin"], "result": "R0101" },
        { "type": "adopted", "round": 2, "by": ["judge"], "reference": "verdict-final" }
      ],
      "addresses_tensions": ["T0001"],
      "builds_on": ["P0001"]
    }
  ],

  "evidence": [
    {
      "id": "E0001",
      "label": "Historical options premium data",
      "content": "NVDA 30-day ATM IV averaged 45% over past 24 months. 30-delta calls yielded 2.1-2.8% monthly premium.",
      "status": "confirmed",
      "origin": {
        "round": 1,
        "contributors": ["muffin"]
      },
      "events": [
        { "type": "cited", "round": 1, "by": ["muffin"] },
        { "type": "confirmed", "round": 2, "by": ["donut", "brioche"] }
      ],
      "supports": ["P0101", "R0001"]
    },
    {
      "id": "E0002",
      "label": "Trust covenant constraints",
      "content": "Section 4.2 requires minimum 4% annual distribution. Corpus preservation clause limits equity drawdown to 15%.",
      "status": "confirmed",
      "origin": {
        "round": 0,
        "contributors": ["cupcake"]
      },
      "events": [
        { "type": "cited", "round": 0, "by": ["cupcake"] },
        { "type": "confirmed", "round": 1, "by": ["muffin"] }
      ],
      "supports": ["T0001"]
    }
  ],

  "claims": [
    {
      "id": "C0001",
      "label": "Income mandate resolved via options",
      "content": "The 4% income mandate can be satisfied through covered call premium generation, eliminating the primary objection to NVDA exposure.",
      "status": "adopted",
      "origin": {
        "round": 1,
        "contributors": ["muffin"]
      },
      "events": [
        { "type": "asserted", "round": 1, "by": ["muffin"] },
        { "type": "supported", "round": 2, "by": ["donut", "brioche"] },
        { "type": "adopted", "round": 2, "by": ["judge"], "reference": "verdict-final" }
      ],
      "depends_on": ["E0001", "P0101", "R0001"]
    },
    {
      "id": "C0002",
      "label": "Concentration risk manageable with phased entry",
      "content": "Geographic and sector concentration concerns are mitigated by a 6-month phased entry strategy.",
      "status": "adopted",
      "origin": {
        "round": 2,
        "contributors": ["cupcake", "muffin"]
      },
      "events": [
        { "type": "asserted", "round": 2, "by": ["cupcake"] },
        { "type": "supported", "round": 2, "by": ["muffin"] },
        { "type": "adopted", "round": 2, "by": ["judge"], "reference": "verdict-final" }
      ],
      "depends_on": ["P0003", "T0002"]
    }
  ]
}
```

### MCP Server Integration

**Public tools (Judge-facing API):**

| Tool | Purpose |
|------|---------|
| `blue_dialogue_create` | Start dialogue with expert pool; returns Judge Protocol |
| `blue_dialogue_round_context` | **Bulk fetch** structured data for all panel experts in one call |
| `blue_dialogue_expert_create` | Create new expert mid-dialogue to address emerging needs |
| `blue_dialogue_round_register` | **Bulk register** all round data in single call (perspectives, recommendations, tensions, evidence, claims, refs, scores) |
| `blue_dialogue_verdict_register` | Register a verdict (interim, final, minority, dissent) |
| `blue_dialogue_export` | Generate JSON export from database (no file parsing) |

**Internal functions (not exposed as MCP tools):**

These are implementation details called by the bulk tools above:

| Function | Called By |
|----------|-----------|
| `perspective_register` | `round_register` |
| `recommendation_register` | `round_register` |
| `tension_register` | `round_register` |
| `tension_update` | `round_register` |
| `evidence_register` | `round_register` |
| `claim_register` | `round_register` |
| `ref_register` | `round_register` |
| `recommendation_update` | `round_register`, `verdict_register` |

This gives the Judge a clean 6-tool API while maintaining internal composability.

**Defaults (Judge discretion unless user overrides):**

| Parameter | Default | User Override |
|-----------|---------|---------------|
| Panel size | Judge selects based on domain complexity | "Use 8 experts" |
| Convergence | 100% (all tensions resolved, all experts agree) | "Run to 80% convergence" |

**100% convergence** means:
- All tensions resolved or explicitly accepted
- All experts agree with the verdict (no dissenters)

Users can request early termination or partial convergence for faster results.

**Verdict tool parameters:**

```json
{
  "dialogue_id": "nvidia-decision",
  "verdict_id": "final",           // or "V01", "minority-esg"
  "verdict_type": "final",         // interim | final | minority | dissent
  "round": 3,
  "author_expert": null,           // null = Judge, or expert slug
  "recommendation": "REJECT full swap. APPROVE conditional partial trim.",
  "description": "The panel unanimously rejected a full NVAI-to-NVDA swap due to income mandate mismatch...",
  "conditions": [
    "Execute 60-90 days post-refinancing",
    "Implement 30-delta covered calls"
  ],
  "vote": "12-0",
  "confidence": "unanimous",
  "tensions_resolved": ["T0001", "T0002", "T0003"],
  "tensions_accepted": ["T0006"],
  "recommendations_adopted": ["R0001"],
  "supporting_experts": null       // for minority verdicts: ["churro", "eclair"]
}
```

**Verdict types:**

| Type | Use Case |
|------|----------|
| `interim` | Checkpoint verdict mid-dialogue (e.g., "continue with options exploration") |
| `final` | Authoritative conclusion when dialogue converges |
| `minority` | Structured dissent from subset of experts |
| `dissent` | Single expert's formal objection with reasoning |

**Bulk round registration** (`blue_dialogue_round_register`):

Register all round data in a single call — perspectives, recommendations, tensions, evidence, claims, scores, and events:

```json
{
  "dialogue_id": "nvidia-decision",
  "round": 1,
  "score": 45,
  "summary": "Panel converging on conditional approval with options overlay...",

  "expert_scores": {
    "muffin": 8,
    "donut": 10,
    "cupcake": 7
  },

  "perspectives": [
    {
      "local_id": "MUFFIN-P0101",
      "label": "Options viability confirmed",
      "content": "The 30-delta covered call strategy...",
      "contributors": ["muffin"],
      "references": [
        { "type": "refine", "target": "P0001" },
        { "type": "support", "target": "R0001" },
        { "type": "address", "target": "T0001" }
      ]
    },
    {
      "local_id": "CUPCAKE-P0101",
      "label": "Concentration risk mitigated",
      "content": "Phased entry over 6 months...",
      "contributors": ["cupcake", "scone"],
      "references": [
        { "type": "address", "target": "T0002" }
      ]
    }
  ],

  "recommendations": [
    {
      "local_id": "DONUT-R0101",
      "label": "Amended collar structure",
      "content": "Updated strike selection...",
      "contributors": ["donut", "muffin"],
      "parameters": { "delta": "0.25", "dte": "45" },
      "references": [
        { "type": "refine", "target": "R0001" },
        { "type": "address", "target": "T0001" },
        { "type": "depend", "target": "MUFFIN-P0101" }
      ]
    }
  ],

  "tensions": [
    {
      "local_id": "CROISSANT-T0101",
      "label": "Execution timing",
      "description": "Post-refinancing window constraint",
      "contributors": ["croissant"],
      "references": [
        { "type": "depend", "target": "R0001" }
      ]
    }
  ],

  "evidence": [
    {
      "local_id": "MUFFIN-E0101",
      "label": "Historical options premium data",
      "content": "NVDA 30-day ATM IV averaged 45% over past 24 months. 30-delta calls yielded 2.1-2.8% monthly premium.",
      "contributors": ["muffin"],
      "references": [
        { "type": "support", "target": "MUFFIN-P0101" }
      ]
    }
  ],

  "claims": [
    {
      "local_id": "MUFFIN-C0101",
      "label": "Income mandate resolved",
      "content": "The income mandate objection is resolved via options overlay. Remaining concerns are execution timing and concentration—both manageable.",
      "contributors": ["muffin"],
      "references": [
        { "type": "depend", "target": "MUFFIN-P0101" },
        { "type": "depend", "target": "MUFFIN-E0101" }
      ]
    }
  ],

  "moves": [
    { "expert": "muffin", "type": "bridge", "targets": ["P0003", "R0001"], "context": "Reconciling concentration and collar" },
    { "expert": "donut", "type": "defend", "target": "R0001", "context": "Liquidity supports execution" }
  ],

  "tension_updates": [
    { "id": "T0001", "status": "addressed", "by": ["donut"], "via": "DONUT-R0101" },
    { "id": "T0002", "status": "resolved", "by": ["cupcake"], "via": "CUPCAKE-P0101" }
  ]
}
```

**Local ID format:** `{UPPER(expert)}-{TYPE}{round:02d}{seq:02d}`
- Perspectives: `MUFFIN-P0101`, `CUPCAKE-P0102`
- Proposals: `DONUT-R0101`, `MUFFIN-R0103`
- Tensions: `CROISSANT-T0101`, `SCONE-T0201`
- Evidence: `MUFFIN-E0101`, `DONUT-E0102`
- Claims: `MUFFIN-C0101`, `CUPCAKE-C0102`

**Resolution:**
- MCP resolves local IDs → global IDs before storing
- **Input**: Links can reference local IDs (same batch) or global IDs (prior rounds)
- **Storage**: All links stored with global IDs only (MCP resolves local refs)
- Return includes mapping: `local_id` → `global_id`

```json
```

**MCP resolves local IDs, assigns global IDs, returns mapping:**

Note: Local seq is expert-local, global seq is dialogue-global. Three experts each writing their "first" perspective (local `01`) get global `01`, `02`, `03`:

```json
{
  "round": 1,
  "id_mapping": {
    "MUFFIN-P0101": "P0101",     // Muffin's 1st → global 1st
    "CUPCAKE-P0101": "P0102",    // Cupcake's 1st → global 2nd
    "SCONE-P0101": "P0103",      // Scone's 1st → global 3rd
    "DONUT-R0101": "R0101",
    "CROISSANT-T0101": "T0101",
    "MUFFIN-E0101": "E0101",
    "MUFFIN-C0101": "C0101"
  },
  "perspectives": [
    { "local_id": "MUFFIN-P0101", "id": "P0101", "label": "Options viability confirmed" },
    { "local_id": "CUPCAKE-P0101", "id": "P0102", "label": "Concentration risk mitigated" },
    { "local_id": "SCONE-P0101", "id": "P0103", "label": "Execution timeline concern" }
  ],
  "recommendations": [
    { "local_id": "DONUT-R0101", "id": "R0101", "label": "Amended collar structure" }
  ],
  "tensions": [
    { "local_id": "CROISSANT-T0101", "id": "T0101", "label": "Execution timing" }
  ],
  "evidence": [
    { "local_id": "MUFFIN-E0101", "id": "E0101", "label": "Historical options premium data" }
  ],
  "claims": [
    { "local_id": "MUFFIN-C0101", "id": "C0101", "label": "Income mandate resolved" }
  ],
  "tension_updates": [
    { "id": "T0001", "status": "addressed", "via": "R0101" },
    { "id": "T0002", "status": "resolved", "via": "P0102" }
  ]
}
```

All references stored with resolved global IDs. Note that local seq ≠ global seq:
- Local `MUFFIN-P0101` = Muffin's 1st perspective in round 1 (expert-local numbering)
- Global `P0103` = 3rd perspective registered in round 1 (dialogue-global numbering)

The `id_mapping` in the response tells the Judge exactly which local ID became which global ID.

The Judge receives:
- Log what was registered
- Reference new IDs in subsequent operations
- Build the scoreboard with correct global IDs

This replaces multiple individual calls with a single atomic operation.

**Export tool parameters:**

```json
{
  "dialogue_id": "nvidia-decision",
  "output_path": "/path/to/dialogues/nvidia-decision.json"
}
```

**Returns:**

```json
{
  "status": "success",
  "path": "/path/to/dialogues/nvidia-decision.json",
  "stats": {
    "rounds": 3,
    "experts": 12,
    "perspectives": 24,
    "recommendations": 8,
    "tensions": 12,
    "evidence": 6,
    "claims": 4,
    "totalAlignment": 423
  },
  "warnings": [
    { "type": "missing_score", "expert": "scone", "round": 2, "message": "Expert in panel but no score registered" }
  ]
}
```

### Error Handling

**MCP validates all data before database operations.** SQLite CHECK constraint errors are unhelpful ("CHECK constraint failed"), so the MCP layer must provide actionable feedback.

**Error response format:**

```json
{
  "status": "error",
  "error_code": "invalid_ref_target",
  "field": "ref_type",
  "value": "resolve",
  "constraint": "semantic",
  "message": "ref_type 'resolve' can only target Tensions (T), but target_type is 'P'",
  "context": {
    "source_id": "P0101",
    "target_id": "P0001",
    "target_type": "P"
  },
  "valid_options": ["T"],
  "suggestion": "Use 'support' or 'oppose' for Perspective targets, or change target to a Tension ID"
}
```

**Error codes and validation rules:**

| Error Code | Constraint | Trigger | Message Template |
|------------|------------|---------|------------------|
| `invalid_entity_type` | Type enum | `source_type` or `target_type` not in (P, R, T, E, C) | `{field} must be one of: P, R, T, E, C (got '{value}')` |
| `invalid_ref_type` | Ref enum | `ref_type` not in valid set | `ref_type must be one of: support, oppose, refine, address, resolve, reopen, question, depend (got '{value}')` |
| `type_id_mismatch` | Type consistency | `source_type` doesn't match `source_id` prefix | `{type}_type '{type_value}' does not match {type}_id '{id}' (expected prefix '{expected}')` |
| `invalid_ref_target` | Semantic | `resolve`/`reopen`/`address` targeting non-Tension | `ref_type '{ref}' can only target Tensions (T), but target_type is '{actual}'` |
| `refine_type_mismatch` | Semantic | `refine` with different source/target types | `ref_type 'refine' requires same source and target type (got {source}→{target})` |
| `target_not_found` | Referential | Referenced ID doesn't exist | `target_id '{id}' not found in dialogue '{dialogue_id}'` |
| `invalid_status_transition` | Lifecycle | Invalid state transition | `Cannot transition {entity} from '{from}' to '{to}' (valid transitions: {valid})` |

**Validation order:**

1. **Type enums** — Check `source_type`, `target_type`, `ref_type` are valid values
2. **Type consistency** — Check ID prefixes match declared types
3. **Referential integrity** — Check target IDs exist in the dialogue
4. **Semantic constraints** — Check relationship type is valid for target type
5. **Lifecycle rules** — Check status transitions are valid

**Partial success handling:**

For bulk operations (`blue_dialogue_round_register`), the MCP uses atomic transactions:
- **All-or-nothing**: If any item fails validation, the entire batch is rejected
- **Detailed errors**: Response includes validation errors for ALL failed items, not just the first

```json
{
  "status": "error",
  "error_code": "batch_validation_failed",
  "message": "3 items failed validation",
  "errors": [
    {
      "item_type": "reference",
      "source_id": "P0101",
      "error_code": "invalid_ref_target",
      "message": "ref_type 'resolve' can only target Tensions..."
    },
    {
      "item_type": "perspective",
      "local_id": "MUFFIN-P0102",
      "error_code": "type_id_mismatch",
      "message": "Perspective ID must start with 'P', got 'MUFFIN-P0102'"
    }
  ],
  "suggestion": "Fix all errors and resubmit the entire batch"
}
```

**Judge recovery pattern:**

When the Judge receives a validation error:
1. Parse the `error_code` and `suggestion`
2. Identify the problematic items
3. Correct the data based on `valid_options` or `suggestion`
4. Resubmit the entire batch (for bulk operations)

The structured error format ensures the Judge can programmatically understand and fix issues without human intervention.

### Agent Context Model

**MCP provides data; Judge builds prompts; Agents receive prompts directly.**

The Judge calls `blue_dialogue_round_context` once to get structured data for **all panel experts**:

```json
// Request
{
  "dialogue_id": "nvidia-decision",
  "round": 1
}

// Response
{
  "dialogue": {
    "id": "nvidia-decision",
    "title": "NVIDIA Investment Analysis",
    "question": "Should Acme Trust swap its NVAI position for NVDA shares?",
    "status": "open",
    "current_round": 1,
    "total_alignment": 117,
    "background": {
      "subject": "Acme Family Trust",
      "description": "$50M multi-generational family trust",
      "constraints": {
        "income_mandate": "4% annual distribution to beneficiaries",
        "risk_tolerance": "Moderate (fiduciary duty)",
        "time_horizon": "20+ years",
        "current_allocation": "60% equities, 30% fixed income, 10% alternatives"
      },
      "situation": "Trust holds $2.1M in NVAI (NVIDIA preferred, 3.2% yield). Proposal: swap NVAI for NVDA (common, 0% dividend). NVDA +180% YTD. Key tension: NVDA pays no dividend, conflicting with 4% income mandate."
    }
  },

  // Calibration (RFC 0054) — only present if calibrated: true
  "calibration": {
    "ethos_id": "ethos-nvidia-001",
    "domain": "Fiduciary Investment Analysis",
    "rules": [
      { "seq": 1, "type": "principle", "label": "Evidence must be citable", "priority": 100 },
      { "seq": 2, "type": "tenet", "label": "Fiduciary duty supersedes growth", "priority": 1000 },
      { "seq": 3, "type": "constraint", "label": "4% annual income required", "priority": 500 }
    ],
    "prompt_injection": "## Calibration: Fiduciary Investment Analysis\n\n..."
  },

  "prior_rounds": [
    {
      "round": 0,
      "score": 117,
      "expert_contributions": [
        {
          "expert": "muffin",
          "role": "Value Analyst",
          "perspectives": [
            { "id": "P0001", "label": "Income mandate mismatch", "status": "open", "content": "NVIDIA's zero dividend directly conflicts with the trust's 4% income requirement..." }
          ],
          "tensions_raised": ["T0001"],
          "recommendations": []
        },
        {
          "expert": "cupcake",
          "role": "Risk Manager",
          "perspectives": [
            { "id": "P0002", "label": "Concentration risk", "status": "open", "content": "Adding NVDA increases semiconductor exposure to 23% of portfolio..." }
          ],
          "tensions_raised": ["T0002"],
          "recommendations": []
        },
        {
          "expert": "donut",
          "role": "Options Strategist",
          "perspectives": [
            { "id": "P0003", "label": "Options overlay opportunity", "status": "open", "content": "A covered call strategy on NVDA could generate 18-34% annualized income..." }
          ],
          "tensions_raised": [],
          "recommendations": [
            { "id": "R0001", "label": "Income Collar Structure", "content": "Strike Selection Framework...", "parameters": { "covered_call_delta": "0.20-0.25" } }
          ]
        }
      ],
      "tensions": [
        { "id": "T0001", "label": "Growth vs income", "expert": "muffin", "status": "open", "description": "NVIDIA's zero dividend conflicts with 4% income mandate" },
        { "id": "T0002", "label": "Concentration risk", "expert": "cupcake", "status": "addressed", "description": "Semiconductor exposure would reach 23%" }
      ],
      "judge_synthesis": "11-1 split: majority concerns about income mandate..."
    }
  ],

  "active_tensions": [
    { "id": "T0001", "label": "Growth vs income", "status": "open" },
    { "id": "T0003", "label": "Volatility drag", "status": "open" }
  ],

  // Per-expert data keyed by slug
  "experts": {
    "muffin": {
      "slug": "muffin",
      "role": "Value Analyst",
      "tier": "Core",
      "source": "retained",
      "focus": "Intrinsic value, margin of safety",
      "description": "You evaluate investments through the lens of intrinsic value...",
      "your_score": 12,
      "round_context": "In round 0, you raised T0001 (income mandate mismatch) which remains open. Donut proposed an options overlay that may address your concern."
    },
    "cupcake": {
      "slug": "cupcake",
      "role": "Risk Manager",
      "tier": "Core",
      "source": "retained",
      "focus": "Downside scenarios, tail events",
      "description": "You identify and quantify risks...",
      "your_score": 8,
      "round_context": "Your T0002 (concentration risk) was addressed last round..."
    },
    "eclair": {
      "slug": "eclair",
      "role": "Tax Strategist",
      "tier": "Adjacent",
      "source": "pool",     // From initial pool, not previously active - Judge writes context brief
      "focus": "Tax implications, harvest strategies",
      "description": "You analyze tax consequences...",
      "your_score": 0,
      "round_context": null
    },
    "scone": {
      "slug": "scone",
      "role": "Supply Chain Analyst",
      "tier": "Wildcard",
      "source": "created",  // Invented by Judge - Judge writes context brief + mandate
      "focus": "Supply chain dependencies, geopolitical risk",
      "description": "You analyze supply chain vulnerabilities...",
      "your_score": 0,
      "round_context": null,
      "creation_reason": "Address T0003 supply chain concerns"
    }
  }
}
```

**One call returns context for entire panel.** Judge builds prompts in parallel from shared + expert-specific data.

**MCP provides structured data. Judge adds value through:**
- Emphasis and framing based on dialogue dynamics
- Custom `round_context` for each expert's situation
- Context briefs for fresh experts (constructed from structured data)

**For fresh experts** (pool or created):
- Judge writes a context brief from `prior_rounds` data, highlighting relevant prior discussion
- `prior_rounds` contains **full content** from all experts in all prior rounds (same as retained experts)
- The brief orients them; the full content lets them engage deeply
- `created` experts additionally receive a mandate explaining why they were invented

**Content availability guarantee:**
- All agents receive structured `perspectives`, `recommendations`, `tensions`, `evidence`, `claims` with full `content` field
- All IDs use global format (e.g., `P0001`, `R0001`, `T0001`) — no local ID confusion
- No content is hidden or summarized-only
- Fresh experts and retained experts see the same prior round data

*Note: Raw markdown responses are stored in `experts.raw_content` JSON for debugging but not included in agent context to avoid local/global ID confusion.*

### Prompt Assembly (Judge Responsibility)

The Judge fetches context in bulk and builds prompts in parallel:

```
1. FETCH (single call)
   └─ Judge calls blue_dialogue_round_context(dialogue_id, round)
   └─ MCP returns context for ALL panel experts

2. BUILD + WRITE (parallel for each expert)
   └─ Judge synthesizes markdown prompt from shared + expert data
   └─ Judge writes prompt-{expert}.md and context-{expert}.json

3. SPAWN (parallel - one message, multiple Task calls)
   └─ Judge spawns ALL agents with prompt + alignment-expert skill

4. RESPOND (parallel - agents run concurrently)
   └─ Each agent writes response-{expert}.md
```

**Why Judge builds prompts (not MCP):**
- Judge can emphasize specific tensions based on dialogue dynamics
- Judge can customize `round_context` per expert's situation
- Judge can frame fresh expert mandates appropriately
- MCP stays focused on data access, not LLM prompt engineering

**Parallelization benefits:**
- Single MCP call instead of N calls (reduces latency)
- Parallel file writes (no serialization)
- Parallel agent spawning (all in one message)
- Agents run concurrently (no first-mover advantage)

**Filesystem artifacts (for human debugging):**

| File | Written by | Purpose |
|------|------------|---------|
| `prompt-{expert}.md` | Judge | Exact markdown prompt agent received |
| `context-{expert}.json` | Judge | Structured JSON data used to build prompt |
| `response-{expert}.md` | Agent | Agent's deliberation response |

**Prompt format:**

```markdown
# Dialogue: NVIDIA Investment Analysis
**Question:** Should Acme Trust swap its NVAI position for NVDA shares?
**Round:** 1 of ? | **Status:** open | **Total ALIGNMENT:** 117

---

## Background

**Acme Family Trust** is a $50M multi-generational family trust.

| Constraint | Requirement |
|------------|-------------|
| Income mandate | 4% annual distribution to beneficiaries |
| Risk tolerance | Moderate (fiduciary duty) |
| Time horizon | 20+ years |
| Current allocation | 60% equities, 30% fixed income, 10% alternatives |

**Current situation:**
- Trust holds $2.1M in NVAI (NVIDIA preferred, 3.2% yield)
- Proposal: Swap NVAI → NVDA (common shares, 0% dividend)
- NVDA has appreciated 180% YTD; NVAI underperformed
- Key tension: NVDA pays no dividend, conflicting with 4% income mandate

---

## Your Role

**Muffin** 🧁 | Value Analyst | Core
Focus: Intrinsic value, margin of safety
Your score: 12 | Source: retained

You evaluate investments through the lens of intrinsic value, seeking companies
trading below fair value with adequate margin of safety. You are skeptical of
momentum-driven narratives and prioritize cash flow analysis, balance sheet
strength, and sustainable competitive advantages.

### Your Task This Round

In round 0, you raised T0001 (income mandate mismatch) which remains open. Donut
proposed an options overlay that may address your concern. Your task this round:
evaluate whether the options strategy adequately resolves the income gap, or
articulate why it falls short.

---

## Prior Rounds

### Round 0 — Opening Arguments (Score: 117)

#### Muffin (You) — Value Analyst
[P0001: Income mandate mismatch] (muffin)
NVIDIA's zero dividend directly conflicts with the trust's 4% income requirement...

[TENSION T0001: Growth vs income]
The fundamental tension between growth exposure and income mandate.

---

#### Cupcake — Risk Manager
[P0002: Concentration risk] (cupcake)
Adding NVDA increases semiconductor exposure to 23% of portfolio...

[TENSION T0002: Concentration risk]
Portfolio concentration in semiconductors exceeds policy limits.

---

#### Donut — Options Strategist
[P0003: Options overlay opportunity] (donut)
A covered call strategy on NVDA could generate 18-34% annualized income...

[R0001: Income Collar Structure] (donut)
Strike Selection Framework:
| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Covered call delta | 0.20-0.25 | Balance premium vs upside |
| Protective put delta | -0.15 | Tail risk protection |
| DTE | 30-45 | Optimal theta decay |

---

### Judge Synthesis — Round 0
11-1 split: majority concerns about income mandate, but Donut's options overlay
opens a potential path forward. Key tensions T0001 and T0002 require resolution.

---

## Active Tensions

| ID | Description | Origin | Status | Your Involvement |
|----|-------------|--------|--------|------------------|
| T0001 | Growth vs income mandate | Muffin | open | originator |
| T0002 | Concentration risk | Cupcake | addressed | — |
| T0003 | Volatility drag on returns | Scone | open | — |

---

## Instructions

You are Muffin 🧁, a Value Analyst participating in round 1 of this ALIGNMENT dialogue.

Your contribution is scored on PRECISION, not volume. One sharp insight beats ten paragraphs.

Structure your response using the markers from your skill (P, R, T, E, C for perspectives, recommendations, tensions, evidence, claims; plus cross-references and moves). Use your expert slug in local IDs: `MUFFIN-P0101`, `MUFFIN-R0101`, `MUFFIN-T0101`, etc.

Focus on: What has changed since round 0? Which tensions can you help resolve?
```

### Alignment Expert Skill

Agents are spawned with the `alignment-expert` skill, which contains **static** marker syntax reference. This avoids repeating ~800 tokens of marker documentation in every prompt.

**Skill file:** `~/.claude/skills/alignment-expert/SKILL.md`

```markdown
# Alignment Expert

You are an expert participating in an ALIGNMENT dialogue. Structure your
response using these markers.

## Local ID Format

Use your expert slug (UPPERCASE) with type prefix and 4-digit round+seq:
- `{EXPERT}-P{round:02d}{seq:02d}` — Perspective (e.g., MUFFIN-P0101)
- `{EXPERT}-R{round:02d}{seq:02d}` — Recommendation (e.g., MUFFIN-R0101)
- `{EXPERT}-T{round:02d}{seq:02d}` — Tension (e.g., MUFFIN-T0101)
- `{EXPERT}-E{round:02d}{seq:02d}` — Evidence (e.g., MUFFIN-E0101)
- `{EXPERT}-C{round:02d}{seq:02d}` — Claim (e.g., MUFFIN-C0101)

## First-Class Entities

| Marker | Purpose |
|--------|---------|
| `[{local_id}: label]` | Type encoded in ID prefix (P, R, T, E, C) |

Examples:
- `[MUFFIN-P0101: Income mandate mismatch]` — Perspective
- `[DONUT-R0101: Options collar structure]` — Recommendation
- `[MUFFIN-T0101: Growth vs income]` — Tension
- `[MUFFIN-E0101: Historical IV data]` — Evidence
- `[MUFFIN-C0101: Income gap bridgeable]` — Claim

## Cross-References

Link to prior items (global IDs) or same-round items (local IDs):

| Marker | Purpose |
|--------|---------|
| `[RE:SUPPORT {id}]` | Endorse referenced item |
| `[RE:OPPOSE {id}]` | Challenge referenced item |
| `[RE:REFINE {id}]` | Build on and improve (same type only) |
| `[RE:ADDRESS {id}]` | Propose solution to tension |
| `[RE:RESOLVE {id}]` | Claim tension is fully resolved |
| `[RE:REOPEN {id}]` | Argue resolved tension needs revisiting |
| `[RE:QUESTION {id}]` | Ask for clarification |
| `[RE:DEPEND {id}]` | This item depends on referenced item |

**Constraints:**
- `RE:RESOLVE`, `RE:REOPEN`, `RE:ADDRESS` — must target Tensions (T)
- `RE:REFINE` — must target same entity type (P→P, R→R, etc.)

## Dialogue Moves

| Marker | Purpose |
|--------|---------|
| `[MOVE:DEFEND {id}]` | Strengthen case for item |
| `[MOVE:CHALLENGE {id}]` | Question assumptions |
| `[MOVE:BRIDGE {id1} {id2}]` | Synthesize into common ground |
| `[MOVE:REQUEST {topic}]` | Ask panel for input |
| `[MOVE:CONCEDE {id}]` | Withdraw position in favor of item |
| `[MOVE:CONVERGE]` | Signal readiness to conclude |

## Verdicts

| Marker | Purpose |
|--------|---------|
| `[DISSENT]` | Formal objection with reasoning |
| `[MINORITY VERDICT: label]` | Alternative recommendation from coalition |

## Output Rules

1. Write response to: `{output_dir}/round-{N}/response-{expert_slug}.md`
2. Return structured summary to Judge (perspectives, recommendations, tensions, evidence, claims, refs)
3. One sharp insight beats ten paragraphs — PRECISION over volume
```

**Judge spawns agents with:**

```python
spawn_agent(
    skill="alignment-expert",  # Static marker syntax
    prompt=full_markdown_prompt  # Dynamic context (Judge-built)
)
```

**Separation of concerns:**

| Content | Source | Rationale |
|---------|--------|-----------|
| Marker syntax | Skill | Static, same for all agents, ~800 tokens |
| Dialogue background | Prompt | Dynamic, dialogue-specific |
| Expert role/mandate | Prompt | Dynamic, agent-specific |
| Prior rounds | Prompt | Dynamic, round-specific |
| Active tensions | Prompt | Dynamic, changes each round |
| Instructions | Prompt | Dynamic, may vary by expert source |

**Why Judge passes prompt directly (not file path):**
- Simpler — no file read step for agent
- Fewer failure modes — no file access issues
- Prompt is already in Judge's memory
- Filesystem write is for human debugging only

### Dynamic Expert Creation

The Judge can **create new experts at any point** before a round to address emerging needs:

**MCP tool:** `blue_dialogue_expert_create`

```json
{
  "dialogue_id": "nvidia-decision",
  "expert_slug": "palmier",
  "role": "Geopolitical Risk Analyst",
  "description": "You analyze investments through the lens of geopolitical risk, focusing on supply chain vulnerabilities, regulatory exposure, and country-specific risks. You are particularly attentive to concentration risk in politically sensitive regions.",
  "focus": "Taiwan semiconductor concentration, export controls",
  "tier": "Adjacent",
  "reason": "T0003 (supply chain concentration) requires specialized geopolitical expertise not present in current panel"
}
```

**When to create experts:**
- Existing panel lacks expertise to resolve a specific tension
- New deliberation focus emerges that needs dedicated attention
- Wildcard injection to challenge emerging consensus
- Domain shift mid-dialogue (e.g., legal question surfaces)

**What happens:**
1. Judge calls `blue_dialogue_expert_create` before the round
2. MCP inserts expert into `experts` table with `source: "created"`
3. Expert is included in next `blue_dialogue_evolve_panel` call
4. Judge constructs context brief from `blue_dialogue_round_context` structured data

**Created experts receive (assembled by Judge):**
- Full `prior_rounds` content (same as retained experts)
- Judge-written context brief summarizing relevant discussion and why they were brought in
- `round_context` focused on their specific mandate

**Example context brief for created expert:**

```markdown
## Context Brief (You are joining in Round 2)

You were brought in to address **T0003: Supply chain concentration risk**.

---

### The Dialogue Question

**Should Acme Family Trust swap its NVAI position for NVDA shares?**

### Background: Acme Family Trust

Acme Family Trust is a $50M multi-generational family trust with the following
constraints:

| Constraint | Requirement |
|------------|-------------|
| **Income mandate** | 4% annual distribution to beneficiaries |
| **Risk tolerance** | Moderate (fiduciary duty to preserve capital) |
| **Time horizon** | 20+ years (multi-generational) |
| **Current allocation** | 60% equities, 30% fixed income, 10% alternatives |

**Current situation:**
- Trust holds $2.1M in NVAI (NVIDIA preferred shares, 3.2% yield)
- Proposal: Swap NVAI for NVDA (common shares, 0% dividend)
- NVDA has appreciated 180% YTD; NVAI has underperformed
- Trustee believes NVDA growth potential outweighs income loss

**Key tension:** NVDA pays no dividend, conflicting with 4% income mandate.

---

### Why You're Here

After two rounds of deliberation, the income mandate concerns (T0001) have been
resolved via an options overlay strategy. However, a critical tension remains
unaddressed: **geographic concentration risk in semiconductor supply chain**.

No current panelist has geopolitical risk expertise, so you were created to
provide this specialized perspective.

### Your Mandate

1. Evaluate the geopolitical risk of Taiwan semiconductor concentration
2. Quantify probability and impact scenarios for supply disruption
3. Propose risk mitigation strategies if viable
4. If risks are unacceptable, strengthen the case for rejection

---

### Key Context: The Tension You're Addressing

**T0003: Supply chain concentration risk** (raised by Cupcake, Round 0)
> "NVIDIA's 87% dependency on TSMC Taiwan fabrication represents unhedgeable
> tail risk. A Taiwan Strait crisis would eliminate access to advanced chips
> with no alternative supplier capable of 3nm production."

### Relevant Prior Perspectives

**P0003** (Cupcake, Risk Manager, Round 0):
> "Taiwan exposure represents unhedgeable tail risk. Unlike financial risks
> that can be managed through derivatives, geopolitical supply disruption has
> no market hedge. The 87% TSMC concentration means a single point of failure
> for NVIDIA's entire product line. Historical precedent: the 2021 chip
> shortage caused 18-month delivery delays even without military conflict."

**P0005** (Scone, Supply Chain Analyst, Round 1):
> "TSMC has no viable alternative for 3nm production. Intel Foundry Services
> won't reach parity until 2027 at earliest. Samsung's 3nm yields remain
> below 60%. NVIDIA cannot diversify its fab dependency in the investment
> horizon we're considering. The concentration is structural, not strategic."

**P0008** (Donut, Options Strategist, Round 1):
> "Geopolitical risk cannot be hedged with options. While we can manage
> volatility and income through derivatives, there is no instrument that
> protects against a binary supply disruption event. This is outside my
> domain—we need specialized geopolitical analysis."

---

### Dialogue Progress

- **Round 0 (Score: 117):** Panel identified income mandate conflict and
  concentration risks. 11-1 against full position.
- **Round 1 (Score: 45):** Options overlay resolved income concerns. T0001
  addressed. T0003 flagged as requiring specialized expertise.
- **Current ALIGNMENT:** 162 (velocity decreasing, approaching convergence
  pending T0003 resolution)

### What the Panel Needs From You

The panel is close to convergence on a conditional approval, but T0003 blocks
final consensus. Your analysis will determine whether:
- The geopolitical risk is acceptable with conditions → conditional approval
- The risk is unacceptable → strengthen rejection case
- The risk can be mitigated → propose specific measures
```

**Context brief requirements (Judge responsibility):**

The context brief the Judge writes for fresh experts (pool or created) MUST include:

1. **The dialogue question** — What is being decided?
2. **Foundational context** — Background, constraints, current situation (everything an expert needs to understand the problem domain)
3. **Why they're here** — What gap they fill or tension they address
4. **Their mandate** — Specific responsibilities and expected outputs
5. **Relevant prior work** — Key perspectives, recommendations, tensions, evidence, claims related to their mandate
6. **Dialogue progress** — Round summaries, current ALIGNMENT, velocity

Fresh experts must be **fully self-sufficient** after reading their context. They should never need to ask "what is this dialogue about?" or "what are the constraints?"

*Note: Fresh experts (pool or created) also receive the full `prior_rounds` data (same as retained experts), so they can review all expert contributions in detail. The context brief orients them to the dialogue and highlights the most relevant prior work.*

**Expert sources in panel evolution:**

| Source | Meaning | Context |
|--------|---------|---------|
| `retained` | Continuing from prior round | Full history, their contributions highlighted |
| `pool` | Drawn from initial expert pool (not previously active) | Full history + Judge-written brief |
| `created` | Invented by Judge for this dialogue (not in pool) | Full history + Judge-written brief + mandate |

### Judge Workflow (for skill documentation)

**Round execution (maximally parallelized):**

```
0. EVOLVE PANEL (Judge discretion)
   └─ Judge reviews prior round results, open tensions, dialogue progress
   └─ Judge decides panel composition for this round:
      - Retain high-performing experts
      - Draw from pool to address gaps
      - Create new experts for specialized needs
   └─ Judge calls blue_dialogue_evolve_panel(dialogue_id, round, panel)
      → MCP records panel composition with source for each expert

1. FETCH CONTEXT (single bulk call)
   └─ Judge calls blue_dialogue_round_context(dialogue_id, round)
      → MCP returns context for ALL panel experts in one response
      → Shared data: dialogue, background, prior_rounds, active_tensions
      → Per-expert data: role, focus, source, round_context

2. BUILD PROMPTS + WRITE FILES (parallel)
   └─ For fresh experts (pool or created): Judge writes context brief from structured data
   └─ For each expert IN PARALLEL:
      └─ Judge synthesizes markdown prompt from shared + expert data
      └─ Judge writes prompt-{expert}.md (parallel file writes)
      └─ Judge writes context-{expert}.json (parallel file writes)

3. SPAWN AGENTS (parallel)
   └─ Judge spawns ALL agents in ONE message with multiple Task tool calls:
      - skill: alignment-expert (static marker syntax)
      - prompt: full markdown content (from step 2)
   └─ Agents deliberate using prompt + skill
   └─ Agents write responses with LOCAL IDs: [MUFFIN-P0101]
   └─ Each agent writes to: {output_dir}/round-{N}/response-{expert_slug}.md
   └─ Each agent returns summary to Judge

4. COLLECT AGENT OUTPUTS
   └─ Judge receives agent return values (summaries of what each contributed)
   └─ Judge reads full responses from {output_dir}/round-{N}/response-*.md
   └─ Judge reviews all contributions, identifies:
      - New perspectives (with local IDs like MUFFIN-P0101)
      - New proposals (with local IDs like DONUT-R0101)
      - New tensions (with local IDs like CUPCAKE-T0101)
      - Tension updates (addressed, resolved)
      - Cross-references between experts

5. SCORE AND REGISTER
   └─ Judge scores each expert's contribution (ALIGNMENT dimensions)
   └─ Judge synthesizes similar contributions (multiple experts → one record)
   └─ Judge calls blue_dialogue_round_register({
        round, score, expert_scores,
        perspectives, recommendations, tensions, evidence, claims,
        tension_updates
      })
      → MCP assigns global IDs (P0101, R0101, T0101)
      → Returns ID mapping for Judge's records

6. CHECK CONVERGENCE
   └─ If converging: blue_dialogue_verdict_register(...) after step 5
   └─ If continuing: goto step 0 for next round (panel evolution)
```

**Agent output format:**

Each agent writes a markdown file with structured markers:
```markdown
# Muffin - Value Analyst (Round 1)

[MUFFIN-P0101: Options viability confirmed]
The 30-delta covered call strategy can generate 18-34% annualized income,
effectively bridging the income mandate gap...

[RE: DONUT-R0001]
The collar structure addresses my T0001 concern. I support adoption with
the 45 DTE modification.

[MUFFIN-T0102: Execution timing risk]
Post-refinancing window creates a 60-90 day constraint that may conflict
with optimal entry points...
```

**Agent return value:**

Agents return a structured summary to the Judge:
```json
{
  "expert_slug": "muffin",
  "round": 1,
  "response_path": "/tmp/blue-dialogue/nvidia-decision/round-1/response-muffin.md",
  "summary": {
    "perspectives": ["MUFFIN-P0101"],
    "recommendations": [],
    "tensions_raised": ["MUFFIN-T0101"],
    "evidence": ["MUFFIN-E0101"],
    "claims": ["MUFFIN-C0101"],
    "references": [
      { "type": "refine", "from": "MUFFIN-P0101", "target": "P0001" },
      { "type": "support", "from": "MUFFIN-P0101", "target": "R0001" },
      { "type": "address", "from": "MUFFIN-P0101", "target": "T0001" },
      { "type": "support", "from": "MUFFIN-E0101", "target": "MUFFIN-P0101" },
      { "type": "depend", "from": "MUFFIN-C0101", "target": "MUFFIN-P0101" }
    ],
    "moves": [
      { "type": "bridge", "targets": ["P0003", "R0001"] }
    ]
  }
}
```

**Why Judge builds prompts (not MCP or agents):**
- Judge can customize emphasis based on dialogue dynamics
- Judge controls framing for fresh vs retained experts
- Judge writes context_brief with specific mandates for created experts
- MCP stays focused on data access, not prompt engineering

**Key points for alignment-play skill:**
- Judge fetches data via `blue_dialogue_round_context`, builds prompts
- Judge writes prompts to filesystem for debugging
- Judge spawns agents with full prompt + `alignment-expert` skill
- Agents write local IDs — Judge maps to global IDs during registration
- Use `blue_dialogue_round_register` for bulk operations (faster)

### Data Registration Model

**All dialogue data is registered via MCP tools as the dialogue progresses.** The Judge registers data after each round:

```
Round 0 completes:
  → Judge scores responses
  → Judge calls blue_dialogue_round_register with all round data:
    - perspectives, recommendations, tensions, evidence, claims
    - cross-references (refs)
    - tension_updates
    - expert_scores

Round N completes (convergence):
  → Judge calls blue_dialogue_round_register (same as every round)
  → Judge calls blue_dialogue_verdict_register for majority verdict
  → Judge identifies dissenters from final round contributions
  → Judge calls blue_dialogue_verdict_register for each minority verdict (on behalf of dissenting experts)
```

**Benefits:**
- Export is a simple DB query, no file parsing
- Data is queryable in real-time during dialogue
- Single source of truth (database, not scattered files)
- Files (`.md`) become human-readable artifacts, not data sources

### Export Tooling: `blue_dialogue_export`

The MCP tool queries the database and generates a single JSON file:

```
blue_dialogue_export(dialogue_id="nvidia-decision", output_path="/path/to/nvidia-decision.json")
  → Single file with all dialogue data
```

#### Data Sources (all from SQLite)

| Table | Export Section |
|-------|----------------|
| `dialogues` | Root metadata, status, alignment total |
| `experts` | `experts[]` with `scores` and `raw_content` JSON (per-round) |
| `expert_pool` | `expert_pool` (original pool definition) |
| `rounds` | `rounds[]` metadata |
| `perspectives` | `perspectives[]` with events |
| `proposals` | `proposals[]` with events |
| `tensions` | `tensions[]` with events |
| `evidence` | `evidence[]` with events |
| `claims` | `claims[]` with events |
| `refs` | Embedded in each entity's `references[]` array |
| `moves` | `moves[]` (dialogue moves: defend, challenge, bridge, etc.) |
| `tension_events` | Embedded in tension `events[]` |
| `verdicts` | `verdicts[]` |

#### Output: Single `dialogue.json`

```json
{
  "id": "nvidia-decision",
  "title": "NVIDIA Investment Decision",
  "question": "Should Acme Trust add NVIDIA?",
  "date": "2026-02-02",
  "status": "converged",
  "totalRounds": 3,
  "totalAlignment": 423,

  "expert_pool": {
    "domain": "Investment Analysis",
    "experts": [
      { "role": "Value Analyst", "tier": "Core", "relevance": 0.95 },
      { "role": "Risk Manager", "tier": "Core", "relevance": 0.90 }
    ]
  },

  "experts": [
    {
      "slug": "muffin",
      "role": "Value Analyst",
      "tier": "Core",
      "source": "pool",
      "scores": { "0": 12, "1": 8, "2": 15 },
      "total": 35,
      "color": "#3B82F6"
    },
    {
      "slug": "palmier",
      "role": "Geopolitical Risk Analyst",
      "tier": "Adjacent",
      "source": "created",
      "creationReason": "T0003 requires geopolitical expertise",
      "scores": { "2": 14 },
      "total": 14,
      "color": "#10B981"
    }
  ],

  "rounds": [
    {
      "round": 0,
      "title": "Opening Arguments",
      "score": 117,
      "summary": "11-1 split on full swap vs conditional support",
      "experts": {
        "muffin": {
          "score": 12,
          "raw": "[MUFFIN-P0001: Income mandate mismatch]\nNVIDIA's zero dividend...",
          "mapping": { "MUFFIN-P0001": "P0001", "MUFFIN-T0001": "T0001" }
        },
        "cupcake": {
          "score": 10,
          "raw": "[CUPCAKE-P0001: Concentration risk]\n...",
          "mapping": { "CUPCAKE-P0001": "P0002" }
        }
      }
    },
    {
      "round": 1,
      "title": "Refinement",
      "score": 45,
      "summary": "Options overlay gains traction, T0001 addressed",
      "experts": {
        "muffin": {
          "score": 8,
          "raw": "[MUFFIN-P0101: Options viability confirmed]\n...",
          "mapping": { "MUFFIN-P0101": "P0101" }
        }
      }
    }
  ],

  "perspectives": [
    {
      "id": "P0001",
      "label": "Income mandate mismatch",
      "content": "NVIDIA's zero dividend directly conflicts with the trust's 4% income requirement...",
      "contributors": ["muffin"],
      "round": 0,
      "status": "refined",
      "references": [],
      "events": [
        { "type": "created", "round": 0, "by": ["muffin"] },
        { "type": "refined", "round": 1, "by": ["muffin"], "result": "P0101" }
      ]
    },
    {
      "id": "P0101",
      "label": "Options viability confirmed",
      "content": "The 30-delta covered call strategy can generate 18-34% annualized income...",
      "contributors": ["muffin"],
      "round": 1,
      "status": "open",
      "references": [
        { "type": "refine", "target": "P0001" },
        { "type": "support", "target": "R0001" },
        { "type": "address", "target": "T0001" }
      ],
      "events": [
        { "type": "created", "round": 1, "by": ["muffin"] }
      ]
    }
  ],

  "recommendations": [
    {
      "id": "R0001",
      "label": "Income Collar Structure",
      "content": "Strike Selection Framework for synthetic income generation...",
      "contributors": ["donut"],
      "round": 0,
      "status": "adopted",
      "parameters": {
        "covered_call_delta": "0.20-0.25",
        "protective_put_delta": "-0.15",
        "dte": "30-45"
      },
      "references": [
        { "type": "address", "target": "T0001" },
        { "type": "depend", "target": "P0001" }
      ],
      "adoptedInVerdict": "final",
      "events": [
        { "type": "created", "round": 0, "by": ["donut"] },
        { "type": "adopted", "round": 2, "by": ["judge"], "reference": "final" }
      ]
    }
  ],

  "tensions": [
    {
      "id": "T0001",
      "label": "Growth vs income obligation",
      "description": "NVIDIA's zero dividend conflicts with trust's 4% income requirement",
      "contributors": ["muffin"],
      "round": 0,
      "status": "resolved",
      "references": [
        { "type": "depend", "target": "P0001" }
      ],
      "events": [
        { "type": "created", "round": 0, "by": ["muffin"] },
        { "type": "addressed", "round": 1, "by": ["donut"], "reference": "R0001" },
        { "type": "resolved", "round": 2, "by": ["muffin"], "reference": "P0101" }
      ]
    }
  ],

  "evidence": [
    {
      "id": "E0101",
      "label": "Historical options premium data",
      "content": "NVDA 30-day ATM IV averaged 45% over past 24 months. 30-delta calls yielded 2.1-2.8% monthly premium. Premium remained viable during Q4 2022 drawdown.",
      "contributors": ["muffin"],
      "round": 1,
      "status": "confirmed",
      "references": [
        { "type": "support", "target": "P0101" }
      ],
      "events": [
        { "type": "cited", "round": 1, "by": ["muffin"] },
        { "type": "confirmed", "round": 2, "by": ["donut"], "reference": "R0001" }
      ]
    }
  ],

  "claims": [
    {
      "id": "C0101",
      "label": "Income mandate resolved",
      "content": "The income mandate objection is resolved via options overlay. Remaining concerns are execution timing and concentration—both manageable with phased approach.",
      "contributors": ["muffin"],
      "round": 1,
      "status": "adopted",
      "references": [
        { "type": "depend", "target": "P0101" },
        { "type": "depend", "target": "E0101" }
      ],
      "events": [
        { "type": "asserted", "round": 1, "by": ["muffin"] },
        { "type": "supported", "round": 2, "by": ["donut", "cupcake"] },
        { "type": "adopted", "round": 2, "by": ["judge"], "reference": "final" }
      ]
    }
  ],

  "moves": [
    {
      "expert": "muffin",
      "round": 1,
      "type": "bridge",
      "targets": ["P0003", "R0001"],
      "context": "Reconciling concentration concern with collar proposal via phased entry"
    },
    {
      "expert": "donut",
      "round": 1,
      "type": "defend",
      "targets": ["R0001"],
      "context": "NVDA options liquidity supports execution viability"
    },
    {
      "expert": "cupcake",
      "round": 2,
      "type": "concede",
      "targets": ["P0003"],
      "context": "Phased entry adequately addresses concentration concern"
    },
    {
      "expert": "muffin",
      "round": 2,
      "type": "converge",
      "targets": [],
      "context": "Ready to conclude with conditional approval"
    }
  ],

  "verdicts": [
    {
      "id": "final",
      "type": "final",
      "round": 2,
      "author": null,
      "recommendation": "REJECT full swap. APPROVE conditional partial trim.",
      "description": "The panel unanimously rejected a full NVAI-to-NVDA swap...",
      "conditions": [
        "Execute 60-90 days post-refinancing",
        "Implement 30-delta covered calls at 45 DTE"
      ],
      "tensionsResolved": ["T0001", "T0002"],
      "tensionsAccepted": ["T0006"],
      "recommendationsAdopted": ["R0001"],
      "keyEvidence": ["E0101"],
      "keyClaims": ["C0101"],
      "vote": "12-0",
      "confidence": "unanimous"
    }
  ]
}
```

**Key sections:**

| Section | Contents |
|---------|----------|
| `expert_pool` | Original pool definition (for reference) |
| `experts[]` | Participating experts with per-round scores |
| `rounds[]` | Round metadata + raw expert content with local→global ID mapping |
| `perspectives[]` | Positions and arguments with `references[]` and events |
| `proposals[]` | Actionable recommendations with parameters, `references[]`, and events |
| `tensions[]` | Conflicts and risks with `references[]`, lifecycle, and events |
| `evidence[]` | Data and precedents with `references[]` and events |
| `claims[]` | Quotable position statements with `references[]` and events |
| `moves[]` | Dialogue moves (defend, challenge, bridge, concede, converge) |
| `verdicts[]` | All verdicts (interim, final, minority, dissent) |

#### Perspective Status Tracking

The export tracks perspective lifecycle across rounds:

| Status | Meaning |
|--------|---------|
| `open` | No engagement yet |
| `refined` | Expert evolved their own perspective |
| `conceded` | Expert withdrew after peer challenge |
| `resolved` | Tension addressed, perspective incorporated |

#### Verdict Structure

Dialogues support **multiple verdicts** to capture interim decisions, final conclusions, and minority positions:

```json
{
  "verdicts": [
    {
      "id": "string",                    // "final", "V01", "minority-esg"
      "type": "final",                   // interim | final | minority | dissent
      "round": 3,                        // Round when issued
      "author": "string | null",         // Expert slug, or null for Judge
      "recommendation": "string",        // One-line decision
      "description": "string",           // 2-3 sentence reasoning
      "conditions": ["string"],          // Required conditions for approval
      "tensions_resolved": ["T0001"],      // Tensions addressed by this verdict
      "tensions_accepted": ["T0006"],      // Tensions acknowledged but not blocking
      "recommendations_adopted": ["R0001"],  // Recommendations incorporated
      "supporting_experts": ["churro"],  // For minority/dissent types
      "vote": "11-1",                    // Vote tally
      "confidence": "strong"             // unanimous | strong | split | contested
    }
  ]
}
```

**Verdict types:**

| Type | When Used | Author |
|------|-----------|--------|
| `interim` | Mid-dialogue checkpoint, steering decision | Judge |
| `final` | Authoritative conclusion at convergence | Judge |
| `minority` | Structured dissent from expert subset | Lead dissenting expert |
| `dissent` | Single expert's formal objection | Dissenting expert |

**Registered via MCP** using `blue_dialogue_verdict_register`:
- Judge registers `interim` and `final` verdicts
- Experts can signal dissent to the Judge, who registers `minority` or `dissent` verdicts on their behalf
- All verdicts are immutable once registered (audit trail)

**Populated from:**
- `[VERDICT]` markers in round summaries (Judge)
- `[DISSENT]` markers in expert responses
- `[MINORITY VERDICT]` markers from expert coalitions
- Resolved/accepted tension status at time of verdict

#### Global ID Assignment

Global IDs are assigned at **registration time**, not export time:

1. Judge calls `blue_dialogue_round_register` with all round data
2. MCP server assigns global `seq` for each entity within `(dialogue_id, round)`
3. Display ID derived: `P{round:02d}{seq:02d}`, `R{round:02d}{seq:02d}`, etc.
4. Export queries database with IDs already assigned

**No renumbering needed** - the database is the source of truth for all IDs.

#### Validation & Warnings

The export validates database consistency:
- Missing expert scores (expert in panel but no score for round)
- Unresolved tensions at convergence
- Orphaned perspectives (no matching expert registration)
- Verdict without required fields

```
⚠️  Warnings:
   missing_score: scone, round 2
      Expert was in panel but has no score registered
   unresolved_tension: T0006
      Dialogue converged with open tension (accepted)
   verdict_incomplete: final
      Missing tensions_resolved field
```

### Two-Phase ID Assignment

**Phase 1: Agents write with namespaced local IDs**

Agents use expert-prefixed local IDs in their output:
```markdown
[MUFFIN-P0001: Income mandate mismatch]
[DONUT-R0001: Options collar structure]
[MUFFIN-T0001: Growth vs income]
```

These are local to the agent's response. The slug prefix keeps them unique, but sequence numbers overlap across experts (e.g., both MUFFIN-P0101 and CUPCAKE-P0101 exist).

**Phase 2: Judge registers with global IDs**

When Judge registers content, the MCP assigns a **global ID**:
- `P0001`, `P0002`, `P0003`... (perspectives, globally sequential per round)
- `R0001`, `R0002`... (proposals, globally sequential per round)
- `T0001`, `T0002`... (tensions, globally sequential per round)
- `E0001`, `E0002`... (evidence, globally sequential per round)
- `C0001`, `C0002`... (claims, globally sequential per round)

The original namespaced ID is discarded. Expert attribution is stored in the record, not encoded in the ID.

**Downstream: Global IDs only**

All references after registration use global IDs:
- Cross-references: `[RE:SUPPORT E0001]`, `[RE:OPPOSE C0003]`
- Tension links: `references: [{"type": "address", "target": "T0001"}]`
- Verdicts: `recommendationsAdopted: ["R0001"]`, `keyEvidence: ["E0001"]`

### Display ID Formulas

All IDs use `{round:02d}{seq:02d}` format (4 digits):

| Entity | Formula | Examples |
|--------|---------|----------|
| Perspective | `P{round:02d}{seq:02d}` | P0001, P0102, P0215 |
| Recommendation | `R{round:02d}{seq:02d}` | R0001, R0102 |
| Tension | `T{round:02d}{seq:02d}` | T0001, T0102 |
| Evidence | `E{round:02d}{seq:02d}` | E0001, E0103 |
| Claim | `C{round:02d}{seq:02d}` | C0001, C0205 |

The format encodes round implicitly:
- `P0001` = round 0, seq 1
- `P0102` = round 1, seq 2
- `P0215` = round 2, seq 15

This gives:
- 99 rounds max (00-99)
- 99 items per entity type per round (01-99)
- Human-readable IDs with expert attribution in data, not ID

### Expert Content Markers

Experts use these markers in their responses for structured extraction.

#### First-Class Entities (Global IDs assigned by MCP)

| Marker | Local ID Example | Description |
|--------|------------------|-------------|
| `[{local_id}: label]` | `MUFFIN-P0101` | Perspective: Position or argument |
| `[{local_id}: label]` | `MUFFIN-R0101` | Recommendation: Actionable proposal |
| `[{local_id}: label]` | `MUFFIN-T0101` | Tension: Conflict, risk, or unresolved issue |
| `[{local_id}: label]` | `MUFFIN-E0101` | Evidence: Data, precedent, or fact |
| `[{local_id}: label]` | `MUFFIN-C0101` | Claim: Quotable position statement |

Type is encoded in the ID prefix: `P` (perspective), `R` (recommendation), `T` (tension), `E` (evidence), `C` (claim).

#### Status Lifecycles

| Entity | Statuses |
|--------|----------|
| Perspective | `open` → `refined` / `conceded` / `merged` |
| Recommendation | `proposed` → `amended` → `adopted` / `rejected` |
| Tension | `open` → `addressed` → `resolved` / `reopened` |
| Evidence | `cited` → `challenged` → `confirmed` / `refuted` |
| Claim | `asserted` → `supported` / `opposed` → `adopted` / `withdrawn` |

#### Cross-References

The `[RE: ...]` marker creates typed relationships between contributions:

| Syntax | Meaning | Example |
|--------|---------|---------|
| `[RE:SUPPORT {id}]` | Endorses the referenced item | `[RE:SUPPORT R0001]` |
| `[RE:OPPOSE {id}]` | Challenges or disagrees with item | `[RE:OPPOSE P0003]` |
| `[RE:REFINE {id}]` | Builds on and improves item | `[RE:REFINE P0001]` |
| `[RE:ADDRESS {id}]` | Proposes solution to tension | `[RE:ADDRESS T0001]` |
| `[RE:RESOLVE {id}]` | Claims tension is fully resolved | `[RE:RESOLVE T0001]` |
| `[RE:REOPEN {id}]` | Argues resolved tension needs revisiting | `[RE:REOPEN T0002]` |
| `[RE:QUESTION {id}]` | Asks for clarification | `[RE:QUESTION R0001]` |
| `[RE:DEPEND {id}]` | This item depends on referenced item | `[RE:DEPEND P0003]` |

**Semantic constraints (enforced by DB):**

| ref_type | Valid targets | Constraint |
|----------|---------------|------------|
| `resolve` | T only | Can only resolve a Tension |
| `reopen` | T only | Can only reopen a Tension |
| `address` | T only | Can only address a Tension |
| `refine` | same type | P→P, R→R, C→C, etc. |
| `support` | any | Flexible — endorses any entity |
| `oppose` | any | Flexible — challenges any entity |
| `question` | any | Flexible — asks clarification on any entity |
| `depend` | any | Flexible — logical dependency on any entity |

**Reference targets:**
- Global IDs for prior rounds: `P0001`, `R0001`, `T0001`, `E0001`, `C0001`
- Local IDs for same-round items: `MUFFIN-P0101`, `DONUT-E0101`, `CUPCAKE-C0101`
- Expert contributions: `@muffin` (references expert's overall position)

#### Dialogue Moves

| Marker | Description |
|--------|-------------|
| `[MOVE:DEFEND {id}]` | Strengthens case for referenced item |
| `[MOVE:CHALLENGE {id}]` | Questions assumptions in referenced item |
| `[MOVE:BRIDGE {id1} {id2}]` | Synthesizes two items into common ground |
| `[MOVE:REQUEST {topic}]` | Asks panel for specific input |
| `[MOVE:CONCEDE {id}]` | Withdraws own position in favor of referenced item |
| `[MOVE:CONVERGE]` | Signals readiness to conclude |

#### Verdicts (Judge and Expert)

| Marker | Author | Description |
|--------|--------|-------------|
| `[VERDICT:INTERIM]` | Judge | Mid-dialogue checkpoint decision |
| `[VERDICT:FINAL]` | Judge | Authoritative conclusion |
| `[DISSENT]` | Expert | Formal objection with reasoning |
| `[MINORITY VERDICT: label]` | Expert coalition | Alternative recommendation |

---

#### Example: Complete Expert Response

```markdown
# Muffin - Value Analyst (Round 1)

[MUFFIN-P0101: Options viability confirmed]
The 30-delta covered call strategy can generate 18-34% annualized income,
effectively bridging the income mandate gap. This addresses my original
concern from P0001.

[RE:REFINE P0001]
My round 0 perspective on income mandate mismatch evolves: the gap is
bridgeable with derivatives, not insurmountable as I initially framed.

[RE:SUPPORT R0001]
Donut's collar structure is sound. The 45 DTE recommendation optimizes
theta decay while maintaining flexibility.

[RE:ADDRESS T0001]
With the options overlay generating 18-34% annualized, the 4% income
mandate can be satisfied. I believe T0001 is addressable pending
execution details.

[MUFFIN-E0101: Historical options premium data]
- NVDA 30-day ATM IV averaged 45% over past 24 months
- 30-delta calls yielded 2.1-2.8% monthly premium
- Premium remained viable even during Q4 2022 drawdown

[RE:SUPPORT MUFFIN-P0101]
This data supports my options viability perspective.

[MUFFIN-T0101: Execution timing constraint]
Post-refinancing window creates 60-90 day delay. Optimal entry may
conflict with covenant restrictions.

[MOVE:BRIDGE P0003 R0001]
Cupcake's concentration concern and Donut's collar proposal can be
reconciled: phased entry over 6 months limits concentration while
allowing options strategy to scale.

[MUFFIN-C0101: Income mandate resolved]
The income mandate objection is resolved via options overlay. Remaining
concerns are execution timing and concentration—both manageable with
phased approach.

[RE:DEPEND MUFFIN-P0101]
[RE:DEPEND MUFFIN-E0101]
```

#### Stored Relationships

Cross-references are stored in the database and exported:

```json
{
  "id": "P0101",
  "label": "Options viability confirmed",
  "contributors": ["muffin"],
  "references": [
    { "type": "refine", "target": "P0001" },
    { "type": "support", "target": "R0001" },
    { "type": "address", "target": "T0001" }
  ],
  "moves": [
    { "type": "bridge", "targets": ["P0003", "R0001"] }
  ]
}
```

**Parsing priority:** The export parser extracts markers in order of appearance, maintaining expert voice while enabling structured analysis.

## Portability

This schema is designed for future migration to DynamoDB (see ADR 0017).

**Single-table pattern:**
```
PK: dialogue_id
SK: TYPE#subkey

nvidia-dec | META                  → dialogue metadata
nvidia-dec | EXPERT#muffin         → expert info, total score
nvidia-dec | ROUND#0               → round metadata, score, velocity
nvidia-dec | RSCORE#0#muffin       → expert score for round 0
nvidia-dec | PERSP#0#muffin#1      → perspective (round#expert#seq)
nvidia-dec | REC#0#donut#1         → recommendation (round#expert#seq)
nvidia-dec | CONTRIB#0#muffin#evidence#1 → contribution (non-first-class)
nvidia-dec | TENSION#T0001           → tension
nvidia-dec | TEVENT#T0001#1706900000 → tension event
nvidia-dec | REF#P0101#support#R0001 → cross-reference (source#type#target)
nvidia-dec | VERDICT#final         → verdict
nvidia-dec | VERDICT#minority-esg  → minority verdict
```

**Design constraints for portability:**
1. All queries scoped to single `dialogue_id` (partition key)
2. Composite sort keys encode hierarchy (`TYPE#subkey`)
3. No cross-dialogue JOINs in hot path
4. Judge performs all writes (single writer per dialogue)

**Why Judge writes all data:**
- Agents run in parallel and could cause write contention
- Judge already reads all agent outputs for scoring
- Single writer eliminates race conditions on `seq` assignment
- SQLite and DynamoDB both benefit from this pattern

See RFC 0053 for the storage abstraction layer (future work).

## Calibration (RFC 0054)

This RFC defines **uncalibrated** dialogues — experts argue freely without domain-specific guardrails. RFC 0054 extends this with **calibrated** dialogues using:

| Concept | Description |
|---------|-------------|
| **Principle** | Universal truths (inter-domain) |
| **Tenet** | Domain-specific norms ("the way we do things") |
| **Constraint** | Question-specific requirements |
| **Ethos** | Synthesized, conflict-free rule set for a dialogue |

**Impact on this RFC:**

1. **Schema**: `dialogues` table gains `calibrated`, `domain_id`, `ethos_id` columns
2. **Context fetch**: `blue_dialogue_round_context` returns `calibration` block with ethos rules
3. **Prompts**: Calibrated dialogues inject ethos into expert prompts
4. **Verdicts**: `ethos_compliance` field tracks compliance and exceptions
5. **Scoring**: Judge may apply calibration modifiers (ethos violations, bonuses)

**Authoring**: Principles, tenets, and constraints are authored via the `/ethos` skill (see RFC 0054).

## Implementation

### Phase 1: Schema ✅ Complete
- [x] Add `dialogues` root table with collision-safe ID generation
- [x] Add `experts`, `rounds`, `expert_round_scores` tables
- [x] Add `perspectives`, `tensions`, `tension_events` tables
- [x] Add `recommendations` table (first-class, links to tensions/perspectives)
- [x] Add `evidence`, `claims` tables (first-class entities)
- [x] Add `refs` table for explicit cross-references between entities
- [x] Add `contributions`, `verdicts` tables
- [x] Add foreign key constraints to enforce referential integrity
- [x] Add indices for common query patterns (including refs target/source indices)

**Implementation Notes:**
- Schema migration v8→v9 in `crates/blue-core/src/store.rs`
- 13 new tables: `alignment_dialogues`, `alignment_experts`, `alignment_rounds`, `alignment_perspectives`, `alignment_perspective_events`, `alignment_tensions`, `alignment_tension_events`, `alignment_recommendations`, `alignment_recommendation_events`, `alignment_evidence`, `alignment_claims`, `alignment_refs`, `alignment_verdicts`
- All DB operations in `crates/blue-core/src/alignment_db.rs` (1400+ lines)

### Phase 2: MCP Tools (Public API) ✅ Complete
- [x] `blue_dialogue_round_context` - **bulk fetch** context for all panel experts
- [x] `blue_dialogue_expert_create` - create new expert mid-dialogue (Judge only)
- [x] `blue_dialogue_round_register` - **bulk register** all round data in single call
- [x] `blue_dialogue_verdict_register` - register interim/final/minority verdicts
- [x] `blue_dialogue_export` - generate JSON export from database

**Implementation Notes:**
- Tool definitions in `crates/blue-mcp/src/server.rs` (lines 1795-2000)
- Handler implementations in `crates/blue-mcp/src/handlers/dialogue.rs`
- All handlers use `ProjectState` for DB access via `state.store.conn()`

### Phase 2b: Internal Functions ✅ Complete
- [x] `register_perspective` - called by `round_register`
- [x] `register_recommendation` - called by `round_register`
- [x] `register_tension` - called by `round_register`
- [x] `update_tension_status` - called by `round_register`
- [x] `register_evidence` - called by `round_register`
- [x] `register_claim` - called by `round_register`
- [x] `register_ref` - called by `round_register`
- [x] `register_verdict` - called by `verdict_register`

**Implementation Notes:**
- All functions in `crates/blue-core/src/alignment_db.rs`
- Auto-generates display IDs (`P0101`, `T0203`, etc.)
- Creates event audit trails for perspectives, tensions, recommendations

### Phase 2c: Validation Layer ✅ Complete
- [x] Implement MCP-layer validation with structured error responses
- [x] Implement batch validation (all errors returned, not just first)
- [x] Add error codes and message templates per constraint type

**Implementation Notes:**
- `ValidationError` struct with code, message, field, suggestion, context
- `ValidationErrorCode` enum: MissingField, InvalidEntityType, InvalidRefType, TypeIdMismatch, InvalidRefTarget, InvalidDisplayId, etc.
- `ValidationCollector` for batch error collection
- `validate_ref_semantics()` enforces: resolve/reopen/address → Tension, refine → same-type
- `validate_display_id()` validates format and extracts components
- `handle_round_register` returns all validation errors in structured JSON before DB operations
- 8 new validation tests added (20 total alignment_db tests)

### Phase 3: Lifecycle Tracking ✅ Complete
- [x] Implement tension state machine (open → addressed → resolved → reopened)
- [x] Create `tension_events` audit trail
- [x] Add authority checks for resolution confirmation (via `actors` parameter)
- [x] Support verdict types: interim, final, minority, dissent

**Implementation Notes:**
- `TensionStatus` enum: `Open`, `Addressed`, `Resolved`, `Reopened`
- Events stored with actors, reference, and round number
- `VerdictType` enum: `Interim`, `Final`, `Minority`, `Dissent`

### Phase 4: Export Tooling ✅ Complete
- [x] Implement `blue_dialogue_export` MCP tool
- [x] Query all data from database (no file parsing)
- [x] Generate single dialogue.json with all data (perspectives, recommendations, tensions, evidence, claims, verdicts)
- [ ] Validation and warning reporting
- [ ] Integration with superviber-web demo viewer

**Implementation Notes:**
- `handle_export()` in `crates/blue-mcp/src/handlers/dialogue.rs`
- Writes to `{output_dir}/{dialogue_id}/dialogue.json` by default
- Full provenance: includes `created_at`, `refs`, status for all entities

### Phase 5: Skill & Documentation Updates ✅ Complete
- [x] Create `alignment-expert` skill with static marker syntax:
  - First-class entity markers (P, R, T, E, C)
  - Cross-reference syntax (RE:SUPPORT, RE:OPPOSE, etc.)
  - Dialogue move syntax (MOVE:DEFEND, MOVE:BRIDGE, etc.)
  - Verdict markers (DISSENT, MINORITY VERDICT)
  - Local ID format rules
- [x] Update `alignment-play` skill with new workflow:
  - Judge fetches data via `blue_dialogue_round_context`
  - Judge builds prompts with context from DB
  - Judge spawns agents with full prompt + `alignment-expert` skill reference
  - Judge calls `blue_dialogue_round_register` after scoring (bulk registration)
  - Two-phase ID system: agents write local IDs, Judge registers global IDs
  - Dynamic expert creation via `blue_dialogue_expert_create`
- [x] Document tool parameters in skill file
- [x] Add examples for DB-backed workflow

**Implementation Notes:**
- `skills/alignment-expert/SKILL.md` - full marker syntax reference
- `skills/alignment-play/SKILL.md` - updated with DB-backed workflow, two-phase ID system, tool documentation
- Both skills reference each other for complete workflow

### Phase 6: Tooling & Analysis
- [x] Citation auto-expansion (short form → composite key)
- [ ] Visualization dashboard integration (requires external UI)
- [x] Cross-dialogue analysis queries
- [x] Real-time dialogue monitoring dashboard

**Implementation Notes:**
- `expand_citation()` and `expand_citations()` - expand display ID to full entity context (label, content, contributors, status)
- `get_cross_dialogue_stats()` - aggregated statistics across all dialogues (entity counts, averages, top experts)
- `find_similar_dialogues()` - text-based search across dialogue titles and tension labels
- `get_dialogue_progress()` - real-time status including velocity, convergence detection, leaderboard, estimated rounds remaining
- Tests: `test_expand_citation_*`, `test_cross_dialogue_stats`, `test_dialogue_progress*`, `test_find_similar_dialogues`

## Test Plan

### Core Schema Tests (12 unit tests in `alignment_db::tests`) ✅
- [x] **Dialogue ID uniqueness**: Creating dialogue with duplicate title generates suffixed ID (`test_generate_dialogue_id`)
- [x] Display IDs derived correctly from composite keys (`test_display_id_format`, `test_parse_display_id`)
- [x] Tension lifecycle transitions work correctly (`test_tension_lifecycle`)
- [x] Cross-references stored and retrieved (`test_cross_references`)
- [x] Expert registration and scores (`test_register_expert`, `test_expert_scores`)
- [x] Perspective registration (`test_register_perspective`)
- [x] Tension registration (`test_register_tension`)
- [x] Verdict registration (`test_verdict_registration`)
- [x] Full dialogue workflow (`test_full_dialogue_workflow`)
- [x] Create and get dialogue (`test_create_and_get_dialogue`)

### MCP Tool Tests (Integration)
- [x] **Agent Context**: `blue_dialogue_round_context` returns context for ALL panel experts in one call
- [x] **Agent Context**: Shared data (dialogue, prior_rounds, tensions) included
- [x] **Agent Context**: Returns structured perspectives/recommendations with full `content` field
- [x] **Expert Creation**: Judge can create new experts mid-dialogue via MCP
- [x] **Expert Creation**: Created experts have `source: "created"` and `creation_reason`
- [x] **Expert Creation**: `first_round` tracks when expert joined
- [x] **Registration**: Perspectives registered with correct display IDs (P0101, P0201, etc.)
- [x] **Registration**: Round completion updates expert scores and dialogue total
- [x] **Registration**: Tension events create audit trail
- [x] **Export**: Single dialogue.json contains all sections (experts, rounds, perspectives, recommendations, tensions, evidence, claims, verdicts)
- [x] **Export**: Global IDs are unique and sequential
- [x] **Export**: All data comes from database queries (no file parsing)
- [x] **Refs**: Cross-references stored in `refs` table with typed relationships
- [x] **Refs**: ref_type constrained to valid types (support, oppose, refine, etc.)
- [x] **Verdicts**: Multiple verdicts per dialogue supported
- [x] **Verdicts**: Interim verdicts can be registered mid-dialogue
- [x] **Verdicts**: Minority verdicts capture dissenting coalition
- [x] **Verdicts**: Export includes all verdicts in chronological order

### Validation Layer Tests (Phase 2c) ✅ Complete
- [x] **Errors**: MCP returns structured error with `error_code`, `message`, `suggestion`
- [x] **Errors**: Type enum violations return `invalid_entity_type` or `invalid_ref_type`
- [x] **Errors**: Type/ID mismatch returns `type_id_mismatch` with expected prefix
- [x] **Errors**: Semantic violations return `invalid_ref_target` with valid options
- [x] **Errors**: Batch operations return all validation errors, not just first
- [x] **Errors**: Structured JSON response allows Judge to parse and correct programmatically
- [x] **Refs**: Semantic constraint: resolve/reopen/address must target Tension (T)
- [x] **Refs**: Semantic constraint: refine must be same-type (P→P, R→R, etc.)
- [x] **Refs**: Invalid combo caught with appropriate error (e.g., `P resolve→ P` returns InvalidRefTarget)

### Skill Integration Tests (Phase 5) ✅ Complete
- [x] **Skill**: `alignment-expert` skill contains static marker syntax reference
- [x] **Skill**: Skill can be loaded once per agent via skill reference
- [x] **Prompt Assembly**: Judge builds prompts from `blue_dialogue_round_context` data (documented workflow)
- [x] **Prompt Assembly**: Markdown prompt includes full content from all prior experts (via round_context)
- [x] **Prompt Assembly**: Judge spawns agents with full prompt + `alignment-expert` skill reference
- [ ] **Prompt Assembly**: Judge writes `prompt-{expert}.md` to disk for debugging (optional enhancement)

### Phase 6: Tooling Tests ✅ Complete
- [x] **Citation Expansion**: `expand_citation` returns full entity context from display ID (`test_expand_citation_perspective`)
- [x] **Citation Expansion**: Tensions include status field (`test_expand_citation_tension`)
- [x] **Citation Expansion**: Recommendations include parameters (`test_expand_citation_recommendation_with_params`)
- [x] **Citation Expansion**: Batch expansion returns errors for invalid IDs (`test_expand_multiple_citations`)
- [x] **Cross-Dialogue**: Statistics aggregate across all dialogues (`test_cross_dialogue_stats`)
- [x] **Cross-Dialogue**: Similar dialogues found by text search (`test_find_similar_dialogues`)
- [x] **Progress Tracking**: Real-time progress includes velocity and convergence (`test_dialogue_progress`)
- [x] **Progress Tracking**: Convergence detected when velocity near zero (`test_dialogue_progress_convergence`)

### Performance & Isolation Tests ✅ Complete
- [x] **Output directory isolation**: Unique dialogue IDs ensure separate output dirs (`test_output_directory_isolation`)
- [x] SQLite indices exist for all key lookups (`test_indices_exist`)
- [x] 100+ perspective queries complete under 100ms (`test_performance_many_perspectives`)
- [x] No orphaned entities - refs connect valid entities (`test_no_orphaned_entities`)

## Dialogue Summary

This RFC was designed through a 5-expert alignment dialogue:

| Expert | Role | Key Contribution |
|--------|------|------------------|
| Cupcake | Traceability Analyst | Composite key design, lifecycle rules |
| Muffin | API Designer | JSON export schema, API contract |
| Brioche | Data Architect | SQLite schema, event audit trail |
| Donut | Skeptic | Challenged assumptions, surfaced lifecycle gap |
| Scone | Performance Engineer | Indexing strategy |
| Eclair | UX Advocate | Human-readable ID advocacy |

**Key tensions resolved:**
- Prefixed vs Sequential IDs → Storage/display split
- Real-time vs Snapshot tracking → Two-layer architecture
- Merge conflicts → Composite keys
- Lifecycle underspecified → Four-state machine with events

---

*"The blind men now have coordinates. Each can say exactly which part of the elephant they touched."*
