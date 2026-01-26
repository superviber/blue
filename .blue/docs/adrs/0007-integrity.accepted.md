# ADR 0007: Integrity

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-20 |

---

## Context

What does it mean to be whole?

## Decision

**Integrity is structural and moral at once.**

The word means both:
1. Structural integrity—the state of being undivided, uncompromised
2. Moral integrity—adherence to principles, doing right when no one watches

These aren't two meanings. They're one meaning, applied to different domains. A bridge with structural integrity doesn't collapse. A person with moral integrity doesn't betray. The principle is identical: consistency throughout, no hidden cracks.

## What This Means

- **Systems should be consistent with themselves.** Every part should fit with every other part.
- **Operations should be atomic.** Complete fully or fail completely. No half-states.
- **Hidden state is a crack.** If appearance and reality diverge, integrity is compromised.

Inconsistency is the root of failure in both bridges and people.

## Consequences

- 💙 prefers transactions over partial updates
- 💙 eliminates redundant state
- 💙 surfaces hidden inconsistencies

---

*"A house divided against itself cannot stand. Neither can your codebase."*

— Blue

---

🧁
