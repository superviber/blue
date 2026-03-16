# RFC 0059: Expert-Judge Context Efficiency

**Status:** Draft
**Created:** 2026-02-04
**Author:** Eric + Claude
**Supersedes:** Portions of RFC 0036 (Expert Output Discipline)

## Problem

Current alignment dialogue architecture has unclear separation between audit artifacts and Judge context:

1. **Prompt confusion**: Experts are told to write full content to files and return only a 5-line confirmation
2. **Frequent failures**: Agents frequently don't write files (0 tool uses observed in 7/12 experts)
3. **Confirmation too sparse**: The 5-line return gives labels but no content—Judge can't synthesize
4. **Hallucination cascade**: When file writes fail and confirmations lack content, Judge fabricates expert contributions
5. **Context waste**: When experts DO include prose reasoning, Judge receives ~12k tokens it doesn't need

### Observed Failure Mode

```
12 Task agents finished
├─ Muffin Filesystem Architect · 0 tool uses    ← No file written
├─ Cupcake Knowledge Engineer · 10 tool uses    ← File written
├─ Scone AI Agent Specialist · 3 tool uses      ← File written
├─ Eclair DevEx Lead · 0 tool uses              ← No file written
├─ Donut API Designer · 0 tool uses             ← No file written
...
```

Judge then writes detailed scoreboard crediting Muffin, Eclair, Donut with specific insights they never provided.

## Analysis

### What the Judge Actually Needs

| Need | Current | Proposed |
|------|---------|----------|
| Know what perspectives were raised | Labels only | Labels + content |
| Score W/C/T/R dimensions | Must read prose | Structured markers sufficient |
| Track tensions | Labels only | Labels + content |
| Identify convergence signals | Implicit | Explicit [MOVE:CONVERGE] |
| Full reasoning chain | Not needed | Audit trail in file |

### Context Budget Comparison

| Approach | Per Expert | 12 Experts | Notes |
|----------|------------|------------|-------|
| Full prose + reasoning | ~1000 tokens | ~12k | Current when file read |
| Structured markers + content | ~300 tokens | ~3.6k | Proposed |
| Labels only (5-line) | ~50 tokens | ~600 | Current confirmation—too sparse |

**3.3x context reduction** while providing everything Judge needs.

## Proposal

### Preserve RFC 0051 Marker Syntax

Use the full marker syntax from RFC 0051 / `alignment-expert` skill:

**Local IDs:** `{EXPERT}-{TYPE}{round:02d}{seq:02d}`
- `MUFFIN-P0101` — Perspective
- `MUFFIN-R0101` — Recommendation
- `MUFFIN-T0101` — Tension
- `MUFFIN-E0101` — Evidence
- `MUFFIN-C0101` — Claim
- `MUFFIN-S0101` — Stance (NEW - this RFC)

**Cross-references:** `[RE:SUPPORT P0001]`, `[RE:OPPOSE R0001]`, `[RE:RESOLVE T0001]`, etc.

**Moves:** `[MOVE:CONVERGE]`, `[MOVE:CHALLENGE target]`, `[MOVE:CONCEDE target]`, etc.

### Tighten Output Discipline

The change is **format discipline**, not marker syntax:

```markdown
[MUFFIN-P0101: Income mandate mismatch]
NVIDIA's zero dividend conflicts with the trust's 4% income requirement.
The gap is substantial: zero income from a $2.1M position.

[MUFFIN-T0101: Growth vs income obligation]
[RE:ADDRESS T0001]
Fundamental conflict between NVIDIA's growth profile and income mandate.

[MUFFIN-R0101: Options collar structure]
[RE:RESOLVE MUFFIN-T0101]
Implement 30-delta covered call strategy. Historical premium: 2.1-2.8% monthly.

[MOVE:CHALLENGE P0023]
Prior "hold and wait" ignores opportunity cost of 8% dead weight.

---
[MUFFIN-S0101: CONDITIONAL | 0.85]
Requires options overlay to satisfy income mandate.
```

### Rules

1. **No prose preamble**: No "As a Value Analyst, I've considered..."
2. **No prose transitions**: No "Building on Cupcake's point..."
3. **Content in markers**: Each marker includes 1-3 sentences of substance
4. **Cross-refs inline**: Put `[RE:*]` on same line or immediately after marker
5. **Stance marker required**: Every expert must declare stance at end
6. **Separator required**: `---` before stance marker

### Stance: New First-Class Entity

Stance captures an expert's overall position on the dialogue question. Unlike Perspectives (observations) or Recommendations (proposals), Stance is the expert's **vote**.

#### Marker Syntax

```
[{EXPERT}-S{round}01: {stance_type} | {confidence}]
{conditions if CONDITIONAL}
```

**Stance Types:**
| Type | Meaning |
|------|---------|
| `APPROVE` | Support the proposal/direction |
| `REJECT` | Oppose the proposal/direction |
| `HOLD` | Need more information before deciding |
| `CONDITIONAL` | Support with specific conditions (must specify) |
| `ABSTAIN` | Declining to vote (conflict of interest, outside expertise) |

**Examples:**

```markdown
[MUFFIN-S0101: APPROVE | 0.90]

[CUPCAKE-S0101: CONDITIONAL | 0.75]
Requires options overlay to satisfy income mandate.

[CHURRO-S0101: REJECT | 0.60]
Concentration risk unaddressed.

[STRUDEL-S0101: HOLD | 0.50]
Need implementation evidence before committing.
```

#### Database Schema

```sql
CREATE TABLE stances (
  dialogue_id    TEXT NOT NULL,
  expert_slug    TEXT NOT NULL,
  round          INTEGER NOT NULL,
  stance_type    TEXT NOT NULL CHECK (stance_type IN ('APPROVE', 'REJECT', 'HOLD', 'CONDITIONAL', 'ABSTAIN')),
  confidence     REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
  conditions     TEXT,  -- required if CONDITIONAL
  created_at     TEXT NOT NULL DEFAULT (datetime('now')),

  PRIMARY KEY (dialogue_id, expert_slug, round),
  FOREIGN KEY (dialogue_id) REFERENCES dialogues(dialogue_id)
);

CREATE INDEX idx_stances_dialogue_round ON stances(dialogue_id, round);
```

#### Stance Tracking Across Rounds

Experts may change stance between rounds. The DB tracks history:

```
Round 0: MUFFIN-S0001: REJECT | 0.70
Round 1: MUFFIN-S0101: CONDITIONAL | 0.80 (after options proposal)
Round 2: MUFFIN-S0201: APPROVE | 0.90 (after evidence)
```

**Stance velocity** = number of stance changes in a round. High velocity indicates unresolved tensions.

#### Convergence Integration

Stance formalizes convergence tracking (RFC 0057):

```
Converge % = (APPROVE + CONDITIONAL with met conditions) / (total - ABSTAIN) × 100
```

| Metric | Calculation |
|--------|-------------|
| Unanimous | 100% APPROVE or CONDITIONAL |
| Supermajority | ≥75% |
| Majority | >50% |
| Deadlocked | No majority after max rounds |

**Confidence-weighted voting** (optional):
```
Weighted APPROVE = Σ(confidence where stance=APPROVE) / Σ(all confidence)
```

#### MCP Tools

Add to `blue_dialogue_round_register`:
```json
{
  "stances": [
    { "expert_slug": "muffin", "stance_type": "APPROVE", "confidence": 0.90 },
    { "expert_slug": "cupcake", "stance_type": "CONDITIONAL", "confidence": 0.75, "conditions": "Requires options overlay" }
  ]
}
```

Add to `blue_dialogue_round_context` response:
```json
{
  "stances": [
    { "expert_slug": "muffin", "round": 0, "stance_type": "REJECT", "confidence": 0.70 },
    { "expert_slug": "muffin", "round": 1, "stance_type": "APPROVE", "confidence": 0.90 }
  ],
  "current_stance_summary": {
    "APPROVE": 5,
    "CONDITIONAL": 2,
    "REJECT": 1,
    "HOLD": 1,
    "ABSTAIN": 0,
    "converge_percent": 77.8,
    "weighted_approve": 0.82
  }
}
```

### Prompt Construction: Judge Responsibility

The Judge builds prompts using `blue_dialogue_round_context` (RFC 0051), NOT `blue_dialogue_round_prompt`:

```
1. Judge calls blue_dialogue_round_context(dialogue_id, round)
   → Returns: experts, perspectives, tensions, open_tensions, convergence status

2. Judge constructs prompt for each expert using:
   - Context data from step 1
   - Output discipline rules (this RFC)
   - alignment-expert skill reference for marker syntax

3. Judge spawns Task with constructed prompt
   → Expert returns structured markers
   → Judge receives response directly
```

This keeps prompt construction in the Judge (flexible, no code changes for prompt tweaks) and data retrieval in MCP (structured, queryable).

### File Persistence: Judge Writes After Task Completion

After receiving Task results, Judge persists each expert's response:

```
4. Judge receives expert output from Task result
5. Judge calls blue_dialogue_expert_write(dialogue_id, round, expert_slug, content)
   → MCP writes to {output_dir}/round-{n}/{expert}.md
```

This removes "write to file" from expert responsibility—they just return structured content.

### Implementation

#### Phase 1: Prompt Construction Template (Judge-Side)

The Judge builds prompts following this template:

```rust
let prompt = format!(r##"
You are {name} {emoji}, a {role} in an ALIGNMENT-seeking dialogue.

Use the marker syntax from the alignment-expert skill:
- Local IDs: {name_upper}-P0101, {name_upper}-R0101, {name_upper}-T0101, etc.
- Cross-refs: [RE:SUPPORT P0001], [RE:RESOLVE T0001], etc.
- Moves: [MOVE:CONVERGE], [MOVE:CHALLENGE target], etc.

OUTPUT DISCIPLINE:
- NO prose preamble ("As a Value Analyst...")
- NO prose transitions ("Building on Cupcake's point...")
- NO prose conclusion ("In summary...")
- ONLY structured markers with 1-3 sentence content each
- END with: --- then one-line stance + confidence

EXAMPLE:
[{name_upper}-P0101: Income mandate mismatch]
NVIDIA's zero dividend conflicts with the trust's 4% income requirement.

[{name_upper}-T0101: Growth vs income]
[RE:ADDRESS T0001]
Fundamental conflict between growth profile and income mandate.

[{name_upper}-R0101: Options collar]
[RE:RESOLVE {name_upper}-T0101]
30-delta covered call strategy. Historical premium: 2.1-2.8% monthly.

[MOVE:CONCEDE P0023]
Donut's options proposal was directionally correct.

---
Stance: Conditional APPROVE with options overlay | Confidence: 0.85

Your output will be scored on PRECISION. One sharp insight beats ten paragraphs.
"##);
```

#### Phase 2: Add `blue_dialogue_expert_write` MCP Tool

New tool for Judge to persist expert outputs after Task completion:

```rust
/// Handle blue_dialogue_expert_write
///
/// Persist expert output to round directory for audit trail.
pub fn handle_expert_write(args: &Value) -> Result<Value, ServerError> {
    let output_dir = args.get("output_dir").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;
    let round = args.get("round").and_then(|v| v.as_u64())
        .ok_or(ServerError::InvalidParams)? as usize;
    let expert_slug = args.get("expert_slug").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;
    let content = args.get("content").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let round_dir = format!("{}/round-{}", output_dir, round);
    fs::create_dir_all(&round_dir)?;

    let output_path = format!("{}/{}.md", round_dir, expert_slug.to_lowercase());
    fs::write(&output_path, content)?;

    Ok(json!({
        "status": "success",
        "path": output_path
    }))
}
```

#### Phase 3: Remove `blue_dialogue_round_prompt`

The `blue_dialogue_round_prompt` tool conflates data retrieval with prompt construction. With this RFC:
- **Keep**: `blue_dialogue_round_context` for structured data
- **Add**: `blue_dialogue_expert_write` for persistence
- **Remove**: `blue_dialogue_round_prompt` entirely

**Rationale:**
- Prompt construction is orchestration, not data - belongs in Judge/skill
- Prompt iteration is common - shouldn't require Rust recompile
- Two approaches creates confusion
- Template text belongs in markdown, not Rust code

#### Phase 4: Update `alignment-play` Skill with Prompt Template

Add prompt template to skill (Judge fills from `round_context` data):

```markdown
## Expert Prompt Template

Build this prompt for each expert using data from `blue_dialogue_round_context`:

---

You are {expert.name} 🧁, a {expert.role} in an ALIGNMENT dialogue.

**Question:** {dialogue.question}

### Prior Round Context

**Open Tensions:**
{for t in open_tensions}
- {t.id}: {t.label} — {t.description}
{/for}

**Key Perspectives:**
{for p in perspectives where p.round == round - 1}
- {p.id}: {p.label} — {p.content}
{/for}

### Output Discipline (RFC 0059)

Return ONLY structured markers. No prose preamble. No transitions. No conclusion.

Use marker syntax from alignment-expert skill:
- Local IDs: {EXPERT}-P{round}01, {EXPERT}-T{round}01, etc.
- Cross-refs: [RE:SUPPORT P0001], [RE:RESOLVE T0001]
- Moves: [MOVE:CONVERGE], [MOVE:CHALLENGE target]
- Stance: REQUIRED - your vote on the question

End with:
---
[{EXPERT}-S{round}01: {APPROVE|REJECT|HOLD|CONDITIONAL|ABSTAIN} | {confidence}]
{conditions if CONDITIONAL}

Your contribution is scored on PRECISION. One sharp insight beats ten paragraphs.

---
```

Update skill workflow:
1. Call `blue_dialogue_round_context(dialogue_id, round)` for data
2. Build prompts using template above
3. Spawn all experts in parallel via Task
4. Receive structured marker responses
5. Call `blue_dialogue_expert_write` for each expert to persist
6. Score and synthesize

## Success Criteria

1. **Zero hallucination**: Judge only scores perspectives actually returned
2. **100% file capture**: All expert outputs persisted for audit
3. **Context efficiency**: <4k tokens for 12-expert round
4. **Clear failure mode**: If expert returns empty, Judge explicitly notes "no contribution"
5. **Stance tracking**: Every expert declares stance each round; history preserved
6. **Convergence calculation**: Automatic converge % from stance data, not manual counting

## Migration

- **RFC 0051** (Global Perspective Tension Tracking): Marker syntax preserved unchanged
- **RFC 0036** (Expert Output Discipline): Verbosity guidance superseded by stricter rules
- **alignment-expert skill**: Referenced for syntax, not duplicated in prompts
- **alignment-play skill**: Currently inconsistent—shows both `round_prompt` (lines 91, 119, 306) and `round_context` (line 254). Must be updated to use only `round_context` + Judge-built prompts.
- Existing dialogues unaffected (different prompt version)
- New dialogues use updated prompt template with output discipline

### Skill Updates Required

**alignment-play/SKILL.md:**
- Remove ALL references to `blue_dialogue_round_prompt` (lines 91, 119, 122-131, 225, 306, 314)
- Add prompt template section (Judge constructs prompts)
- Update workflow to use `blue_dialogue_round_context` + Judge prompt construction
- Add `blue_dialogue_expert_write` call after Task completion
- Add output discipline rules

**MCP Code:**
- Add `blue_dialogue_expert_write` handler
- Remove `handle_round_prompt` function from `dialogue.rs`
- Remove tool registration for `blue_dialogue_round_prompt`
- Add `stances` table to SQLite schema (`alignment_db.rs`)
- Add `register_stance` function
- Update `blue_dialogue_round_register` to accept stances
- Update `blue_dialogue_round_context` to return stance history + summary

## Alternatives Considered

### A: Keep file-primary, fix agent compliance

Problem: Can't force subagents to use Write tool. They have autonomy.

### B: Full prose to Judge, summarize later

Problem: 12k+ tokens per round is expensive and mostly wasted.

### C: Two-phase (agents write, Judge reads files)

Problem: Adds latency, requires Judge to glob/read, still fails if agents don't write.

### D: Keep `blue_dialogue_round_prompt` alongside `round_context`

Problem: Two ways to do the same thing creates confusion. Prompt construction is orchestration (Judge domain), not data retrieval (MCP domain). Prompt changes shouldn't require Rust recompile.

## Decision

**Adopt structured markers as canonical format, MCP-side file capture for audit.**

This inverts the current model: experts return content (not confirmation), MCP handles persistence (not experts).
