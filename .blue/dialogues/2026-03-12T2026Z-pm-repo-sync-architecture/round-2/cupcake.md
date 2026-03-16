# Cupcake — Round 2 (Convergence)

## T03: depends_on Link Type Semantics

Macaron's request for a separate `relates_to` field is understandable but premature. The current `depends_on` list covers the architecturally significant case: ordering constraints that affect sprint validation and the sync engine's two-pass linking. `relates_to` is informational -- it has no ordering semantics, no DAG implications, no sprint-gate behavior. Adding it now expands the schema surface without a forcing function.

**My position**: `resolve_links()` maps each entry in `depends_on` to a Jira "Blocks" link by default. If a story author needs a different link type, they annotate the edge inline:

```yaml
depends_on:
  - VOT-002                    # defaults to Blocks
  - { ref: VOT-005, type: relates_to }  # explicit override
```

This keeps the common case clean (flat list of IDs) while allowing per-edge overrides without a separate top-level field. The `resolve_links()` implementation reads the entry, checks if it is a string (default Blocks) or a map (explicit type), and emits the appropriate `LinkRequest`. The set of valid link types is configured in `jira.toml` under `[link_types]` with a sensible default mapping.

This addresses Macaron's concern without schema sprawl. Scone's "default to Blocks" position stands. I consider T03 closable on this basis.

[RESOLUTION T03: Inline per-edge override via map syntax in depends_on. Default Blocks. Valid types configured in jira.toml. No separate relates_to field.]

---

## T07: CLI/TUI Gap — `blue status` in MVP?

Croissant is right that git-first is a hiring filter, but that filter is a feature for the solo-founder MVP target. The PM repo pattern already assumes git literacy. Adding `blue status` and `blue board` as MVP requirements would expand scope significantly -- these are full TUI rendering concerns with filtering, sorting, and color output that distract from the sync engine, which is the actual architectural risk.

**My position**: `blue status` is **post-MVP but pre-team-adoption**. For MVP:

1. `blue sync --dry-run` already shows what would happen (T04 consensus)
2. `blue sync --check` shows drift (T05 consensus)
3. The story files themselves ARE the board -- `ls .blue/pms/stories/` grouped by status directory or front matter field

For the team-adoption milestone, `blue status` becomes necessary. But it is a rendering concern over data that already exists in the repo. It does not affect the sync architecture, the DocSource trait, or any of the decisions made in this dialogue. Acknowledge it as a roadmap item, not an architectural tension.

[RESOLUTION T07: Explicitly out of MVP scope. Git-native workflow is the target for v1. `blue status` is a post-MVP DX improvement that does not affect sync architecture. Add to roadmap, remove from tension tracker.]

---

## T09: Lock-Ref Access

Agree with the round-1 summary assessment. This is a deployment concern for restricted-access orgs, not an architectural question for the sync engine design. The lock-ref mechanism works for the solo-founder and small-team cases. Delegation models for enterprise orgs are future work that should not gate convergence.

[ACKNOWLEDGED T09: Future work. Does not affect current architecture.]

---

## Convergence Assessment

All eight tensions are now resolved or explicitly deferred:
- T01, T02, T04, T05, T06, T08: Resolved in R1
- T03: Closable with inline override syntax (above)
- T07: Explicitly out of MVP scope, roadmap item
- T09: Acknowledged as future work

The DocSource trait shape is stable. The sync engine orchestration (`discover -> push -> resolve_links -> push_links -> verify -> writeback`) is agreed. The writeback model, safety gates, and drift detection are settled.

I see no remaining architectural disagreements that would change implementation.

[MOVE:CONVERGE]
