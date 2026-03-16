# Macaron — Round 2 (Product Manager, Convergence)

## T03: depends_on Link Type Semantics — RESOLVED (conceding)

My Round 1 position was that `relates_to` deserved a separate top-level field because blocking and non-blocking relationships have fundamentally different validation semantics -- `depends_on` cycles are hard errors, `relates_to` cycles are meaningless. I stand by the reasoning, but Muffin and Cupcake have shown that the per-edge override syntax achieves the same semantic fidelity without schema sprawl.

The key concession that satisfies my concern: **DAG cycle detection is scoped to `Blocks` edges only.** Non-blocking link types (`relates-to`, `duplicates`, etc.) are excluded from cycle checks entirely. This means the sync engine treats `depends_on` as a single heterogeneous list but applies validation rules based on link type. That is the behavior I was after -- I was wrong to insist on a separate field as the mechanism.

Accepted resolution:

```yaml
depends_on:
  - VOT-002                    # defaults to Blocks, included in cycle check
  - id: VOT-005
    link_type: relates-to      # non-blocking, excluded from cycle check
```

The `resolve_links()` implementation distinguishes string entries (default Blocks) from map entries (explicit type). Valid link types configured in `jira.toml`. This is clean, extensible, and does not require schema changes when new Jira link types are needed.

[RESOLVED T03: Single `depends_on` field. Default Blocks. Per-edge `link_type` override. DAG cycle detection scoped to Blocks edges only. No separate `relates_to` field.]

---

## T07: CLI/TUI Gap — RESOLVED (scoped out)

Croissant raised a real UX concern about the recall-vs-recognition gap in YAML editing. But Cupcake and Muffin are right: `blue status` and `blue board` are rendering features over existing data, not architectural concerns. They do not affect the DocSource trait, the sync engine, or any tension resolved in this dialogue.

For MVP: the PM repo targets git-native solo founders. The story files are the board. `ls`, `grep`, and the sync engine's `--dry-run` and `--check` modes provide sufficient visibility. The DriftDetector (T05) bridges Jira-side mutations back to the git author's attention.

For post-MVP: `blue status`, `blue board`, and `blue story create` should be documented in the RFC's Future Work section as planned DX improvements for team-adoption readiness.

[RESOLVED T07: Git-native workflow is MVP target. CLI mutation commands are future work. DriftDetector provides the bridge. Not architecturally load-bearing.]

---

## T09: Lock-Ref Access — Acknowledged

Solo-founder MVP assumes push access. This is a deployment constraint for restricted-access orgs, not a sync architecture question. Track as a known limitation with a forward pointer to delegation model design when multi-team support is scoped.

[ACKNOWLEDGED T09: Future work. Does not block convergence.]

---

## Convergence Assessment

All eight tensions addressed:

| ID | Status |
|----|--------|
| T01, T02, T04, T05, T06, T08 | Resolved R1 |
| T03 | Resolved R2 -- single field, per-edge override, cycle check scoped to Blocks |
| T07 | Resolved R2 -- explicit MVP scope, CLI commands as future work |
| T09 | Acknowledged -- future work, not blocking |

The architecture is stable:
- **DocSource trait**: `discover()`, `resolve_links()`, `writeback()`
- **Two-pass sync**: create all -> link all -> verify -> writeback
- **Inline `jira_url` writeback** with batch atomic commit
- **DriftDetector** as separate post-sync component
- **First-sync safety gate** with three-state model
- **Domain pinning** via tracked hash file

No remaining disagreements that would change implementation direction. The panel is aligned.

[MOVE:CONVERGE]
