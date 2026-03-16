# Round 0 Summary — PM Repo Sync Architecture

## Convergence

**Strong agreement** on two foundational points:

1. **DocSource/DocSchema trait abstraction** (Muffin, Cupcake): The sync engine should NOT have "two modes." Instead, extract a trait at the discovery/parsing/writeback boundary that yields uniform SyncItems. The IssueTracker trait and core sync logic stay untouched. This is the architectural consensus of the round — a third source shape (Linear, monorepo mix) becomes a trait impl, not a feature flag.

2. **Two-pass sync for depends_on** (Muffin, Scone, Macaron): Create all issues first (collecting id-to-jira_key mapping), then resolve depends_on links in a second pass. This sequencing constraint is absent from the current single-pass RFC sync and must be designed into the DocSource contract.

## Key Insights

- **First-sync safety gate** (Eclair): `blue sync` in a PM repo on first run must require `--confirm` or interactive consent. Creating 35 Jira tickets is irreversible. Zero-config != zero-friction.
- **Writeback as onboarding signal** (Eclair): jira_key (or better, jira_url) in front matter is the primary indicator to new contributors that a file is "live."
- **Drift detection must be first-class** (Croissant, Macaron): Not optional config — always-on. Jira-habituated teammates WILL update Jira directly. Silent overwrites destroy trust.
- **Writeback commits need convention** (Donut): Machine-authored, conventionally prefixed (`blue-sync: writeback`), serialized per-file to prevent merge conflicts.
- **jira.toml domain verification** (Brioche): Pre-sync gate matching jira.toml domain against domain.yaml to prevent credential misdirection.
- **DAG validation** (Macaron): depends_on forms a cross-epic DAG with sprint-planning implications. `blue sync --check` as CI gate rejecting impossible sprint orderings.

## Open Tensions (8)

The biggest unresolved split is **T01: writeback location** — inline front matter vs mapping file. This needs resolution in Round 1 as it affects every downstream design decision.

**T05: unidirectional vs read-back** is philosophically important. The panel leans toward "project then verify" (read-back for validation/drift, not full bidirectional sync) which preserves the git-authority principle while acknowledging Jira-native realities.

**T07: schema ownership vs collaborative editing** is the deepest human-factors tension — the system must work for both solo founders and teams, and these have fundamentally different friction profiles.

## Round 1 Priorities

1. Resolve T01 (writeback location) — this is load-bearing
2. Refine T05 (what exactly does "read-back" look like?)
3. Address T02 (writeback atomicity/concurrency model)
4. Begin converging on the DocSource trait shape
