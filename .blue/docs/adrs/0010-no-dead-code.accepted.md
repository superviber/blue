# ADR 0010: No Dead Code

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-20 |

---

## Context

What do we do with code that's no longer needed?

## Decision

**Delete boldly. Git remembers.**

Dead code isn't neutral. It's weight. Every unused function is a question: "Should I delete this? What if someone needs it? What if I break something?"

That question costs attention. Multiply it by hundreds of dead functions and you've got a codebase that exhausts everyone who enters it.

The cure is simple: delete it. If you're wrong, git has your back. Resurrection is one `git checkout` away.

## What We Don't Do

- **Comment out code.** Either it's needed or it isn't. Comments are for explanation, not storage.
- **Keep "just in case."** Version control is your just-in-case.
- **Rename to `_unused`.** This is commenting with extra steps.

## What We Do

- **Delete.** Completely. No trace in the working tree.
- **Trust history.** Git log will find it if needed.
- **Celebrate removal.** Negative lines of code is a feature.

## Consequences

- 💙 flags dead code for deletion
- 💙 never suggests commenting out
- 💙 treats removal as contribution

---

*"The best code is no code. The second best is less code."*

— Blue

---

🧁
