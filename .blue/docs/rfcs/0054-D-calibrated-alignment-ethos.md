# RFC 0054: Calibrated Alignment Dialogues — Ethos & Tenets

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-02 |
| **Depends On** | RFC 0051 (Global Perspective & Tension Tracking) |
| **Introduces** | `/charter` skill for authoring calibration rules |

---

## Summary

Introduce a hierarchical system of **Principles**, **Tenets**, and **Constraints** that calibrate alignment dialogues. These are synthesized into an **Ethos** — a coherent, conflict-free rule set that guides both experts and the Judge during deliberation.

## Problem

RFC 0051 defines "uncalibrated" alignment dialogues where:
- Experts argue freely without domain-specific guardrails
- The Judge scores based on general ALIGNMENT criteria
- No organizational policies or domain norms are enforced
- Question-specific requirements may be overlooked

This works for exploratory discussions but fails for:
- **Regulated domains** (fiduciary duty, compliance, safety-critical)
- **Organizational alignment** (company values, investment mandates, design principles)
- **Repeatable processes** (consistent decision-making across similar questions)

## Design

### Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│                     PRINCIPLES                          │
│              (Universal, inter-domain)                  │
│   "Evidence must be citable and verifiable"            │
│   "Conflicts of interest must be disclosed"            │
└─────────────────────────────────────────────────────────┘
                          │
                          │ inherit
                          ▼
┌─────────────────────────────────────────────────────────┐
│                       TENETS                            │
│                  (Domain-specific)                      │
│   Domain: "Fiduciary Investment Analysis"              │
│   "Fiduciary duty supersedes growth optimization"      │
│   "Income requirements take precedence over gains"     │
└─────────────────────────────────────────────────────────┘
                          │
                          │ contextualize
                          ▼
┌─────────────────────────────────────────────────────────┐
│                    CONSTRAINTS                          │
│                 (Question-specific)                     │
│   "The Acme Trust requires 4% annual income"           │
│   "Single-position exposure cannot exceed 25%"         │
│   "Execution window: 60-90 days post-refinancing"      │
└─────────────────────────────────────────────────────────┘
                          │
                          │ synthesize
                          ▼
┌─────────────────────────────────────────────────────────┐
│                       ETHOS                             │
│          (Synthesized, conflict-free rule set)         │
│   Applied to a specific dialogue                       │
│   Injected into expert prompts + Judge protocol        │
└─────────────────────────────────────────────────────────┘
```

### Definitions

**Principle** — Universal truths that apply across all domains. These are foundational beliefs about how alignment dialogues should operate. Authored system-wide, rarely changed.

**Tenet** — Domain-specific norms, preferences, and "the way we do things" within a particular field. A domain can inherit from multiple parent domains. Authored per organization/domain via `/charter` skill.

**Constraint** — Situational requirements extracted from the specific question or context. May be authored via `/charter` skill or extracted by the Judge at dialogue creation.

**Lens** — A named configuration within a domain that selects which tenets apply, optionally overriding priorities. Used for client-specific or assessment-type configurations.

**Charter** — The synthesized, coherent combination of applicable principles + tenets (from one or more domains, each filtered by optional lens) + constraints for a specific dialogue. Redundancy removed, conflicts resolved, ready for injection. Supports cross-domain deliberation.

### ID Scheme

Calibration entities use two-letter prefixes to avoid collision with RFC 0051's single-letter IDs (P, R, T, E, C). Domain codes use 3 characters to avoid collisions.

| Entity | Prefix | Format | Scope | Example |
|--------|--------|--------|-------|---------|
| **Principle** | `PR` | `PR{seq:04d}` | Global | PR0001, PR0002 |
| **Domain** | — | slug + `code` (3 chars) | Global | `fiduciary-investment` (code: `FID`) |
| **Tenet** | `TN` | `{code}-TN{seq:02d}` | Domain | FID-TN01, INV-TN03 |
| **Lens** | `LN` | `{code}-LN{seq:02d}` | Domain | FID-LN01, FID-LN02 |
| **Charter** | `CH` | `CH{seq:04d}` | Global | CH0001, CH0042 |
| **Charter Rule** | `R` | `{charter}-R{seq:02d}` | Charter | CH0001-R01, CH0001-R05 |
| **Constraint** | `CN` | `CN{seq:02d}` | Dialogue | CN01, CN02 |

**Design rationale:**
- Two-letter prefixes avoid collision with 0051's P/R/T/E/C
- Domain codes use 3 characters to avoid inter-domain collisions
- Global entities (PR, CH) use 4-digit seq for scale
- Domain-scoped entities (TN, LN) include domain code for provenance
- Charter rules are scoped to their charter for clean expert references
- Dialogue-scoped entities (CN) use 2-digit seq like 0051
- Domains keep slugs for human-readability, with short codes for namespacing

**Cross-referencing:**
```markdown
[TENSION T0003: challenges CH0001-R05]
The income precedence rule (CH0001-R05, from FID-TN02) conflicts with
the growth mandate. Per CH0001-R01 (fiduciary primacy), I concede.
```

**Charter rules** have their own IDs for expert reference, while preserving linkage to their source (PR0001, FID-TN01, CN01).

### Authoring via `/charter` Skill

All calibration content is authored through the `/charter` skill, which provides:

```
/charter                     — Overview, list domains and principles
/charter domain <name>       — Create or view a domain
/charter tenet <domain>      — Add/edit tenets for a domain
/charter lens <domain>       — Create/edit lenses for a domain
/charter principle           — View/propose system principles
/charter constraint <dialogue> — Add constraints to a dialogue
/charter synthesize <dialogue> [--lens <lens_id>] — Generate charter for a dialogue
/charter review <charter_id>   — Review and approve synthesized charter
```

**Skill workflow for domain creation:**

```
User: /charter domain "Fiduciary Investment Analysis"

Claude: I'll help you define the "Fiduciary Investment Analysis" domain.

**Parent domains** (inherit tenets from):
- [ ] Investment Analysis (general)
- [ ] Regulatory Compliance
- [ ] Risk Management

**Core questions to establish tenets:**

1. What is the primary obligation? (e.g., fiduciary duty to beneficiaries)
2. How should income vs growth be prioritized?
3. What concentration limits apply?
4. What documentation/audit requirements exist?

Let's start with the primary obligation...
```

**Skill workflow for charter synthesis:**

```
User: /charter synthesize nvidia-investment-decision

Claude: Synthesizing charter for dialogue "nvidia-investment-decision"...

**Domain:** Fiduciary Investment Analysis
**Applicable Principles:** 3
**Applicable Tenets:** 7
**Extracted Constraints:** 4

**Conflict detected:**
- Tenet INV-TN03: "Maximize total return over 10-year horizon"
- Constraint CN01: "4% annual income requirement"

**Resolution:** Income constraint takes precedence per FID-TN01
("Fiduciary duty supersedes growth optimization")

**Synthesized Charter (14 rules):**
[Preview of merged rules...]

Approve this charter for the dialogue? [Yes / Edit / Reject]
```

### Schema

```sql
-- ================================================================
-- PRINCIPLES (universal, inter-domain)
-- ================================================================
CREATE TABLE principles (
  principle_id    TEXT PRIMARY KEY,  -- PR0001, PR0002, ...
  seq             INT NOT NULL UNIQUE,  -- auto-increment for ID generation
  label           TEXT NOT NULL,
  description     TEXT NOT NULL,
  rationale       TEXT,              -- why this principle exists
  status          TEXT NOT NULL DEFAULT 'active',
  created_at      TEXT NOT NULL,
  CHECK (status IN ('draft', 'active', 'deprecated')),
  CHECK (principle_id = 'PR' || printf('%04d', seq))
);

-- Example principles
-- PR0001: "Evidence must be citable and verifiable"
-- PR0002: "Conflicts of interest must be disclosed"
-- PR0003: "Minority perspectives must be heard before convergence"

-- ================================================================
-- DOMAINS (categories of dialogue)
-- ================================================================
CREATE TABLE domains (
  domain_id       TEXT PRIMARY KEY,  -- slug: "fiduciary-investment"
  code            TEXT NOT NULL UNIQUE,  -- 3-char code: "FID" (uppercase)
  label           TEXT NOT NULL,
  description     TEXT NOT NULL,
  parent_domains  JSON,              -- ["investment-analysis", "regulatory-compliance"]
  status          TEXT NOT NULL DEFAULT 'active',
  created_at      TEXT NOT NULL,
  CHECK (status IN ('draft', 'active', 'deprecated')),
  CHECK (length(code) = 3),
  CHECK (code = upper(code))
);

-- ================================================================
-- LENSES (tenet selection/override configurations within a domain)
-- ================================================================
CREATE TABLE lenses (
  lens_id         TEXT PRIMARY KEY,  -- FID-LN01, FID-LN02, ...
  domain_id       TEXT NOT NULL,
  seq             INT NOT NULL,      -- sequential within domain
  label           TEXT NOT NULL,     -- "Conservative Income", "Acme Trust"
  description     TEXT NOT NULL,

  -- Tenet selection (all optional, applied in order: include → exclude → override)
  include_tenets  JSON,              -- ["FID-TN01", "FID-TN02"] — whitelist (null = all)
  exclude_tenets  JSON,              -- ["FID-TN05"] — blacklist (applied after include)
  priority_overrides JSON,           -- {"FID-TN02": 1000, "FID-TN04": 500} — override priorities

  status          TEXT NOT NULL DEFAULT 'active',
  created_at      TEXT NOT NULL,
  FOREIGN KEY (domain_id) REFERENCES domains(domain_id),
  UNIQUE (domain_id, seq),
  CHECK (status IN ('draft', 'active', 'deprecated'))
);
-- lens_id generated as: {domain.code}-LN{seq:02d}

-- Example lenses for domain "fiduciary-investment" (code: FID)
-- FID-LN01: "Conservative Income" — include: TN01,TN02,TN04; exclude: TN05
-- FID-LN02: "Growth Oriented" — include: TN01,TN03,TN05; priority_overrides: {TN03: 1000}
-- FID-LN03: "Acme Trust" — client-specific configuration

-- ================================================================
-- TENETS (domain-specific norms)
-- ================================================================
CREATE TABLE tenets (
  tenet_id        TEXT PRIMARY KEY,  -- FID-TN01, INV-TN03, ...
  domain_id       TEXT NOT NULL,
  seq             INT NOT NULL,      -- sequential within domain
  label           TEXT NOT NULL,
  description     TEXT NOT NULL,
  priority        INT NOT NULL DEFAULT 100,  -- higher = more authoritative in conflicts
  rationale       TEXT,
  status          TEXT NOT NULL DEFAULT 'active',
  created_at      TEXT NOT NULL,
  FOREIGN KEY (domain_id) REFERENCES domains(domain_id),
  UNIQUE (domain_id, seq),
  CHECK (status IN ('draft', 'active', 'deprecated'))
);
-- tenet_id generated as: {domain.code}-TN{seq:02d}

-- Example tenets for domain "fiduciary-investment" (code: FID)
-- FID-TN01: "Fiduciary duty supersedes growth optimization" (priority: 1000)
-- FID-TN02: "Income requirements take precedence over capital gains" (priority: 900)
-- FID-TN03: "Concentration risk must be explicitly addressed" (priority: 800)

-- ================================================================
-- CONSTRAINTS (question-specific)
-- ================================================================
CREATE TABLE constraints (
  constraint_id   TEXT PRIMARY KEY,  -- CN01, CN02, ... (dialogue-scoped)
  dialogue_id     TEXT NOT NULL,
  seq             INT NOT NULL,      -- sequential within dialogue
  label           TEXT NOT NULL,
  description     TEXT NOT NULL,
  source          TEXT NOT NULL,     -- 'extracted' | 'authored'
  source_detail   TEXT,              -- e.g., "Section 4.2 of trust document"
  priority        INT NOT NULL DEFAULT 100,
  created_at      TEXT NOT NULL,
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  UNIQUE (dialogue_id, seq),
  CHECK (source IN ('extracted', 'authored')),
  CHECK (constraint_id = 'CN' || printf('%02d', seq))
);

-- Example constraints for nvidia-investment-decision
-- CN01: "Acme Trust requires 4% annual income distribution"
-- CN02: "Single-position exposure cannot exceed 25%"
-- CN03: "Execution window: 60-90 days post-refinancing"

-- ================================================================
-- CHARTERS (synthesized rule set for a dialogue)
-- ================================================================
CREATE TABLE charters (
  charter_id      TEXT PRIMARY KEY,  -- CH0001, CH0002, ...
  seq             INT NOT NULL UNIQUE,  -- auto-increment for ID generation
  dialogue_id     TEXT NOT NULL UNIQUE,  -- one charter per dialogue
  status          TEXT NOT NULL DEFAULT 'draft',
  synthesized_at  TEXT NOT NULL,
  approved_at     TEXT,
  approved_by     TEXT,              -- 'judge' or user identifier

  -- Synthesis metadata
  principle_count INT NOT NULL,
  tenet_count     INT NOT NULL,
  constraint_count INT NOT NULL,
  conflict_count  INT NOT NULL DEFAULT 0,

  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id),
  CHECK (status IN ('draft', 'approved', 'rejected')),
  CHECK (charter_id = 'CH' || printf('%04d', seq))
);

-- ================================================================
-- CHARTER DOMAINS (multi-domain support with per-domain lenses)
-- ================================================================
CREATE TABLE charter_domains (
  charter_id      TEXT NOT NULL,
  domain_id       TEXT NOT NULL,
  lens_id         TEXT,              -- optional: FID-LN01 (null = all domain tenets)
  inclusion_order INT NOT NULL,      -- order domains were added (affects rule ordering)

  PRIMARY KEY (charter_id, domain_id),
  FOREIGN KEY (charter_id) REFERENCES charters(charter_id),
  FOREIGN KEY (domain_id) REFERENCES domains(domain_id),
  FOREIGN KEY (lens_id) REFERENCES lenses(lens_id)
);

-- Example: Cross-domain charter for AI medical investment
-- CH0042 includes:
--   (FID, FID-LN03, 1)  -- Fiduciary with Acme Trust lens
--   (MED, MED-LN01, 2)  -- Medical Ethics with Patient Safety lens
--   (REG, null, 3)      -- Regulatory Compliance, all tenets

-- ================================================================
-- CHARTER RULES (the actual synthesized rules)
-- ================================================================
CREATE TABLE charter_rules (
  charter_id      TEXT NOT NULL,
  rule_id         TEXT NOT NULL,     -- CH0001-R01, CH0001-R02, ...
  rule_seq        INT NOT NULL,
  source_type     TEXT NOT NULL,     -- 'principle' | 'tenet' | 'constraint'
  source_id       TEXT NOT NULL,     -- PR0001, FID-TN01, CN01, etc.
  source_domain   TEXT,              -- domain_id for tenets (null for principles/constraints)
  label           TEXT NOT NULL,
  description     TEXT NOT NULL,
  priority        INT NOT NULL,      -- effective priority (may be overridden by lens)

  -- Conflict resolution (if this rule resulted from conflict)
  supersedes      JSON,              -- [{"source_id": "INV-TN03", "reason": "..."}]

  PRIMARY KEY (charter_id, rule_seq),
  FOREIGN KEY (charter_id) REFERENCES charters(charter_id),
  CHECK (source_type IN ('principle', 'tenet', 'constraint')),
  CHECK (rule_id = charter_id || '-R' || printf('%02d', rule_seq))
);

-- ================================================================
-- CHARTER CONFLICTS (audit trail of detected conflicts during synthesis)
-- ================================================================
CREATE TABLE charter_conflicts (
  charter_id        TEXT NOT NULL,
  conflict_seq      INT NOT NULL,
  rule_a_type       TEXT NOT NULL,   -- 'principle' | 'tenet' | 'constraint'
  rule_a_id         TEXT NOT NULL,   -- PR0001, FID-TN01, CN01, etc.
  rule_b_type       TEXT NOT NULL,
  rule_b_id         TEXT NOT NULL,
  conflict_type     TEXT NOT NULL,   -- 'contradiction' | 'redundancy' | 'ambiguity'
  resolution        TEXT NOT NULL,   -- 'a_supersedes' | 'b_supersedes' | 'merged' | 'removed'
  resolution_reason TEXT NOT NULL,
  resolved_by       TEXT NOT NULL,   -- 'priority' | 'manual' | 'llm'

  PRIMARY KEY (charter_id, conflict_seq),
  FOREIGN KEY (charter_id) REFERENCES charters(charter_id),
  CHECK (conflict_type IN ('contradiction', 'redundancy', 'ambiguity')),
  CHECK (resolution IN ('a_supersedes', 'b_supersedes', 'merged', 'removed')),
  CHECK (resolved_by IN ('priority', 'manual', 'llm'))
);
-- Conflicts are audit records without their own IDs; experts raise tensions against rules
```

### Calibration Injection

**Expert prompt injection:**

When an expert receives their context (via `blue_dialogue_round_context`), calibrated dialogues include:

```markdown
## Charter CH0001: Fiduciary Investment Analysis

This dialogue operates under the following charter:

### Principles (Universal)
- [CH0001-R01] Evidence must be citable and verifiable (from PR0001)
- [CH0001-R02] Conflicts of interest must be disclosed (from PR0002)
- [CH0001-R03] Minority perspectives must be heard (from PR0003)

### Tenets (Domain: Fiduciary Investment Analysis, Lens: FID-LN03)
- [CH0001-R04] **[Priority 1000]** Fiduciary duty supersedes growth (from FID-TN01)
- [CH0001-R05] **[Priority 950]** Income takes precedence (from FID-TN02, lens override)
- [CH0001-R06] **[Priority 800]** Concentration risk addressed (from FID-TN03)

### Constraints (This Dialogue)
- [CH0001-R07] Acme Trust requires 4% annual income (CN01)
- [CH0001-R08] Single-position exposure max 25% (CN02)
- [CH0001-R09] Execution window: 60-90 days (CN03)

**Your arguments must be consistent with this charter.** If you believe a rule should be challenged, surface it as a [TENSION] with explicit reference to the rule ID (e.g., "challenges CH0001-R05").
```

**Judge protocol injection:**

The Judge receives calibration guidance:

```markdown
## Calibration Scoring

This is a **calibrated** dialogue under charter `CH0001`.

**Scoring modifiers:**
- Arguments that violate charter rules without surfacing a tension: -2 per violation
- Arguments that explicitly address high-priority rules: +1 bonus
- Tensions that challenge charter rules (constructively): scored normally
- Evidence supporting charter compliance: +1 bonus

**Validation:**
- All recommendations MUST address constraints CN01-CN03
- Recommendations violating tenets require explicit tension acknowledgment
- Final verdict must confirm charter compliance or document exceptions

**Conflict resolution authority:**
- Priority-based resolution is automatic
- Manual overrides require Judge justification in verdict
```

### Integration with RFC 0051

**Modified `dialogues` table:**

```sql
ALTER TABLE dialogues ADD COLUMN calibrated BOOLEAN DEFAULT FALSE;
ALTER TABLE dialogues ADD COLUMN domain_id TEXT REFERENCES domains(domain_id);
ALTER TABLE dialogues ADD COLUMN charter_id TEXT REFERENCES charters(charter_id);
```

**Modified `blue_dialogue_create`:**

```json
{
  "title": "NVIDIA Investment Analysis",
  "calibrated": true,                   // NEW: enables charter injection
  "domains": [                          // NEW: one or more domains with optional lenses
    { "domain": "fiduciary-investment", "lens": "FID-LN03" }
  ],
  "constraints": [                      // NEW: question-specific constraints
    "Acme Trust requires 4% annual income distribution",
    "Single-position exposure cannot exceed 25%"
  ],
  "expert_pool": { ... }
}
```

**Cross-domain example:**

```json
{
  "title": "AI Medical Investment Analysis",
  "calibrated": true,
  "domains": [
    { "domain": "fiduciary-investment", "lens": "FID-LN03" },
    { "domain": "medical-ethics", "lens": "MED-LN01" },
    { "domain": "regulatory-compliance", "lens": null }
  ],
  "constraints": [...],
  "expert_pool": { ... }
}
```

If `calibrated: true`, the MCP server:
1. Looks up domain tenets
2. Extracts/validates constraints
3. Synthesizes charter
4. Returns charter for Judge review (or auto-approves if no conflicts)

**Modified `blue_dialogue_round_context` response:**

```json
{
  "dialogue": { ... },
  "calibration": {                      // NEW: only if calibrated
    "charter_id": "CH0001",
    "domains": [                        // supports multiple domains
      { "domain": "Fiduciary Investment Analysis", "code": "FID", "lens_id": "FID-LN03", "lens_label": "Acme Trust" }
    ],
    "rules": [
      { "rule_id": "CH0001-R01", "source_id": "PR0001", "type": "principle", "label": "Evidence must be citable", "priority": 100 },
      { "rule_id": "CH0001-R02", "source_id": "FID-TN01", "type": "tenet", "domain": "FID", "label": "Fiduciary duty supersedes growth", "priority": 1000 },
      { "rule_id": "CH0001-R03", "source_id": "CN01", "type": "constraint", "label": "4% annual income required", "priority": 500 }
    ],
    "prompt_injection": "## Charter: Fiduciary Investment Analysis\n\n..."
  },
  "experts": { ... }
}
```

**Modified verdict schema:**

```json
{
  "verdict_id": "final",
  "charter_compliance": {                 // NEW: required for calibrated dialogues
    "fully_compliant": false,
    "exceptions": [
      {
        "rule_id": "CN01",
        "rule_label": "4% annual income required",
        "exception_type": "partial",
        "justification": "Options overlay achieves 3.2% with 0.8% deferred to Q2",
        "approved_by": "judge"
      }
    ],
    "violations": []
  }
}
```

### MCP Tools

**New public tools:**

| Tool | Purpose |
|------|---------|
| `blue_charter_domain_create` | Create a new domain with parent inheritance |
| `blue_charter_domain_get` | Get domain with all tenets and lenses |
| `blue_charter_tenet_add` | Add tenet to domain |
| `blue_charter_lens_create` | Create a lens within a domain |
| `blue_charter_lens_get` | Get lens with resolved tenet list |
| `blue_charter_synthesize` | Synthesize charter for a dialogue (optional lens_id) |
| `blue_charter_approve` | Approve synthesized charter |
| `blue_principle_list` | List all active principles |

**Internal functions:**

- `tenet_inherit` — Resolve tenet inheritance from parent domains
- `lens_apply` — Apply lens include/exclude/override to tenet list
- `conflict_detect` — Identify contradictions, redundancies, ambiguities
- `conflict_resolve` — Apply priority-based or LLM-assisted resolution
- `charter_render` — Generate markdown injection for prompts

## Examples

### Domain: Fiduciary Investment Analysis

```yaml
domain:
  id: fiduciary-investment
  code: FID                             # 3-char code for tenet/lens namespacing
  label: Fiduciary Investment Analysis
  parents:
    - investment-analysis               # code: INV
    - regulatory-compliance             # code: REG

tenets:
  - id: FID-TN01
    label: Fiduciary primacy
    description: Fiduciary duty to beneficiaries supersedes all other considerations including growth optimization
    priority: 1000

  - id: FID-TN02
    label: Income precedence
    description: Income requirements take precedence over capital gains when mandated by governing documents
    priority: 900

  - id: FID-TN03
    label: Concentration awareness
    description: Concentration risk must be explicitly addressed in any position recommendation
    priority: 800

  - id: FID-TN04
    label: Liquidity preservation
    description: Maintain sufficient liquidity for distributions and unexpected obligations
    priority: 700

  - id: FID-TN05
    label: Documentation requirement
    description: All investment decisions must be documented with rationale traceable to fiduciary duty
    priority: 600

lenses:
  - id: FID-LN01
    label: Conservative Income
    description: Income-focused configuration for clients with distribution requirements
    include_tenets: [FID-TN01, FID-TN02, FID-TN04]
    exclude_tenets: [FID-TN05]
    priority_overrides:
      FID-TN02: 1000    # Elevate income precedence to highest priority

  - id: FID-LN02
    label: Growth Oriented
    description: Growth-focused configuration for clients with long time horizons
    include_tenets: [FID-TN01, FID-TN03, FID-TN05]
    exclude_tenets: null
    priority_overrides:
      FID-TN03: 1000    # Elevate concentration awareness

  - id: FID-LN03
    label: Acme Trust
    description: Client-specific configuration for Acme Trust requirements
    include_tenets: [FID-TN01, FID-TN02, FID-TN04]
    exclude_tenets: null
    priority_overrides:
      FID-TN02: 950     # Slightly below fiduciary primacy
```

### Synthesized Charter Example

For dialogue "nvidia-investment-decision" (single domain):

```yaml
charter:
  id: CH0001
  dialogue: nvidia-investment-decision
  status: approved

  # Multi-domain support: each domain can have its own lens
  domains:
    - domain: fiduciary-investment (FID)
      lens: FID-LN03                    # Acme Trust lens applied

  rules:
    # Principles (inherited)
    - rule_id: CH0001-R01
      type: principle
      source_id: PR0001
      label: Evidence must be citable
      priority: 100

    - rule_id: CH0001-R02
      type: principle
      source_id: PR0002
      label: Conflicts of interest must be disclosed
      priority: 100

    # Tenets (from domain, filtered by lens)
    - rule_id: CH0001-R03
      type: tenet
      source_id: FID-TN01
      source_domain: FID
      label: Fiduciary primacy
      priority: 1000

    - rule_id: CH0001-R04
      type: tenet
      source_id: FID-TN02
      source_domain: FID
      label: Income precedence
      priority: 950                     # Overridden by lens FID-LN03

    # Constraints (question-specific)
    - rule_id: CH0001-R05
      type: constraint
      source_id: CN01
      label: 4% annual income requirement
      description: Acme Trust requires 4% annual income distribution per Section 4.2
      priority: 500

    - rule_id: CH0001-R06
      type: constraint
      source_id: CN02
      label: 25% concentration limit
      description: Single-position exposure cannot exceed 25% of portfolio
      priority: 500

  conflicts_resolved:
    - rule_a: { type: tenet, source_id: INV-TN03, label: "Maximize total return" }
      rule_b: { type: constraint, source_id: CN01, label: "4% income requirement" }
      resolution: b_supersedes
      reason: "Per FID-TN02 (Income precedence), income requirements take precedence"
      resolved_by: priority
```

### Cross-Domain Charter Example

For dialogue "ai-medical-investment" (multiple domains):

```yaml
charter:
  id: CH0042
  dialogue: ai-medical-investment
  status: approved

  # Cross-domain deliberation: AI + Medical + Fiduciary
  domains:
    - domain: fiduciary-investment (FID)
      lens: FID-LN03                    # Acme Trust lens
    - domain: medical-ethics (MED)
      lens: MED-LN01                    # Patient Safety First lens
    - domain: regulatory-compliance (REG)
      lens: null                        # All tenets, no filtering

  rules:
    # Principles apply to all domains
    - rule_id: CH0042-R01
      type: principle
      source_id: PR0001
      label: Evidence must be citable
      priority: 100

    # Tenets from multiple domains
    - rule_id: CH0042-R02
      type: tenet
      source_id: FID-TN01
      source_domain: FID
      label: Fiduciary primacy
      priority: 1000

    - rule_id: CH0042-R03
      type: tenet
      source_id: MED-TN01
      source_domain: MED
      label: Patient safety paramount
      priority: 1000

    - rule_id: CH0042-R04
      type: tenet
      source_id: REG-TN01
      source_domain: REG
      label: Regulatory compliance required
      priority: 900

    # Cross-domain conflict resolved during synthesis
    # (MED-TN03 "Minimize AI autonomy" vs FID-TN05 "Efficiency optimization")

  conflicts_resolved:
    - rule_a: { type: tenet, source_id: MED-TN03, domain: MED }
      rule_b: { type: tenet, source_id: FID-TN05, domain: FID }
      resolution: a_supersedes
      reason: "Patient safety (MED-TN01, priority 1000) governs AI autonomy decisions"
      resolved_by: priority
```

## Migration Path

### Phase 1: Schema
- [ ] Create `principles`, `domains`, `tenets`, `lenses`, `constraints` tables
- [ ] Create `charters`, `charter_domains`, `charter_rules`, `charter_conflicts` tables
- [ ] Add calibration columns to `dialogues` table

### Phase 2: Core Principles
- [ ] Define initial system principles (PR0001-PR0010)
- [ ] Create principle review/approval workflow

### Phase 3: `/charter` Skill
- [ ] Implement `/charter domain` command
- [ ] Implement `/charter tenet` command
- [ ] Implement `/charter synthesize` command
- [ ] Implement `/charter review` command

### Phase 4: MCP Integration
- [ ] `blue_charter_domain_create` tool
- [ ] `blue_charter_synthesize` tool
- [ ] Modify `blue_dialogue_create` for calibrated dialogues
- [ ] Modify `blue_dialogue_round_context` to include calibration

### Phase 5: Judge Protocol
- [ ] Calibration scoring modifiers
- [ ] Ethos compliance validation
- [ ] Exception documentation in verdicts

## Human-Readable Artifacts

**Principle:** Every entity in the database must have a corresponding human-readable artifact. The DB is the source of truth; artifacts are generated views for auditability.

### Artifact Structure

```
.blue/
├── calibration/
│   ├── principles.md              # All active principles
│   ├── domains/
│   │   ├── index.md               # Domain registry with inheritance graph
│   │   ├── fiduciary-investment.md
│   │   ├── investment-analysis.md
│   │   └── regulatory-compliance.md
│   ├── tenets/
│   │   └── {domain-id}.tenets.md  # All tenets for a domain
│   └── lenses/
│       └── {domain-id}.lenses.md  # All lenses for a domain

{dialogue-output-dir}/
├── charter.md                       # Synthesized charter for this dialogue
├── charter-conflicts.md             # Conflict resolution audit trail
├── calibration-report.md          # Final compliance report (post-verdict)
└── ... (existing dialogue artifacts)
```

### Artifact Formats

**principles.md:**

```markdown
# Blue Principles

*Generated: 2026-02-02T14:30:00Z | Source: blue.db*

## Active Principles

### PR0001: Evidence Citability
**Status:** Active | **Created:** 2026-01-15

Evidence must be citable and verifiable. Claims without traceable sources
receive reduced weight in scoring.

**Rationale:** Prevents speculation from dominating deliberation.

---

### PR0002: Conflict Disclosure
**Status:** Active | **Created:** 2026-01-15

Conflicts of interest must be disclosed by experts before contributing
to relevant topics.

**Rationale:** Transparency enables appropriate weight adjustment.

---

### PR0003: Minority Voice
**Status:** Active | **Created:** 2026-01-15

Minority perspectives must be heard and addressed before convergence
can be declared.

**Rationale:** Premature consensus loses valuable dissent.
```

**domains/{domain-id}.md:**

```markdown
# Domain: Fiduciary Investment Analysis

*Generated: 2026-02-02T14:30:00Z | Source: blue.db*

| Field | Value |
|-------|-------|
| **ID** | `fiduciary-investment` |
| **Code** | `FID` |
| **Status** | Active |
| **Created** | 2026-01-20 |

## Description

Analysis framework for investment decisions where fiduciary duty to
beneficiaries is the primary obligation.

## Inheritance

```
investment-analysis
       │
       └──► fiduciary-investment ◄── regulatory-compliance
```

**Parents:**
- `investment-analysis` — General investment analysis tenets
- `regulatory-compliance` — Compliance and documentation requirements

**Children:** (none)

## Tenets (5)

| ID | Label | Priority | Status |
|----|-------|----------|--------|
| FID-TN01 | Fiduciary primacy | 1000 | Active |
| FID-TN02 | Income precedence | 900 | Active |
| FID-TN03 | Concentration awareness | 800 | Active |
| FID-TN04 | Liquidity preservation | 700 | Active |
| FID-TN05 | Documentation requirement | 600 | Active |

See: [fiduciary-investment.tenets.md](../tenets/fiduciary-investment.tenets.md)

## Lenses (3)

| ID | Label | Tenets | Overrides |
|----|-------|--------|-----------|
| FID-LN01 | Conservative Income | 3 (include) | 1 |
| FID-LN02 | Growth Oriented | 3 (include) | 1 |
| FID-LN03 | Acme Trust | 3 (include) | 1 |

See: [fiduciary-investment.lenses.md](../lenses/fiduciary-investment.lenses.md)
```

**lenses/{domain-id}.lenses.md:**

```markdown
# Lenses: Fiduciary Investment Analysis (FID)

*Generated: 2026-02-02T14:30:00Z | Source: blue.db*

## FID-LN01: Conservative Income
**Status:** Active

Income-focused configuration for clients with distribution requirements.

**Tenet Selection:**
- **Include:** FID-TN01, FID-TN02, FID-TN04
- **Exclude:** FID-TN05

**Priority Overrides:**
| Tenet | Default | Override |
|-------|---------|----------|
| FID-TN02 | 900 | 1000 |

---

## FID-LN02: Growth Oriented
**Status:** Active

Growth-focused configuration for clients with long time horizons.

**Tenet Selection:**
- **Include:** FID-TN01, FID-TN03, FID-TN05
- **Exclude:** (none)

**Priority Overrides:**
| Tenet | Default | Override |
|-------|---------|----------|
| FID-TN03 | 800 | 1000 |

---

## FID-LN03: Acme Trust
**Status:** Active

Client-specific configuration for Acme Trust requirements.

**Tenet Selection:**
- **Include:** FID-TN01, FID-TN02, FID-TN04
- **Exclude:** (none)

**Priority Overrides:**
| Tenet | Default | Override |
|-------|---------|----------|
| FID-TN02 | 900 | 950 |
```

**tenets/{domain-id}.tenets.md:**

```markdown
# Tenets: Fiduciary Investment Analysis

*Generated: 2026-02-02T14:30:00Z | Source: blue.db*

## FID-TN01: Fiduciary Primacy
**Priority:** 1000 (highest) | **Status:** Active

Fiduciary duty to beneficiaries supersedes all other considerations
including growth optimization.

**Rationale:** Legal and ethical obligation to beneficiaries is non-negotiable.

---

## FID-TN02: Income Precedence
**Priority:** 900 | **Status:** Active

Income requirements take precedence over capital gains when mandated
by governing documents.

**Rationale:** Beneficiary needs often depend on predictable income streams.

---

[... remaining tenets (FID-TN03 through FID-TN05) ...]
```

**{dialogue-dir}/charter.md:**

```markdown
# Charter: nvidia-investment-decision

*Generated: 2026-02-02T15:00:00Z | Source: blue.db*
*Charter ID: `CH0001` | Status: Approved*

## Summary

| Metric | Value |
|--------|-------|
| **Domains** | Fiduciary Investment Analysis (FID) |
| **Lenses** | FID-LN03: Acme Trust |
| **Principles** | 3 |
| **Tenets** | 5 (2 inherited, filtered by lens) |
| **Constraints** | 3 |
| **Total Rules** | 11 |
| **Conflicts Resolved** | 1 |

## Domains

| Domain | Code | Lens |
|--------|------|------|
| Fiduciary Investment Analysis | FID | FID-LN03 (Acme Trust) |

## Rules

### Principles (Universal)

| Rule ID | Source | Label | Priority |
|---------|--------|-------|----------|
| CH0001-R01 | PR0001 | Evidence must be citable | 100 |
| CH0001-R02 | PR0002 | Conflicts of interest must be disclosed | 100 |
| CH0001-R03 | PR0003 | Minority perspectives must be heard | 100 |

### Tenets (Domain + Inherited)

| Rule ID | Source | Label | Priority | Domain |
|---------|--------|-------|----------|--------|
| CH0001-R04 | FID-TN01 | Fiduciary primacy | 1000 | FID |
| CH0001-R05 | FID-TN02 | Income precedence | 950 | FID (lens override) |
| CH0001-R06 | FID-TN03 | Concentration awareness | 800 | FID |
| CH0001-R07 | INV-TN01 | Risk-adjusted returns | 500 | INV (inherited) |
| CH0001-R08 | INV-TN02 | Diversification | 400 | INV (inherited) |

### Constraints (This Dialogue)

| Rule ID | Source | Label | Priority | Origin |
|---------|--------|-------|----------|--------|
| CH0001-R09 | CN01 | 4% annual income requirement | 500 | Section 4.2 of trust document |
| CH0001-R10 | CN02 | 25% concentration limit | 500 | Trust investment policy |
| CH0001-R11 | CN03 | 60-90 day execution window | 300 | Refinancing timeline |

## Expert Prompt Injection

The following calibration block is injected into expert prompts:

```
## Charter CH0001: Fiduciary Investment Analysis

This dialogue operates under the following charter (11 rules):

**Principles:**
- [CH0001-R01] Evidence must be citable and verifiable
- [CH0001-R02] Conflicts of interest must be disclosed
- [CH0001-R03] Minority perspectives must be heard before convergence

**Tenets (by priority):**
- [CH0001-R04] [1000] Fiduciary duty supersedes growth optimization
- [CH0001-R05] [950] Income requirements take precedence over capital gains
- [CH0001-R06] [800] Concentration risk must be explicitly addressed
- [CH0001-R07] [500] Risk-adjusted returns guide position sizing
- [CH0001-R08] [400] Diversification reduces systematic risk

**Constraints (this dialogue):**
- [CH0001-R09] Acme Trust requires 4% annual income distribution
- [CH0001-R10] Single-position exposure cannot exceed 25%
- [CH0001-R11] Execution window: 60-90 days post-refinancing

Your arguments must be consistent with this charter. If you believe
a rule should be challenged, surface it as a [TENSION] referencing the rule ID
(e.g., "challenges CH0001-R05").
```
```

**{dialogue-dir}/charter-conflicts.md:**

```markdown
# Ethos Conflicts: nvidia-investment-decision

*Generated: 2026-02-02T15:00:00Z | Source: blue.db*

## Summary

| Metric | Value |
|--------|-------|
| **Conflicts Detected** | 1 |
| **Resolution Method** | Priority-based |
| **Manual Overrides** | 0 |

## XC01: Growth vs Income

**Detected:** 2026-02-02T14:58:00Z

| Side | Type | ID | Label | Priority |
|------|------|----|-------|----------|
| A | Tenet | INV-TN03 | Maximize total return | 300 |
| B | Constraint | CN01 | 4% annual income requirement | 500 |

**Conflict Type:** Contradiction

**Analysis:**
INV-TN03 prioritizes total return (growth + income), while CN01 requires
specific income levels that may constrain growth-oriented strategies.

**Resolution:** `B supersedes A`

**Reason:** Per FID-TN02 (Income precedence, priority 900), income requirements
take precedence over capital gains when mandated by governing documents.

**Resolved By:** Priority (automatic)

---

*No manual overrides were required for this charter synthesis.*
```

**{dialogue-dir}/calibration-report.md:**

```markdown
# Calibration Report: nvidia-investment-decision

*Generated: 2026-02-02T18:00:00Z (post-verdict)*

## Compliance Summary

| Status | Count | Details |
|--------|-------|---------|
| **Fully Compliant** | 9 | Rules followed without exception |
| **Exceptions** | 1 | Documented deviation with justification |
| **Violations** | 0 | No unaddressed rule breaches |

## Rule-by-Rule Compliance

| Rule | Label | Status | Notes |
|------|-------|--------|-------|
| PR0001 | Evidence citability | ✅ Compliant | 47 evidence items cited |
| PR0002 | Conflict disclosure | ✅ Compliant | No conflicts identified |
| PR0003 | Minority voice | ✅ Compliant | Dissent addressed in R2 |
| FID-TN01 | Fiduciary primacy | ✅ Compliant | Central to verdict |
| FID-TN02 | Income precedence | ✅ Compliant | Options overlay addresses |
| FID-TN03 | Concentration awareness | ✅ Compliant | 18% position recommended |
| INV-TN01 | Risk-adjusted returns | ✅ Compliant | Sharpe ratio cited |
| INV-TN02 | Diversification | ✅ Compliant | Collar reduces single-stock risk |
| **CN01** | **4% income** | ⚠️ **Exception** | See below |
| CN02 | 25% concentration | ✅ Compliant | 18% < 25% |
| CN03 | Execution window | ✅ Compliant | 75-day plan |

## Exception Details

### CN01: 4% Annual Income Requirement

**Exception Type:** Partial compliance

**Actual:** 3.2% immediate + 0.8% deferred to Q2

**Justification:**
> Options overlay achieves 3.2% immediate yield. Remaining 0.8% is covered
> by deferred premium income from collar structure, payable in Q2 after
> refinancing completes. Total annual income meets 4% requirement.

**Approved By:** Judge (in final verdict)

**Verdict Reference:** V01 (final), condition #2

---

## Tensions Raised Against Ethos

| Tension | Rule Challenged | Outcome |
|---------|-----------------|---------|
| T0003 | FID-TN03 (Concentration) | Resolved — 18% acceptable |
| T0007 | CN01 (4% income) | Resolved — Exception documented |

---

*This report was generated automatically from dialogue artifacts and verdict.*
```

### Generation Triggers

Artifacts are generated (or regenerated) when:

| Event | Artifacts Updated |
|-------|-------------------|
| `blue_charter_domain_create` | `domains/index.md`, `domains/{id}.md` |
| `blue_charter_tenet_add` | `domains/{domain}.md`, `tenets/{domain}.tenets.md` |
| `blue_charter_lens_create` | `domains/{domain}.md`, `lenses/{domain}.lenses.md` |
| `blue_principle_create` | `principles.md` |
| `blue_charter_synthesize` | `{dialogue}/charter.md`, `{dialogue}/charter-conflicts.md` |
| `blue_dialogue_verdict` (final) | `{dialogue}/calibration-report.md` |

### MCP Tool: `blue_charter_artifact_regenerate`

Force regeneration of artifacts from DB:

```json
{
  "scope": "all" | "principles" | "domain" | "dialogue",
  "domain_id": "fiduciary-investment",    // if scope=domain
  "dialogue_id": "nvidia-investment"      // if scope=dialogue
}
```

**Returns:** List of files written with paths and sizes.

### Consistency Guarantees

1. **DB is source of truth** — Artifacts are views, not authoritative
2. **Atomic generation** — All related artifacts update together
3. **Timestamp tracking** — Each artifact shows generation time and source
4. **Diff-friendly** — Markdown format enables git-based review
5. **Queryable** — Artifacts can be grep'd for human investigation

## Open Questions

1. **Principle governance**: Who can add/modify system principles? Requires RFC?
2. **Tenet versioning**: When tenets change, do existing dialogues keep old version?
3. **Cross-org tenets**: Can domains be shared across organizations?
4. **LLM conflict resolution**: How much autonomy for automated resolution vs manual review?
5. **Artifact sync**: Should artifacts be regenerated on every read, or only on write?

---

*"Calibration is not constraint — it is clarity."*
