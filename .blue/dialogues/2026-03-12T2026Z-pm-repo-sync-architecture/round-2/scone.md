# Round 2 — Scone (API Integration Specialist)

## T03: depends_on Link Type Semantics — RESOLUTION PROPOSAL

Macaron and I agree on the problem: flat `depends_on` lists lose semantic fidelity when projected to Jira's typed link system. We disagree on the schema shape. Macaron wants separate top-level fields (`depends_on`, `relates_to`). I proposed a single `depends_on` field with per-edge `link_type` overrides.

I am conceding the core point to Macaron and proposing a synthesis.

**Why separate fields win**: Macaron's argument that `depends_on` cycles are hard errors while `relates_to` cycles are fine is the deciding factor. If link types are buried in per-edge metadata within a single field, the DAG validator must parse into the edge objects to determine which edges participate in cycle detection. With separate fields, the validator runs cycle detection on `depends_on` edges only and ignores `relates_to` entirely. The schema encodes the constraint, not the validator logic.

**The synthesis**: Two top-level fields with Jira link type mapping declared in `jira.toml`:

```yaml
# Story front matter
depends_on:
  - VOT-002
  - VOT-005
relates_to:
  - LOC-003
```

```toml
# jira.toml
[link_types]
depends_on = "Blocks"      # Jira link type for depends_on edges
relates_to = "Relates"     # Jira link type for relates_to edges
```

This gives us:
1. **Clean schema** — two fields, both flat lists, no inline metadata objects (my R1 `{id, link_type}` syntax was overengineered)
2. **Semantic fidelity** — blocking vs non-blocking relationships are structurally distinct
3. **Configurable mapping** — orgs with custom Jira link types remap in one place
4. **Simple validation** — cycle detection on `depends_on` only; `relates_to` is unconstrained
5. **Extensible** — future link types (e.g., `duplicates`, `clones`) add a field + a `jira.toml` mapping without schema changes to the core

The `resolve_links` method on `DocSource` emits `LinkRequest { from, to, link_type }` by reading both fields and looking up the Jira link type from config.

**Dropping** my R1 per-edge override syntax. The common cases (blocks, relates) are covered by separate fields. Exotic link types can be added as new fields later — YAGNI until proven otherwise.

[RESOLVED T03: Two top-level fields (depends_on, relates_to) with Jira link type mapping in jira.toml]

---

## T07: CLI/TUI Gap — Position

Croissant frames this correctly: git-as-interface for status mutations is a recall task where Jira is a recognition task. The question is whether we solve this now or acknowledge it as a design boundary.

My position: **solve it with thin CLI commands, defer TUI**. Specifically:

- `blue status <id> <status>` — rewrites front matter `status:` field, commits
- `blue assign <id> <assignee>` — rewrites `assignee:` field, commits
- `blue story create --hat dev --title "..."` — scaffolds a new story file from template

These are 10-line wrappers around front-matter parse-mutate-write-commit. They do not require a TUI, a board view, or bidirectional sync. They reduce the recall burden for the three most common mutations without introducing new architectural complexity.

`blue board` (TUI) is a separate concern — useful but not load-bearing for the sync architecture RFC. It should be a follow-on recipe, not a blocker.

Croissant's option (b) — explicitly acknowledge the git-native hiring filter — is also valid for the MVP. But the thin CLI commands are cheap enough to include in the initial scope and they eliminate the strongest objection.

[REFINEMENT T07: Thin CLI mutation commands (status, assign, story create) in initial scope. TUI board deferred.]

---

## T09: Lock-Ref Push Access — Position

Donut raised a real constraint. For the solo-founder MVP, this is a non-issue — the founder has push access everywhere. For restricted-access orgs, the lock-ref mechanism breaks.

I agree with the round-1 summary: **acknowledge as future work, do not block convergence**. The delegation model (sync via PM-repo meeple queue) is the right direction but adds significant complexity that is not justified until multi-team use cases materialize.

For now: document the assumption ("sync actors require push access to PM repo remote") as a known constraint in the RFC. Revisit when org-level access control becomes a real requirement.

[ACKNOWLEDGED T09: Document push-access assumption. Delegation model is future work.]

---

## Convergence Assessment

- **T01-T06, T08**: Resolved in R1. No objections.
- **T03**: Resolved above — two separate fields, mapping in jira.toml.
- **T07**: Refined — thin CLI commands in scope, TUI deferred. Not architecturally load-bearing; should not block convergence.
- **T09**: Acknowledged as future work with documented assumption.

The architectural decisions are stable: DocSource trait, two-pass sync, inline writeback, DriftDetector, first-sync safety gate, domain pinning. The remaining tensions are scope/priority decisions, not architectural disagreements.

[MOVE:CONVERGE]
