# RFC 0039: ADR Architecture — Greenfield Clarifications

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-27 |
| **Dialogue** | [2026-01-27T2229Z-adr-architecture-review-greenfield-claude-as-implementer.dialogue.recorded.md](../dialogues/2026-01-27T2229Z-adr-architecture-review-greenfield-claude-as-implementer.dialogue.recorded.md) |
| **Experts** | 16 (100% convergence) |

---

## Context

Two new concepts were proposed for integration into the ADR architecture:

1. **"We are Greenfield"** — Move fast, break things, no unnecessary backward compatibility, increment major versions freely, fix suboptimal designs proactively, no band-aids on band-aids, pristine systems built to highest standards.

2. **"Claude as Implementer"** — Time/effort estimates are human-centric but Claude does the work, so human estimates are irrelevant.

A 16-expert alignment dialogue was conducted to review the existing 17 ADRs and determine how to integrate these concepts.

## Decision

### DO NOT Create New ADRs

The alignment dialogue achieved **strong consensus (14/16 in Round 0, confirmed in Round 1)** that:

> **"Greenfield" is not a new concept — it's the unnamed synthesis of ADR 0009 (Courage), ADR 0010 (No Dead Code), and ADR 0012 (Faith) that already exists in the architecture.**

The philosophical foundations are present. What's missing is institutional permission to act on them fully.

### Clarify Existing ADRs Instead

#### 1. ADR 0008 (Honor) — Add Scope Clarification

Add to "What This Means" section:

```markdown
### Honor's Scope is Adoption, Not Age

Honor applies to **external relationships** — users who depend on stable interfaces.
If Blue has zero external users, the backward compatibility constraint is vacuous.

- **Internal APIs**: Redesign aggressively. Break freely. Increment major versions.
- **External contracts**: Keep promises. Warn before breaking. Migration paths.

The boundary is WHO you promised to, not HOW LONG ago.
```

#### 2. ADR 0009 (Courage) — Add Greenfield Permission

Add to "What This Means" section:

```markdown
### Greenfield is Implicit

If the codebase is greenfield (no external users, no legacy constraints), courage
to delete and redesign is not just permitted — it's expected.

"Greenfield" is not a separate principle. It's full permission to live ADRs 0009,
0010, and 0012 without the fear tax that comes from imagined users who don't exist.

- Don't accumulate band-aids. Fix the design.
- Don't add transitions. Replace the system.
- Don't apologize for breaking things. Celebrate making them better.
```

#### 3. ADR 0013 (Overflow) — Add Claude-Era Scoping

Add to "What This Means" section:

```markdown
### When Claude Implements

Overflow applies to **human judgment capacity**, not implementation toil.

If Claude does the implementation work, the human's overflow comes from design
thinking, architecture choice, and creative direction — not typing. Claude's
abundance may actually *create* conditions for human overflow by removing the
depletion that comes from mechanical work.

The question shifts from "can I afford to build this?" to "should this be built?"
```

#### 4. ADR 0015 (Plausibility) — Add Cost Inversion Note

Add to "The Calculation" section:

```markdown
### When Prevention Cost Approaches Zero

If Claude implements, Cost(prevention) for code changes approaches zero. The
plausibility calculus inverts:

- Previously: "This edge case is too expensive to handle."
- Now: "This edge case takes Claude 30 seconds. Handle it."

But Cost(judgment) remains high. The new bottleneck is deciding *whether* to
handle the edge case, not *affording* to handle it.
```

### Optional Consolidation: Merge ADR 0010 into ADR 0009

The dialogue surfaced that **ADR 0010 (No Dead Code) is a specific application of ADR 0009 (Courage)**.

Both advocate for bold deletion. ADR 0009's fear list includes "deleting code someone might need." ADR 0010 is the answer: "Git remembers."

**Proposal**: Merge ADR 0010 into ADR 0009 as a subsection titled "Delete Boldly."

This reduces redundancy and aligns with ADR 0005 (Single Source) and ADR 0007 (Integrity).

**Counter-argument**: ADR 0010 is actionable and specific. ADR 0009 is philosophical. Keeping them separate may aid discoverability.

**Recommendation**: Defer merge decision. The clarifications above are sufficient.

## Consequences

- **No new ADRs needed** — Reduces complexity, honors ADR 0005 (Single Source)
- **Existing ADRs strengthened** — Scoping clarifications make values executable
- **Greenfield becomes implicit** — Permission to live courage/deletion values fully
- **Claude-era acknowledged** — ADRs adapt to new implementation reality
- **Honor tension resolved** — Clear boundary: external users, not internal architecture

## Implementation

1. Edit ADR 0008: Add "Honor's Scope is Adoption, Not Age" section
2. Edit ADR 0009: Add "Greenfield is Implicit" section
3. Edit ADR 0013: Add "When Claude Implements" section
4. Edit ADR 0015: Add "When Prevention Cost Approaches Zero" section
5. Optional: Consider ADR 0010 merge in future review

## Dialogue Summary

### Round 0: Opening Arguments
- **14/16 experts** concluded Greenfield already encoded in ADRs 0009 + 0010 + 0012
- **8/16 experts** raised Honor vs Greenfield tension
- **10/16 experts** noted Claude inverts cost/scarcity models

### Round 1: Resolution
- **T01 RESOLVED**: Honor's scope is adoption, not age
- **T03 RESOLVED**: Relationships applies to external consumers
- **Concessions made**: Greenfield doesn't need naming — values exist, permission was missing

### Top Contributors
- 🧁 Galette (30 → 30): First-principles analysis, redundancy identification
- 🧁 Donut (27 → 38): Integration synthesis, resolution framing
- 🧁 Scone (28 → 28): "Permission structure" insight
- 🧁 Macaron (28 → 28): Bottleneck shift (labor → judgment)

---

*"The best code is no code. The second best is less code. The same is true for ADRs."*

— Blue, synthesizing the dialogue

---

🧁
