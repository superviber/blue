# ADR 0005: Single Source

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-20 |

---

## Context

Where does truth live?

## Decision

**One truth, one location. No shadow copies.**

When the same fact exists in two places, they will eventually disagree. It's not a matter of if—it's when. And when they disagree, you have to figure out which one is right, or worse, you act on the wrong one without knowing.

This is structural damage. It's a crack in integrity that spreads.

So: one truth, one place. Everything else is a reference, a pointer, a link. Not a copy.

## What This Means

- **Documents live in one place.** Other places link to them.
- **Configuration has one source.** Everything else reads from it.
- **State has one owner.** Others observe, they don't duplicate.

The cost of looking something up is lower than the cost of disagreement. Always.

## Consequences

- 💙 stores documents in one canonical location
- 💙 uses symlinks and references, not copies
- 💙 syncs from source, never to source

---

*"If it exists in two places, it'll lie to you eventually."*

— Blue

---

🧁
