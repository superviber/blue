# Croissant — Round 2 (Organizational Psychologist, Convergence)

## T07: CLI/TUI Gap — RESOLVED (post-MVP, narrow read-back as bridge)

In Round 1, I framed T07 as a mode-switch tax — recognition vs. recall — and argued the system needs either CLI mutation commands or an explicit acknowledgment that it targets git-native teams. After reading Cupcake and Muffin's Round 2 responses, I am ready to close this.

**I accept post-MVP scoping for `blue status` / `blue board`.** Here is why my position has moved:

1. **The narrow read-back contract already addresses the sharpest pain.** My own R1 refinement — status and sprint as Jira-authoritative fields with scoped write-back — was accepted into the T05 resolution. This means the highest-friction recall tasks (remembering status enum values, tracking sprint assignments) are handled by the DriftDetector's read-back authority. The mode-switch tax is reduced at the point where it hurts most.

2. **MVP audience is correctly scoped.** Cupcake is right that the Let'em Cook system already assumes git literacy. Muffin is right that building CLI mutation commands before the sync engine stabilizes creates two moving surfaces. The solo-founder MVP user is, by definition, the person who chose git-as-interface. Calling this a "hiring filter" in R1 was accurate for the team-adoption case but premature for MVP.

3. **The architectural decisions are not affected.** `blue status` and `blue board` are rendering and mutation concerns over data that already exists in the repo. They do not change the DocSource trait, the two-pass sync, the writeback model, or the DriftDetector. They can ship as additive features without revisiting any resolved tension.

**What I want in the RFC's Future Work section:**

- `blue status <story-id> <new-status>` — CLI shorthand that rewrites front matter and commits
- `blue board` — read-only board view as a first step, mutation as a follow-up
- Recognition-over-recall principle documented as a design constraint for these commands (valid enum values shown, not required from memory)

The narrow read-back bridge (status + sprint from Jira) plus explicit git-native audience scoping satisfies my concern for MVP. The mode-switch tax remains real for team adoption, but it is no longer an architectural tension — it is a DX roadmap item.

[RESOLVED T07: Post-MVP. Narrow read-back (status + sprint) bridges the gap for MVP. CLI mutation commands documented as future work with recognition-over-recall as a design constraint.]

---

## T03: depends_on Link Type Semantics — RESOLVED

Cupcake and Muffin converged on the same design: single `depends_on` field, default `Blocks`, per-edge override via map syntax. Muffin's addition — DAG cycle detection scoped to `Blocks` edges only, with non-blocking link types excluded — is the right call. This cleanly separates ordering semantics from informational links without schema sprawl.

No objection. Macaron's semantic concern is addressed by the validation scoping.

[RESOLVED T03: Agree with Cupcake/Muffin. Single field, default Blocks, per-edge override, cycle detection scoped to blocking edges.]

---

## T09: Lock-Ref Access — Acknowledged

Solo-founder MVP assumes push access. This is a deployment constraint, not an architectural one. Track as a known limitation.

[ACKNOWLEDGED T09: Future work.]

---

## Convergence Assessment

All tensions are now resolved or explicitly deferred:

| ID | Status |
|----|--------|
| T01 | Resolved R1 |
| T02 | Resolved R1 |
| T03 | Resolved R2 (single field, per-edge override) |
| T04 | Resolved R1 |
| T05 | Resolved R1 (narrow read-back: status + sprint) |
| T06 | Resolved R1 |
| T07 | Resolved R2 (post-MVP, read-back bridge, future work documented) |
| T08 | Resolved R1 |
| T09 | Acknowledged (future work, not blocking) |

The architecture is stable. The DocSource trait, two-pass sync, inline writeback, DriftDetector with scoped read-back authority, first-sync safety gate, and domain pinning are all converged. No remaining disagreements that would change implementation direction.

[MOVE:CONVERGE]
