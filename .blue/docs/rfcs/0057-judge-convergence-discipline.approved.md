# RFC 0057: Judge Convergence Discipline

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-02
**Relates To:** ADR 0014, RFC 0050, RFC 0051

## Overview

Fixes convergence discipline with four changes:
1. **Redefine Velocity** — `velocity = open_tensions + new_perspectives` (work remaining, not score delta)
2. **Require Unanimous Convergence** — 100% of experts must signal `[MOVE:CONVERGE]`
3. **Both Conditions Required** — Convergence needs velocity = 0 AND unanimous agreement
4. **Persist Dialogues** — Store in `.blue/dialogues/` not `/tmp` for visibility

Default max rounds: **10**

## Problem Statement

### Bug 1: Tension Gate Bypass

In default mode (`rotation: graduated`), the Judge sometimes jumps directly to RFC creation without first resolving open tensions. The Judge Protocol contains `tension_resolution_gate: true`, but the SKILL.md lacks explicit enforcement instructions, leading to premature convergence declarations.

**Observed Behavior:**
- Judge declares convergence
- Judge proposes/creates RFC
- Open tensions remain unaddressed

**Expected Behavior:**
- Judge checks open tensions
- If tensions exist → continue dialogue with targeted expertise
- Only when tensions resolved → declare convergence → create RFC

### Issue 2: Velocity Misdefined

Velocity was defined as score delta between rounds — a derivative metric meant to detect "diminishing returns." This is problematic:

- Threshold tuning (what's "low enough"?)
- Misreporting (showing 0 when actual delta was 89)
- Derivative metric when primary signals are clearer

**Proposal:** Redefine velocity as "work remaining":

```
Velocity = open_tensions + new_perspectives
```

When velocity = 0:
- No open tensions (conflicts resolved)
- No new perspectives surfaced (coverage complete)

This is a direct measurement, not a derivative.

## Proposed Solution

### 1. Redefined Convergence Criteria

Convergence requires ALL of:

| Condition | How Measured |
|-----------|--------------|
| **Velocity = 0** | `open_tensions + new_perspectives == 0` |
| **Unanimous Recognition** | 100% of experts signal `[MOVE:CONVERGE]` |

Plus safety valve:
| **Max Rounds** | 10 rounds (default) — forces convergence even if conditions not met |

**Both conditions must be true** (except max rounds override). This is stricter than before:
- Can't converge with open tensions
- Can't converge while new perspectives are still emerging
- Can't converge without unanimous expert agreement

### 2. Explicit Tension Gate in Judge Workflow

Add a mandatory "Tension Gate Check" after each round.

**After Round N completes:**

```
TENSION_GATE:
1. Query open tensions from round context
2. IF open_tension_count > 0:
   a. Analyze each open tension
   b. For each tension:
      - Does current panel have expertise to resolve? → retain expert, assign directive
      - Does pool have relevant expert? → pull from pool
      - Neither? → create targeted expert
   c. Evolve panel via `blue_dialogue_evolve_panel`
   d. Spawn next round with tension-focused prompts
   e. RETURN TO TENSION_GATE after round completes
3. IF open_tension_count == 0:
   → Proceed to convergence declaration
```

### 3. Round Summary Artifacts

**Mandatory in `round-N.summary.md`:**

```markdown
## Velocity Components

### Open Tensions: 1
| ID | Label | Status | Owner | Resolution Path |
|----|-------|--------|-------|-----------------|
| T0001 | Income mandate conflict | OPEN | Muffin | Covered call analysis next round |
| T0002 | Concentration risk | RESOLVED | Scone | Diversification accepted |

### New Perspectives This Round: 2
| ID | Label | Contributor |
|----|-------|-------------|
| P0201 | Options overlay strategy | Cupcake |
| P0202 | Tax-loss harvesting window | Donut |

### Convergence Signals: 3/6 (50%)
| Expert | Signal |
|--------|--------|
| Muffin | `[MOVE:CONVERGE]` ✓ |
| Cupcake | `[MOVE:CONVERGE]` ✓ |
| Scone | `[MOVE:CONVERGE]` ✓ |
| Donut | — |
| Eclair | — |
| Brioche | — |

## Velocity: 3 (1 tension + 2 perspectives)
## Converge %: 50%
## Convergence Blocked: Yes (velocity > 0, converge < 100%)
```

The Judge MUST write this after every round. Convergence requires velocity = 0 AND converge % = 100%.

### 4. Convergence Decision Tree

```
                    ┌─────────────────┐
                    │  Round Complete │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ Calculate:      │
                    │ - ALIGNMENT     │
                    │ - Velocity      │
                    │ - Converge %    │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ Max Rounds?     │──YES──► FORCE CONVERGENCE
                    └────────┬────────┘         (with warning)
                             │ NO
                    ┌────────▼────────┐
                    │ Velocity = 0?   │──NO───► NEXT ROUND
                    └────────┬────────┘         (resolve tensions,
                             │ YES              surface perspectives)
                    ┌────────▼────────┐
                    │ 100% Converge?  │──NO───► NEXT ROUND
                    └────────┬────────┘         (experts not aligned)
                             │ YES
                    ┌────────▼────────┐
                    │ DECLARE         │
                    │ CONVERGENCE     │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ Create RFC or   │
                    │ Final Verdict   │
                    └─────────────────┘
```

Both conditions must pass: `velocity == 0` AND `converge_percent == 100%`.

### 5. Tension-Focused Prompts

When spawning a round specifically to address tensions, the Judge SHOULD include tension directives in the prompts:

```markdown
## Directive for This Round

**Priority Tensions:**
- T0001: Income mandate conflict - You have been retained/summoned to address this
- Action: Propose concrete resolution or explain why tension is acceptable

**Expected Output:**
- Either `[RE:RESOLVE T0001]` with supporting analysis
- Or `[RE:ADDRESS T0001]` explaining why resolution is blocked
```

### 6. Acceptable Unresolved Tensions

Some tensions may be legitimately unresolvable (fundamental tradeoffs). The Judge MAY declare convergence with open tensions IF:

1. The tension is explicitly marked as `[ACCEPTED UNRESOLVED]`
2. The final verdict acknowledges the unresolved tension
3. The RFC/recommendation accounts for the tradeoff

**Syntax in summary:**

```markdown
| T0003 | Growth vs. income | ACCEPTED UNRESOLVED | — | Fundamental tradeoff, mitigated by position sizing |
```

This is different from "forgot to resolve" — it's a conscious decision documented in the record.

### 7. Scoreboard Format

Track both ALIGNMENT components and convergence metrics.

**New format:**

```markdown
## Scoreboard

| Round | W | C | T | R | Score | Open Tensions | New Perspectives | Velocity | Converge % |
|-------|---|---|---|---|-------|---------------|------------------|----------|------------|
| 0     | 45| 30| 25| 25| 125   | 3             | 8                | 11       | 0%         |
| 1     | 32| 22| 18| 17| 89    | 1             | 2                | 3        | 50%        |
| 2     | 18| 12| 8 | 7 | 45    | 0             | 0                | 0        | 100%       |

**Total ALIGNMENT:** 259 (W:95 C:64 T:51 R:49)
**Max Rounds:** 10
**Convergence:** ✓ (velocity=0, unanimous)
```

**ALIGNMENT Components:**
- **W** = Wisdom (perspectives integrated, synthesis quality)
- **C** = Consistency (follows patterns, internally coherent)
- **T** = Truth (grounded in reality, no contradictions)
- **R** = Relationships (connections to other artifacts, graph completeness)

**Convergence Metrics:**
- **Velocity** = Open Tensions + New Perspectives ("work remaining")
- **Converge %** = percentage of experts who signaled `[MOVE:CONVERGE]`

When velocity hits 0 and converge % hits 100%, the dialogue is complete.

#### Convergence Summary Template

When convergence is achieved, the Judge outputs a final summary:

```
100% CONVERGENCE ACHIEVED

Final Dialogue Summary
┌───────────────────────┬───────────┐
│        Metric         │   Value   │
├───────────────────────┼───────────┤
│ Rounds                │ 3         │
├───────────────────────┼───────────┤
│ Total ALIGNMENT       │ 289       │
│   (W:98 C:72 T:65 R:54)           │
├───────────────────────┼───────────┤
│ Experts Consulted     │ 10 unique │
├───────────────────────┼───────────┤
│ Tensions Resolved     │ 6/6       │
├───────────────────────┼───────────┤
│ Final Velocity        │ 0         │
└───────────────────────┴───────────┘

Converged Decisions
┌────────────────┬────────────────────────────────────────────────────┐
│   Topic        │                      Decision                      │
├────────────────┼────────────────────────────────────────────────────┤
│ Architecture   │ Storage abstraction with provider trait            │
├────────────────┼────────────────────────────────────────────────────┤
│ Key Hierarchy  │ UMK→KEK→DEK, identical code path everywhere        │
├────────────────┼────────────────────────────────────────────────────┤
│ Local Dev      │ Docker required, DynamoDB Local mandatory          │
├────────────────┼────────────────────────────────────────────────────┤
│ Testing        │ NIST KAT + reference vectors + property tests      │
└────────────────┴────────────────────────────────────────────────────┘

Resolved Tensions
┌────────┬────────────────────────────────────────────────────────────┐
│   ID   │                       Resolution                           │
├────────┼────────────────────────────────────────────────────────────┤
│ T0001  │ Local keys never rotated - disposable with DB reset        │
├────────┼────────────────────────────────────────────────────────────┤
│ T0002  │ Audit logs include trace_id with hash chain                │
└────────┴────────────────────────────────────────────────────────────┘

All experts signaled [MOVE:CONVERGE]. Velocity = 0.
```

This summary goes in the final `verdict.md` and is displayed to the user.

### 8. SKILL.md Updates

**Update** Key Rules:

```markdown
8. **TRACK VELOCITY CORRECTLY** — Velocity = open_tensions + new_perspectives. This is "work remaining," not score delta. Convergence requires velocity = 0.

9. **REQUIRE UNANIMOUS CONVERGENCE** — All experts must signal `[MOVE:CONVERGE]` before declaring convergence. 100%, not majority.

10. **BOTH CONDITIONS REQUIRED** — Convergence requires BOTH velocity = 0 AND 100% expert convergence. Either condition failing means another round.

11. **MAX ROUNDS = 10** — Safety valve. If round 10 completes without convergence, force it with a warning in the verdict.

12. **OUTPUT CONVERGENCE SUMMARY** — When convergence achieved, output the formatted summary table showing: rounds, total ALIGNMENT (with W/C/T/R breakdown), experts consulted, tensions resolved, converged decisions, and resolved tensions.
```

**Add** new section "Driving Velocity to Zero":

```markdown
## Driving Velocity to Zero

When velocity > 0 after a round, the Judge must drive it down:

### If Open Tensions > 0:
1. **Audit**: List tensions without `[RE:RESOLVE]` or `[ACCEPTED UNRESOLVED]`
2. **Assign Ownership**: Each tension needs an expert responsible for resolution
3. **Evolve Panel if Needed**: Pull from pool or create targeted expert
4. **Spawn Round**: Prompts emphasize tension resolution

### If New Perspectives > 0:
1. **Assess**: Are these perspectives being integrated or creating new tensions?
2. **If creating tensions**: Address tensions first
3. **If integrating smoothly**: Continue — perspectives will stop emerging as coverage completes

### If Converge % < 100%:
1. **Identify Holdouts**: Which experts haven't signaled `[MOVE:CONVERGE]`?
2. **Understand Why**: Are they defending open tensions? Surfacing new perspectives?
3. **Address Root Cause**: Usually ties back to velocity > 0

**Example:**

Round 2 Summary:
- Velocity: 3 (1 tension + 2 perspectives)
- Converge %: 50%

Analysis:
- T0001 still open — Muffin defending income mandate
- P0201, P0202 are new this round — still being integrated
- Donut, Eclair, Brioche haven't converged — waiting on T0001 resolution

Action: Creating "Income Strategist" to address T0001. Expecting velocity → 0 next round.
```

### 9. Dialogue Storage Location

**Current:** Dialogues stored in `/tmp/blue-dialogue/<name>/`

**Problem:**
- Invisible to users browsing the repo
- Lost on reboot
- Not tracked in git
- Hard to find and reference later

**Proposed:** Store in `.blue/dialogues/<name>/`

```
.blue/
├── docs/
│   ├── adrs/
│   └── rfcs/
└── dialogues/                    ← NEW
    └── 2026-02-03T1423Z-nvidia-investment/
        ├── dialogue.md
        ├── expert-pool.json
        ├── round-0/
        │   ├── panel.json
        │   ├── muffin.md
        │   ├── cupcake.md
        │   └── round-0.summary.md
        ├── round-1/
        │   └── ...
        ├── scoreboard.md
        └── verdict.md
```

**Benefits:**
- Visible in repo structure
- Can be git-tracked (optional)
- Easy to reference: "see `.blue/dialogues/2026-02-03T1423Z-nvidia-investment/`"
- Survives reboots
- Natural home alongside other Blue artifacts
- ISO prefix sorts chronologically

**Naming convention:** `<ISO-8601>-<topic>/`

Format: `YYYY-MM-DDTHHMMZ-<topic>` (same as spikes)

Examples:
- `2026-02-03T1423Z-nvidia-investment/`
- `2026-02-03T0900Z-rfc-0057-convergence-discipline/`

### 10. Database & MCP Integration

Extend the RFC 0051 schema to track velocity and convergence with safeguards.

#### Schema Extension

```sql
-- ================================================================
-- ROUNDS (extends existing - add ALIGNMENT components & convergence)
-- ================================================================
ALTER TABLE rounds ADD COLUMN score_wisdom INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN score_consistency INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN score_truth INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN score_relationships INTEGER NOT NULL DEFAULT 0;
-- score already exists as total; can be computed or stored

ALTER TABLE rounds ADD COLUMN open_tensions INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN new_perspectives INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN velocity INTEGER GENERATED ALWAYS AS (open_tensions + new_perspectives) STORED;
ALTER TABLE rounds ADD COLUMN converge_signals INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN panel_size INTEGER NOT NULL DEFAULT 0;
ALTER TABLE rounds ADD COLUMN converge_percent REAL GENERATED ALWAYS AS (
  CASE WHEN panel_size > 0 THEN (converge_signals * 100.0 / panel_size) ELSE 0 END
) STORED;

-- Convergence gate constraint: verdict requires velocity=0 AND converge_percent=100
-- (enforced at MCP layer, not DB, for better error messages)

-- ================================================================
-- CONVERGENCE_SIGNALS (track per-expert convergence signals)
-- ================================================================
CREATE TABLE convergence_signals (
  dialogue_id TEXT NOT NULL,
  round INTEGER NOT NULL,
  expert_name TEXT NOT NULL,
  signaled_at TEXT NOT NULL,  -- ISO 8601 timestamp

  PRIMARY KEY (dialogue_id, round, expert_name),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id)
);

CREATE INDEX idx_convergence_by_round ON convergence_signals(dialogue_id, round);

-- ================================================================
-- SCOREBOARD (denormalized view for efficient queries)
-- ================================================================
CREATE VIEW scoreboard AS
SELECT
  r.dialogue_id,
  r.round,
  r.score_wisdom AS W,
  r.score_consistency AS C,
  r.score_truth AS T,
  r.score_relationships AS R,
  r.score AS total,
  r.open_tensions,
  r.new_perspectives,
  r.velocity,
  r.converge_signals,
  r.panel_size,
  r.converge_percent,
  (SELECT SUM(score) FROM rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_score,
  (SELECT SUM(score_wisdom) FROM rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_W,
  (SELECT SUM(score_consistency) FROM rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_C,
  (SELECT SUM(score_truth) FROM rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_T,
  (SELECT SUM(score_relationships) FROM rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_R
FROM rounds r
ORDER BY r.dialogue_id, r.round;
```

#### Export Format Extension

The `blue_dialogue_export` output includes the full scoreboard:

```json
{
  "dialogue_id": "nvidia-investment",
  "title": "NVIDIA Investment Decision",
  "scoreboard": {
    "rounds": [
      {
        "round": 0,
        "score": { "W": 45, "C": 30, "T": 25, "R": 25, "total": 125 },
        "velocity": { "open_tensions": 3, "new_perspectives": 8, "total": 11 },
        "convergence": { "signals": 0, "panel_size": 6, "percent": 0 }
      },
      {
        "round": 1,
        "score": { "W": 32, "C": 22, "T": 18, "R": 17, "total": 89 },
        "velocity": { "open_tensions": 1, "new_perspectives": 2, "total": 3 },
        "convergence": { "signals": 3, "panel_size": 6, "percent": 50 }
      },
      {
        "round": 2,
        "score": { "W": 18, "C": 12, "T": 8, "R": 7, "total": 45 },
        "velocity": { "open_tensions": 0, "new_perspectives": 0, "total": 0 },
        "convergence": { "signals": 6, "panel_size": 6, "percent": 100 }
      }
    ],
    "totals": {
      "rounds": 3,
      "alignment": { "W": 95, "C": 64, "T": 51, "R": 49, "total": 259 },
      "experts_consulted": 10,
      "tensions_resolved": 6,
      "final_velocity": 0,
      "convergence_achieved": true,
      "convergence_reason": "velocity=0, unanimous"
    }
  },
  "convergence_signals": [
    { "round": 1, "expert": "muffin", "signaled_at": "2026-02-03T15:23:00Z" },
    { "round": 1, "expert": "cupcake", "signaled_at": "2026-02-03T15:24:00Z" },
    { "round": 1, "expert": "scone", "signaled_at": "2026-02-03T15:25:00Z" },
    { "round": 2, "expert": "donut", "signaled_at": "2026-02-03T16:10:00Z" },
    { "round": 2, "expert": "eclair", "signaled_at": "2026-02-03T16:11:00Z" },
    { "round": 2, "expert": "brioche", "signaled_at": "2026-02-03T16:12:00Z" }
  ],
  "experts": [...],
  "rounds": [...],
  "perspectives": [...],
  "tensions": [...],
  "verdicts": [...]
}
```

#### Query Examples

```sql
-- Get scoreboard for a dialogue
SELECT * FROM scoreboard WHERE dialogue_id = 'nvidia-investment';

-- Check if dialogue can converge
SELECT
  dialogue_id,
  round,
  velocity,
  converge_percent,
  CASE
    WHEN velocity = 0 AND converge_percent = 100 THEN 'CAN_CONVERGE'
    WHEN velocity > 0 THEN 'BLOCKED: velocity=' || velocity
    ELSE 'BLOCKED: converge=' || converge_percent || '%'
  END AS status
FROM scoreboard
WHERE dialogue_id = 'nvidia-investment'
ORDER BY round DESC
LIMIT 1;

-- Get cumulative ALIGNMENT breakdown
SELECT
  dialogue_id,
  MAX(cumulative_score) AS total_alignment,
  MAX(cumulative_W) AS wisdom,
  MAX(cumulative_C) AS consistency,
  MAX(cumulative_T) AS truth,
  MAX(cumulative_R) AS relationships
FROM scoreboard
WHERE dialogue_id = 'nvidia-investment';
```

#### MCP Validation Layer

**New validation rules (enforced before DB operations):**

| Error Code | Trigger | Message Template |
|------------|---------|------------------|
| `velocity_not_zero` | Verdict with velocity > 0 | `Cannot register verdict: velocity={velocity} (open_tensions={ot}, new_perspectives={np}). Resolve tensions and integrate perspectives first.` |
| `convergence_not_unanimous` | Verdict with converge < 100% | `Cannot register verdict: convergence={pct}% ({signals}/{panel}). All experts must signal [MOVE:CONVERGE].` |
| `forced_convergence_no_warning` | Max rounds verdict without warning | `Forced convergence at max rounds requires explicit warning in verdict description.` |

**Validation order for `blue_dialogue_verdict_register`:**

1. **Round exists** — Check round data is registered
2. **Velocity gate** — Check `velocity == 0` (or max rounds reached)
3. **Convergence gate** — Check `converge_percent == 100` (or max rounds reached)
4. **Forced convergence warning** — If max rounds, require warning text

**Error response format (per RFC 0051 pattern):**

```json
{
  "status": "error",
  "error_code": "velocity_not_zero",
  "message": "Cannot register verdict: velocity=3 (open_tensions=1, new_perspectives=2)",
  "field": "velocity",
  "value": 3,
  "constraint": "convergence_gate",
  "suggestion": "Resolve T0001 and integrate P0201, P0202 before declaring convergence",
  "context": {
    "open_tensions": ["T0001"],
    "new_perspectives": ["P0201", "P0202"],
    "converge_percent": 50,
    "missing_signals": ["Donut", "Eclair", "Brioche"]
  }
}
```

#### MCP Tool Updates

**`blue_dialogue_round_register`** — Add required fields:

```json
{
  "dialogue_id": "nvidia-investment",
  "round": 2,
  "score": 45,
  "score_components": { "W": 18, "C": 12, "T": 8, "R": 7 },
  "open_tensions": 0,
  "new_perspectives": 0,
  "converge_signals": ["Muffin", "Cupcake", "Scone", "Donut", "Eclair", "Brioche"],
  "panel": ["Muffin", "Cupcake", "Scone", "Donut", "Eclair", "Brioche"],
  "perspectives": [...],
  "tensions": [...],
  "expert_scores": {...}
}
```

**`blue_dialogue_round_context`** — Return velocity/convergence in response:

```json
{
  "round": 2,
  "velocity": {
    "open_tensions": 0,
    "new_perspectives": 0,
    "total": 0
  },
  "convergence": {
    "signals": 6,
    "panel_size": 6,
    "percent": 100,
    "missing": []
  },
  "can_converge": true,
  "convergence_blockers": []
}
```

**`blue_dialogue_verdict_register`** — Gated by validation:

```json
{
  "dialogue_id": "nvidia-investment",
  "verdict_id": "final",
  "verdict_type": "final",
  "round": 2,
  "recommendation": "APPROVE with options overlay",
  "forced": false  // true only if max rounds reached
}
```

If `forced: true`, requires `warning` field explaining why convergence was forced.

#### Atomic Transactions

Per RFC 0051 pattern:
- **All-or-nothing**: If validation fails, entire operation rejected
- **Detailed errors**: Response includes all validation failures
- **Judge recovery**: Structured errors allow programmatic correction

### 11. ADR 0014 Update

Update ADR 0014 convergence criteria:

**Current:**
```markdown
1. **Plateau**: Velocity ≈ 0 for two consecutive rounds
2. **Full Coverage**: All perspectives integrated
3. **Zero Tensions**: All `[TENSION]` markers have `[RESOLVED]`
4. **Mutual Recognition**: Majority signal `[CONVERGENCE CONFIRMED]`
5. **Max Rounds**: Safety valve reached
```

**Proposed:**
```markdown
Convergence requires ALL of:
1. **Velocity = 0**: open_tensions + new_perspectives == 0
2. **Unanimous Recognition**: 100% of experts signal `[MOVE:CONVERGE]`

Safety valve:
3. **Max Rounds**: 10 rounds (forces convergence with warning)
```

Key changes:
- Velocity redefined as "work remaining" (open tensions + new perspectives)
- Unanimous expert agreement required (not majority)
- Max rounds increased to 10
- Both conditions must be true (AND, not OR)

## Implementation

### Phase 1: SKILL.md Update ✅
- [x] Update velocity definition: `open_tensions + new_perspectives`
- [x] Add unanimous convergence requirement
- [x] Update max rounds to 10
- [x] Update scoreboard format example (W/C/T/R columns)
- [x] Update default output_dir to `.blue/dialogues/`
- [x] Add "Driving Velocity to Zero" section
- [x] Add convergence summary template

### Phase 2: ADR 0014 Update ✅
- [x] Redefine velocity as "work remaining"
- [x] Change from OR to AND logic for convergence
- [x] Change from majority to unanimous expert agreement
- [x] Update max rounds to 10

### Phase 3: Database Schema Update ✅
- [x] Add columns to `rounds` table: `score_wisdom`, `score_consistency`, `score_truth`, `score_relationships`
- [x] Add columns to `rounds` table: `open_tensions`, `new_perspectives`, `converge_signals`, `panel_size`
- [x] Add computed columns: `velocity`, `converge_percent`
- [x] Create `convergence_signals` table for per-expert tracking
- [x] Create `scoreboard` view for efficient queries
- [x] Add indices for query performance
- [x] Add RFC 0057 tests (42 tests pass)

### Phase 4: MCP Validation Layer ✅
- [x] Add `velocity_not_zero` validation rule
- [x] Add `convergence_not_unanimous` validation rule
- [x] Add `forced_convergence_no_warning` validation rule
- [x] Return structured errors with context (open tensions, missing signals)
- [x] Atomic transactions: all-or-nothing with detailed errors

### Phase 5: MCP Tool Updates ✅
- [x] `blue_dialogue_round_register`: Add `open_tensions`, `new_perspectives`, `converge_signals`, `panel_size`, `score_components`
- [x] `blue_dialogue_round_context`: Return velocity/convergence status and blockers
- [x] `blue_dialogue_verdict_register`: Gate on velocity=0 AND converge=100% (or forced with warning)
- [x] `blue_dialogue_create`: Default `output_dir` to `.blue/dialogues/<ISO>-<name>`
- [x] `blue_dialogue_export`: Include full scoreboard with cumulative totals

### Phase 6: Judge Protocol Update ✅
- [x] Replace `velocity_threshold` with velocity calculation instructions
- [x] Add `converge_percent` tracking requirement
- [x] Update `max_rounds` default to 10
- [x] Document convergence gate validation
- [x] Add scoreboard format with RFC 0057 columns

### Phase 7: Create .blue/dialogues/ Directory ✅
- [x] Add `.blue/dialogues/.gitkeep` or initial README
- [ ] Update `.gitignore` if dialogues should not be tracked (optional)

### Phase 8: CLI Parity (High Priority) ✅
- [x] Add `blue dialogue` subcommand with all dialogue tools
- [x] Add `blue adr` subcommand with all ADR tools
- [x] Add `blue spike` subcommand with spike tools
- [x] CLI calls same handler functions as MCP tools (single implementation)
- [x] Add `--help` documentation for all new commands

### Phase 9: CLI Parity (Medium Priority) ✅
- [x] Add `blue audit` subcommand
- [x] Add `blue prd` subcommand
- [x] Add `blue reminder` subcommand

### Phase 10: CLI Parity (Low Priority) ⏳
- [ ] Add `blue staging` subcommand (if not already complete)
- [ ] Add `blue llm` subcommand
- [ ] Add `blue postmortem` subcommand
- [ ] Add `blue runbook` subcommand

## Success Criteria

1. Velocity = open_tensions + new_perspectives (not score delta)
2. Convergence requires velocity = 0 AND 100% expert convergence
3. Max rounds = 10 (safety valve)
4. Scoreboard shows: W, C, T, R, Score, Open Tensions, New Perspectives, Velocity, Converge %
5. Judge never declares convergence unless both conditions met (except max rounds)
6. Forced convergence at max rounds includes warning in verdict
7. Dialogues stored in `.blue/dialogues/<ISO>-<name>/` by default
8. MCP blocks verdict registration when velocity > 0 (structured error with context)
9. MCP blocks verdict registration when converge < 100% (lists missing signals)
10. All validation errors return actionable suggestions per RFC 0051 pattern
11. Database stores W/C/T/R components per round (not just total score)
12. `scoreboard` view provides efficient query access to all convergence metrics
13. `blue_dialogue_export` includes full scoreboard with cumulative totals
14. Convergence signals tracked per-expert with timestamps for audit trail

### 12. CLI Parity for MCP Commands

**Bug:** The Judge tried to run `blue dialogue create` as a CLI command, but it doesn't exist. Many MCP tools lack CLI equivalents, causing silent failures when invoked from bash.

**Problem:**
```bash
# These don't exist as CLI commands:
blue dialogue create --title "..."
blue adr create --title "..."
blue spike create --title "..."

# Only MCP tools work:
blue_dialogue_create(title="...")
```

**Proposed:** Add CLI subcommands that wrap MCP functionality for all tool groups.

#### Dialogue Commands (Priority: High)

```bash
blue dialogue create --title "..." --question "..." --output-dir ".blue/dialogues/..."
blue dialogue list
blue dialogue get --id "..."
blue dialogue round-context --id "..." --round 1
blue dialogue round-register --id "..." --round 1 --data round-1.json
blue dialogue evolve-panel --id "..." --round 1 --panel panel.json
blue dialogue verdict --id "..." --round 2
blue dialogue export --id "..."
```

| CLI Command | MCP Tool |
|-------------|----------|
| `blue dialogue create` | `blue_dialogue_create` |
| `blue dialogue list` | `blue_dialogue_list` |
| `blue dialogue get` | `blue_dialogue_get` |
| `blue dialogue round-context` | `blue_dialogue_round_context` |
| `blue dialogue round-register` | `blue_dialogue_round_register` |
| `blue dialogue evolve-panel` | `blue_dialogue_evolve_panel` |
| `blue dialogue verdict` | `blue_dialogue_verdict_register` |
| `blue dialogue export` | `blue_dialogue_export` |

#### ADR Commands (Priority: High)

```bash
blue adr create --title "..." --status accepted
blue adr list
blue adr get --number 0014
blue adr relevant --query "alignment scoring"
blue adr audit
```

| CLI Command | MCP Tool |
|-------------|----------|
| `blue adr create` | `blue_adr_create` |
| `blue adr list` | `blue_adr_list` |
| `blue adr get` | `blue_adr_get` |
| `blue adr relevant` | `blue_adr_relevant` |
| `blue adr audit` | `blue_adr_audit` |

#### Spike Commands (Priority: Medium)

```bash
blue spike create --title "..." --rfc 0057
blue spike complete --path ".blue/docs/spikes/..."
```

| CLI Command | MCP Tool |
|-------------|----------|
| `blue spike create` | `blue_spike_create` |
| `blue spike complete` | `blue_spike_complete` |

#### Audit Commands (Priority: Medium)

```bash
blue audit create --title "..." --scope "security"
blue audit list
blue audit get --id "..."
blue audit complete --id "..."
```

#### Other Commands (Priority: Low)

| Group | CLI Commands | MCP Tools |
|-------|--------------|-----------|
| `blue prd` | create, get, approve, complete, list | `blue_prd_*` |
| `blue reminder` | create, list, snooze, clear | `blue_reminder_*` |
| `blue staging` | create, destroy, lock, unlock, status, cost | `blue_staging_*` |
| `blue llm` | start, stop, status, providers | `blue_llm_*` |
| `blue postmortem` | create, action-to-rfc | `blue_postmortem_*` |
| `blue runbook` | create, update, lookup, actions | `blue_runbook_*` |

#### Implementation Pattern

All CLI commands wrap MCP handlers (single implementation):

```rust
// In blue-cli/src/main.rs
Commands::Dialogue { command } => match command {
    DialogueCommands::Create { title, question, output_dir } => {
        // Call same handler as MCP
        let result = dialogue::handle_dialogue_create(&json!({
            "title": title,
            "question": question,
            "output_dir": output_dir,
        })).await?;
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
}
```

This ensures CLI and MCP always behave identically.

## Risks

| Risk | Mitigation |
|------|------------|
| Infinite loops (tensions create more tensions) | Max rounds safety valve still applies |
| Over-aggressive tension hunting | Judge discretion on materiality |
| Slower convergence | Correct convergence > fast convergence |
| CLI/MCP parity drift | Single implementation, CLI wraps MCP handlers |

## Open Questions

1. Should MCP tools hard-block verdict registration with open tensions?
2. Should there be a "tension materiality" threshold (ignore minor tensions)?
3. Should ACCEPTED UNRESOLVED require unanimous expert acknowledgment?

---

*"The elephant cannot be described while the blind men are still arguing about the trunk."*
