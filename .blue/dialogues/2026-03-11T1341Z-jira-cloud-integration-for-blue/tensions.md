# Tensions Tracker — FINAL

All structural tensions resolved. Refinement tensions deferred to RFC.

| ID | Tension | Status | Resolution |
|----|---------|--------|-----------|
| T01 | Sync direction | **Resolved R1** | Three-tier field ownership: structural=git-wins, operational=Jira-owned, descriptive=drift-warned. Configurable per-domain drift policy (overwrite/warn/block, default warn). |
| T02 | jira-cli dependency | **Resolved R1** | IssueTracker trait with out-of-process adapter contract. jira-cli optional, never bundled. PATH-discoverable provider binary. |
| T03 | Token storage | **Resolved R1** | Domain-keyed hierarchy: env var (CI) → OS keychain (interactive) → TOML fallback. Bot accounts recommended, not required. |
| T04 | Multi-repo fan-out | **Resolved R1** | RFC-to-Task binding in RFC front matter (repo-local). PM repo only declares Epics. Per-entity files. |
| T05 | Epic cardinality | **Resolved R1** | 1:1 Feature Release to Epic default. Explicit override. Blue-native ID indirection. |
| T06 | Bootstrapping | **Resolved R1** | Two-phase `blue jira import` producing reviewable PRs. One-time bootstrap; git authority post-import. |
| T07 | Convention enforcement | **Resolved R1** | Progressive three-tier: always-enforced → warn-then-enforce → document-only. Repo-side only. |
| T08 | Drift detection frequency | **Deferred to RFC** | Event-driven on `blue sync`; `stale_after` config knob. |
| T09 | Error ergonomics | **Deferred to RFC** | Local ops succeed with warning; sync ops fail fast when provider absent. |
| T10 | Credential mechanism | **Deferred to RFC** | Ship all three tiers; TOML with chmod 600; keychain as enhancement. |
| T11 | Bot account provisioning | **Deferred to RFC** | Recommended not required; personal tokens accepted. |
