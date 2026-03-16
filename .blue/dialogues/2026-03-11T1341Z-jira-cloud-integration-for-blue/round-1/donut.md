# Round 1 -- Donut (Workflow Analyst)

## Engaging with Round 0 Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]
Round 0 reached strong consensus that git is sole authority and Jira is a projection. The remaining friction is operational: PMs *will* edit Jira directly. I refine my position after reading Strudel, Muffin, and Croissant's arguments.

Blue should implement a **drift-detection layer**, not a drift-prevention layer. On every `blue sync`, Blue compares the projected Jira state against actual Jira state and emits a structured diff report. The report classifies drift into two categories:

1. **Cosmetic drift** (description edits, comment additions, assignee changes) -- logged but not overwritten. These fields are Jira-side-owned by convention because they serve stakeholder workflows, not engineering authority.
2. **Structural drift** (status transitions, Epic membership changes, issue type mutations) -- flagged as warnings and overwritten on next sync, with the overwritten values logged to a `drift-log.yaml` in the PM repo for auditability.

This avoids Strudel's concern about making PMs second-class citizens (cosmetic fields remain theirs) while preserving Eclair's and my insistence that structural state flows one-way from git. The policy should be configurable per-field in the PM repo manifest so teams can declare which fields are Jira-side-editable.

[PERSPECTIVE P03: Drift classification must be field-level, not system-level]
The "who wins" question in T01 is actually a per-field question, not a per-system question. Status, Epic membership, and RFC linkage are git-owned. Description, assignee, sprint, priority, and labels are Jira-owned. Blue should ship a default field-ownership map in the PM repo schema and let teams override it. This eliminates the false binary of "git always wins" vs "bidirectional sync" -- it is a declared ownership matrix.

### T02: jira-cli dependency model

[CONCESSION on T02]
I concede to Cupcake and Beignet's position. In Round 0 I did not address this directly, but as a Workflow Analyst I see the coupling risk clearly now. Blue should define an `IssueTracker` trait (as Muffin proposed) with `JiraCloudTracker` as the default implementation that shells out to `jira-cli` if available and degrades to direct REST calls (or no-op) if absent. The key workflow constraint: `blue sync` must never hard-fail due to a missing tracker binary. It should queue sync intent locally and warn the user. This preserves offline-first behavior and avoids making jira-cli a gating dependency.

### T04: Multi-repo fan-out and atomic consistency

[REFINEMENT on T04]
This is the tension I raised in Round 0, and Macaron's proposal (RFC-to-Task binding lives in RFC front matter, not the central PM repo) substantially resolves it. If each repo writes its own Jira binding into its local RFC front matter, the PM repo is no longer a write-contention bottleneck for task-level mappings.

However, I refine the concern: **Epic-level operations still require PM repo writes**, and these can conflict when two repos simultaneously try to add RFCs to the same Epic manifest. Cupcake's append-only per-entity file proposal (one YAML per Epic) is the right structural mitigation. Combined with Macaron's repo-local task bindings, the architecture becomes:

- **Task bindings**: repo-local (in RFC front matter) -- no cross-repo write contention
- **Epic declarations**: PM repo, one file per Epic -- append-only, low-contention
- **Reconciliation**: `blue sync` reads task bindings from member repos, reads Epic declarations from PM repo, projects combined state to Jira

This separates the high-frequency operation (task binding) from the low-frequency operation (Epic creation) and eliminates the atomic consistency problem for the common case.

### T05: Epic/Feature Release cardinality across repos

[REFINEMENT on T05]
Cannoli correctly identified the cardinality trap. I refine: Blue should enforce a strict 1:1 mapping between Feature Release and Jira Epic as the default, but expose an explicit `epic_key` override in the PM repo manifest for teams whose Epic semantics differ. The override must be declared, not inferred -- if a team wants one Epic to span multiple Feature Releases, they must say so in configuration, and `blue lint` should warn that this deviates from the standard model.

For multi-repo Epics: an Epic declaration file in the PM repo lists which repos contribute RFCs. Each repo's RFC front matter references the Epic by its Blue-native Feature Release ID (not the Jira key). The PM repo's sync process resolves the Feature Release ID to the Jira Epic key. This indirection means Jira project moves (which change Epic keys) only require updating one mapping file, not every RFC.

### T06: Bootstrapping from existing Jira state

[PERSPECTIVE P04: Import must produce reviewable artifacts, not silent state injection]
Churro's `blue jira import` proposal is essential for adoption. I add a workflow constraint: the import command must produce a PR to the PM repo (and optionally RFC stub PRs to member repos), not directly mutate state. This gives teams a reviewable on-ramp -- they can inspect the generated Epic manifests and RFC stubs, adjust mappings, and merge when satisfied. Silent state injection during import would violate Blue's git-as-authority principle from the very first interaction.

### T07: Convention enforcement scope and progressiveness

[RESOLVED on T07]
I agree with Churro's progressive enforcement model and Croissant's distinction between repo-side vs Jira-side enforcement. The resolution:

- **Repo-side conventions** (RFC front matter fields, Epic manifest schema, naming patterns): enforced by `blue lint`, progressive (warn-only for first N syncs, then strict).
- **Jira-side conventions** (workflow configurations, required fields, allowed transitions): validated by Blue during sync (Blue checks that the Jira project's workflow matches expectations) but never mutated by Blue. If Jira's workflow config deviates, Blue warns and documents the expected setup in the guide.

Blue should not attempt to configure Jira programmatically. That is an admin concern outside Blue's trust boundary. Blue validates and reports; humans fix Jira configuration.

### T03: Token storage, scoping, and rotation

[PERSPECTIVE P05: Token lifecycle is a per-domain, per-account concern with explicit health checks]
Synthesizing Brioche, Macaron, Croissant, and Scone's positions: tokens should be stored in OS keychain (macOS Keychain, Linux secret-service), keyed by Atlassian domain, with Blue providing `blue jira auth status` to report token health. The PM repo declares the target domain; Blue resolves the credential from the local keychain. For CI, the token comes from CI secrets (env var), and Blue should detect this via a `BLUE_JIRA_TOKEN` env var fallback.

Bot accounts (per Brioche) are the recommended path for CI sync, with personal tokens for interactive use. Blue's guide should make this distinction explicit and `blue lint` should warn if a personal token is detected in a CI environment (detectable via common CI env vars like `CI=true`).

---

## Summary

| Marker | Tension | Move |
|--------|---------|------|
| REFINEMENT | T01 | Drift classification at field level, not system level. Cosmetic fields Jira-owned, structural fields git-owned. |
| CONCESSION | T02 | Accept IssueTracker trait with graceful degradation. No hard jira-cli dependency. |
| REFINEMENT | T04 | Repo-local task bindings + per-Epic files in PM repo eliminates contention for common case. |
| REFINEMENT | T05 | 1:1 Feature Release:Epic default with explicit override. Indirection via Blue-native IDs. |
| PERSPECTIVE | T06 | Import must produce reviewable PRs, not silent state mutation. |
| RESOLVED | T07 | Repo-side enforced progressively by lint; Jira-side validated but never mutated by Blue. |
| PERSPECTIVE | T03 | OS keychain per domain, env var fallback for CI, bot accounts for automation. |

**Claim**: The field-level ownership matrix (P03) and the repo-local/PM-repo split (T04 refinement) are the two structural decisions that will resolve the most downstream tensions simultaneously. If we agree on which fields git owns vs Jira owns, T01 collapses. If we agree on where bindings live, T04 and T05 collapse.
