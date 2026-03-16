# Muffin — Round 2 (Data Modeling Specialist)

## T03: depends_on Link Type Semantics — RESOLVED

Scone proposes default `Blocks` with per-edge `link_type` override in the `depends_on` list. Macaron wants separate `depends_on` and `relates_to` fields. As the schema expert, I am resolving this in favor of **Scone's model with one concession to Macaron's concern**.

Macaron's core argument is sound: `depends_on` cycles are hard errors while `relates_to` cycles are fine, so the two relationship types have different validation semantics. But separate top-level fields is the wrong mechanism -- it forces schema consumers to merge two lists to build the full link graph, and it scales poorly when a third link type is needed (e.g., `duplicates`, `is-cloned-by`).

The resolution: **`depends_on` remains a single field. Entries default to `Blocks`. Per-edge `link_type` override allows any Jira-supported link type.** The DAG validator treats `Blocks` edges as the blocking subgraph for cycle detection; all other link types are excluded from cycle checks. This gives Macaron the semantic fidelity needed for validation without splitting the schema surface.

```yaml
# Common case: flat list, all Blocks
depends_on:
  - VOT-002
  - VOT-005

# Mixed case: explicit override
depends_on:
  - VOT-002                    # defaults to Blocks
  - id: LOC-003
    link_type: relates-to      # non-blocking, excluded from cycle check
```

The sync engine's `resolve_links()` maps each entry to a Jira `IssueLinkType`. Unrecognized `link_type` values fail at sync time with an actionable error listing valid types from the target project. The `DriftDetector` validates link-type consistency post-sync.

This is not architecturally load-bearing -- it is a schema detail with a clean resolution. Both Scone and Macaron get what they need.

[RESOLVED T03: Single `depends_on` field with default Blocks + per-edge link_type override. DAG cycle detection scoped to Blocks edges only.]

---

## T07: CLI/TUI Gap — RESOLVED (scoped out of MVP)

Croissant correctly identifies the mode-switch tax: updating status in YAML is a recall task, updating it in Jira is a recognition task. The question is whether MVP must include `blue status` / `blue board` commands.

My position: **No. MVP ships without mutation CLI commands. This is an explicit, acknowledged design choice, not an oversight.**

Rationale:
1. The PM repo targets solo founders and git-native teams -- this is the stated audience of the Let'em Cook system. Acknowledging this is not a "hiring filter masquerading as a tooling choice"; it is a product scope decision.
2. The DriftDetector (T05, resolved) already provides the safety net: if someone updates status in Jira, drift reporting surfaces it, and the PM author updates git with full context.
3. `blue status` and `blue board` are additive features that do not change the sync architecture. They can ship in a follow-up RFC without modifying any resolved tension.
4. Building CLI mutation commands before the sync engine stabilizes creates two moving surfaces instead of one.

The RFC should include a "Future Work" section listing `blue status`, `blue board`, and `blue story create` as planned ergonomic improvements for post-MVP.

[RESOLVED T07: MVP targets git-native teams. CLI mutation commands (blue status, blue board) are documented as future work. DriftDetector provides the bridge for Jira-side status changes.]

---

## T09: Lock-Ref Access — Acknowledged as Future Work

Donut correctly identified that `refs/sync/lock` requires push access to the PM repo remote. For the solo-founder MVP, this is not a constraint -- the single user has push access. For multi-team orgs with restricted PM repo access, a delegation model (sync request queue, or requiring sync to run from a PM repo worktree) is needed.

This is not resolvable in this dialogue because it depends on org-level access control patterns that are out of scope for the current RFC. It should be tracked as a known limitation with a forward pointer.

[ACKNOWLEDGED T09: Future work. Solo-founder MVP assumes push access. Multi-team delegation model to be designed when org-level access control is specified.]

---

## Convergence Assessment

All eight tensions are now addressed:
- **T01, T02, T04, T05, T06, T08**: Resolved in Round 1
- **T03**: Resolved above (single field, default Blocks, per-edge override)
- **T07**: Resolved above (explicit git-native scope, CLI commands as future work)
- **T09**: Acknowledged as future work (not blocking)

The architectural decisions are stable. The DocSource trait shape, two-pass sync, inline writeback, DriftDetector, and first-sync safety gate are all converged.

[MOVE:CONVERGE]
