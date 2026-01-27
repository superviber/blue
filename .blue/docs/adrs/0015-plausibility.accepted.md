# ADR 0015: Plausibility

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-25 |

---

## Context

Most engineering decisions are made in fear of the implausible. We add error handling for errors that won't happen. We build abstractions for flexibility we'll never need. We guard against threats that don't exist.

## Decision

**Act on the plausible. Ignore the implausible.**

1. **Probability matters.** A 0.001% risk does not deserve the same treatment as a 10% risk.

2. **Rare failures are acceptable.** A system that fails once per million operations is not broken.

3. **Don't guard against fantasy.** If you can't articulate a realistic scenario, remove the guard.

4. **Recover over prevent.** For implausible failures, recovery is cheaper than prevention.

5. **Trust reasonable assumptions.** "What if the user passes negative infinity?" is not serious if the user is you.

## Consequences

- Less defensive code
- Simpler error handling
- Faster development
- Occasional rare failures that we fix when they occur

## The Calculation

```
Expected Cost = P(failure) × Cost(failure) + P(success) × Cost(prevention)
```

If `P(failure)` is near zero, almost any `Cost(failure)` is acceptable. We waste more engineering time preventing implausible failures than we would spend recovering from them.
