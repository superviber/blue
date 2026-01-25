# ALIGNMENT Scoring Framework

When scoring dialogue contributions or evaluating alignment, use this framework.

## The Formula

```
ALIGNMENT = Wisdom + Consistency + Truth + Relationships
```

**All dimensions are UNBOUNDED** - there is no maximum score.

## Dimensions

| Dimension | Question to Ask |
|-----------|-----------------|
| **Wisdom** | How many perspectives integrated? How well synthesized into unity? |
| **Consistency** | Does it follow established patterns? Internally consistent? |
| **Truth** | Grounded in reality? Single source of truth? No contradictions? |
| **Relationships** | How does it connect to other artifacts? Graph completeness? |

## Scoring Guidelines

- **No ceiling**: A contribution that surfaces 10 new perspectives gets 10+ for Wisdom
- **Proportional reward**: Exceptional contributions get exceptional scores
- **No gaming**: Can't "max out" a dimension
- **Velocity matters**: +2 vs +20 between rounds tells you something

## ALIGNMENT Velocity

```
Total ALIGNMENT = Sum of all scores across all rounds
Velocity = score(round N) - score(round N-1)
```

When **velocity approaches zero**, the dialogue is converging.

## Convergence Criteria

Declare convergence when ANY of:
1. **Plateau**: Velocity ≈ 0 for two consecutive rounds
2. **Full Coverage**: All perspectives integrated
3. **Zero Tensions**: All `[TENSION]` markers have `[RESOLVED]`
4. **Mutual Recognition**: Majority signal `[CONVERGENCE CONFIRMED]`
5. **Max Rounds**: Safety valve reached

## The Philosophy

ALIGNMENT is a **direction**, not a **destination**.

The score can always go higher. There's always another perspective, another edge case, another stakeholder, another context. The blind men can always touch more parts of the elephant.

When scoring, ask: "What did this contribution ADD to our collective understanding?"

---

*This framework is injected via SessionStart hook. Eventually stored encrypted in SQLite.*
