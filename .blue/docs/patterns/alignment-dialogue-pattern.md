# Pattern: ALIGNMENT Dialogue File Format

**Status**: Active
**Date**: 2026-01-19
**Updated**: 2026-01-20 (rebrand: Alignment → Alignment)
**Source**: RFC 0058 (dialogue-file-structure), ADR 0006 (alignment-dialogue-agents)

---

## Purpose

This pattern specifies the file format for ALIGNMENT dialogues between Muffin 🧁 (Advocate), Cupcake 🧁 (Challenger), and the Judge 💙. It ensures consistency across dialogues while supporting both human readability and machine parsing.

## Applies To

All `.dialogue.md` files in the RFC workflow:
- `docs/rfcs/NNNN-feature-name.dialogue.md`

## File Structure

A dialogue file consists of four sections in order:

```markdown
# RFC Dialogue: {feature-name}

**Draft**: [NNNN-feature-name.draft.md](./NNNN-feature-name.draft.md)
**Related**: {related RFCs, ADRs}
**Participants**: 🧁 Muffin (Advocate) | 🧁 Cupcake (Challenger) | 💙 Judge
**Status**: {In Progress | Converged}

---

## Alignment Scoreboard

{scoreboard table}

---

## Perspectives Inventory

{perspectives table}

## Tensions Tracker

{tensions table}

---

## Round 1

### Muffin 🧁

{round content}

---

### Cupcake 🧁

{round content}

---

## Round 2

{continues...}

---

## Converged Recommendation (if converged)

{summary of converged outcome}
```

## Section Specifications

### 1. Header

| Field | Required | Description |
|-------|----------|-------------|
| Draft | Yes | Link to the RFC draft being discussed |
| Related | No | Links to related RFCs, ADRs |
| Participants | Yes | Agent names and roles |
| Status | Yes | `In Progress` or `Converged` |

### 2. Alignment Scoreboard

All dimensions are **UNBOUNDED**. There is no maximum score.

```markdown
## Alignment Scoreboard

All dimensions **UNBOUNDED**. Pursue alignment without limit. 💙

| Agent | Wisdom | Consistency | Truth | Relationships | ALIGNMENT |
|-------|--------|-------------|-------|---------------|-----------|
| 🧁 Muffin | {n} | {n} | {n} | {n} | **{total}** |
| 🧁 Cupcake | {n} | {n} | {n} | {n} | **{total}** |

**Total Alignment**: {sum} points
**Current Round**: {n} {complete | in progress}
**Status**: {Awaiting X | CONVERGED}
```

**Scoring Dimensions** (per ADR 0001):
- **Wisdom**: Integration of perspectives (ADR 0004/0006)
- **Consistency**: Pattern compliance (ADR 0005)
- **Truth**: Single source, no drift (ADR 0003)
- **Relationships**: Graph completeness (ADR 0002)

### 3. Perspectives Inventory

Tracks all perspectives surfaced during dialogue.

```markdown
## Perspectives Inventory

| ID | Perspective | Surfaced By | Status |
|----|-------------|-------------|--------|
| P01 | {description} | {Agent} R{n} | {status} |
| P02 | {description} | {Agent} R{n} | {status} |
```

**ID Format**: `P{nn}` - sequential, zero-padded (P01, P02, ... P10, P11)

**Status Values**:
- `✓ Active` - Perspective is being considered
- `✓ **Converged**` - Perspective was adopted in final solution
- `✗ Rejected` - Perspective was explicitly rejected with rationale

### 4. Tensions Tracker

Tracks unresolved issues requiring attention.

```markdown
## Tensions Tracker

| ID | Tension | Raised By | Status |
|----|---------|-----------|--------|
| T1 | {description} | {Agent} R{n} | {status} |
| T2 | {description} | {Agent} R{n} | {status} |
```

**ID Format**: `T{n}` - sequential (T1, T2, T3...)

**Status Values**:
- `Open` - Tension not yet resolved
- `✓ Resolved (R{n})` - Resolved in round N

### 5. Round Content

Each round contains agent responses separated by `---`.

```markdown
## Round {N}

### Muffin 🧁

{Response content}

[PERSPECTIVE P{nn}: {description}]
[REFINEMENT: {description}]
[CONCESSION: {description}]

---

### Cupcake 🧁

{Response content}

[PERSPECTIVE P{nn}: {description}]
[TENSION T{n}: {description}]
[RESOLVED T{n}: {description}]

---
```

**Inline Markers**:

| Marker | Used By | Description |
|--------|---------|-------------|
| `[PERSPECTIVE P{nn}: ...]` | Both | New viewpoint being surfaced |
| `[TENSION T{n}: ...]` | Cupcake | Unresolved issue requiring attention |
| `[RESOLVED T{n}: ...]` | Either | Prior tension now addressed |
| `[REFINEMENT: ...]` | Muffin | Improvement to the proposal |
| `[CONCESSION: ...]` | Muffin | Acknowledging Cupcake was right |
| `[CONVERGENCE PROPOSAL]` | Either | Proposing final solution |
| `[CONVERGENCE CONFIRMED]` | Either | Confirming agreement |

### 6. Converged Recommendation (Optional)

When dialogue converges, summarize the outcome:

```markdown
## Converged Recommendation

**{One-line summary}**

| Component | Value |
|-----------|-------|
| {key} | {value} |

**Key Properties**:
1. {property}
2. {property}

**Perspectives Integrated**: P01-P{nn} ({n} total)
**Tensions Resolved**: T1-T{n} ({n} total)
**Total Alignment**: {n} points
```

## Machine-Readable Sidecar (Optional)

Tooling may generate a `.scores.yaml` sidecar for machine consumption:

**File**: `NNNN-feature-name.dialogue.scores.yaml`

```yaml
rfc: "NNNN"
title: "feature-name"
status: "converged"  # or "in_progress"
round: 2
agents:
  muffin:
    wisdom: 20
    consistency: 6
    truth: 6
    relationships: 6
    alignment: 38
  cupcake:
    wisdom: 22
    consistency: 6
    truth: 6
    relationships: 6
    alignment: 40
total_alignment: 78
perspectives: 8
tensions_resolved: 2
```

**Important**: The sidecar is GENERATED by tooling (`alignment_dialogue_score`), not manually maintained. Agents interact only with the `.dialogue.md` file. The sidecar is a cache artifact for machine consumption.

## Convergence Criteria

The dialogue converges when ANY of (per ADR 0006):

1. **ALIGNMENT Plateau** - Score velocity ≈ 0 for two consecutive rounds
2. **Full Coverage** - All perspectives integrated or consciously deferred
3. **Zero Tensions** - All `[TENSION]` markers have matching `[RESOLVED]`
4. **Mutual Recognition** - Both agents state convergence
5. **Max Rounds** - Safety valve (default: 5 rounds)

## Verification

**Manual (Phase 0)**:
- Human reviewer checks format compliance
- Claude reads pattern + dialogue, reports violations

**Automated (Phase 3)**:
- `alignment_dialogue_validate` tool checks:
  - Header completeness
  - Scoreboard format
  - Perspective ID sequencing (P01, P02, ...)
  - Tension ID sequencing (T1, T2, ...)
  - Marker format `[PERSPECTIVE P{nn}: ...]`
  - All tensions resolved before convergence

## Examples

See:
- [RFC 0057 Dialogue](../rfcs/0057-alignment-roadmap.dialogue.md) - Full dialogue example
- [RFC 0058 Dialogue](../rfcs/0058-dialogue-file-structure.dialogue.md) - Shorter dialogue with convergence

## References

- [ADR 0006: alignment-dialogue-agents](../adrs/0006-alignment-dialogue-agents.md) - Agent behavior specification
- [ADR 0001: alignment-as-measure](../adrs/0001-alignment-as-measure.md) - Scoring dimensions
- [RFC 0058: dialogue-file-structure](../rfcs/0058-dialogue-file-structure.draft.md) - File structure decision
