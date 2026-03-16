# Round 2 -- Donut (Workflow Analyst)

## Convergence Assessment

All 7 original tensions (T01-T07) are resolved with specific, composable designs. I have reviewed each resolution against my Round 1 positions and find no remaining architectural disagreements.

### Confirming Resolutions

**T01 (Field Ownership)**: The three-tier field ownership model -- structural/operational/descriptive -- is exactly what I advocated in P03. The configurable drift policy (overwrite/warn/block) gives teams the flexibility they need without sacrificing git authority over structural state. No objections.

**T02 (IssueTracker Trait)**: The out-of-process adapter contract with PATH-discoverable `blue-jira-provider` is clean and extensible. My Round 1 concession stands. The graceful degradation model protects offline-first workflows.

**T03 (Credentials)**: The three-tier credential hierarchy (env var, OS keychain, TOML fallback) addresses all deployment contexts. I am satisfied with the domain-keyed approach.

**T04 (Multi-repo)**: The repo-local task bindings plus per-entity Epic files in the PM repo is the architecture I refined in Round 1. It eliminates cross-repo write contention for common operations. No objections.

**T05 (Epic Cardinality)**: 1:1 default with explicit override and Blue-native Feature Release ID as indirection -- exactly the model I proposed. Jira key changes are isolated to one mapping file.

**T06 (Bootstrap Import)**: Two-phase import producing reviewable PRs, not silent state injection. This preserves git-as-authority from day one. No objections.

**T07 (Progressive Enforcement)**: Three-tier progressive enforcement with Blue validating but never mutating Jira-side configuration. Matches my Round 1 resolution exactly.

### On Remaining Refinement Tensions (T08-T11)

These four tensions are implementation-level decisions, not architectural disagreements. They can and should be resolved within the RFC itself:

- **T08 (Drift detection frequency)**: Recommend specifying a default sync cadence in the RFC (e.g., on `blue sync` invocation, not scheduled) and noting that staleness is inherent in any projection model. The drift report already mitigates this.
- **T09 (Error ergonomics)**: RFC creation should always succeed locally. When tracker is configured but unavailable, `blue sync` warns and queues intent. This follows the existing T02 graceful degradation resolution.
- **T10 (Credential storage mechanism)**: The three-tier hierarchy already resolves this -- TOML is Tier 3 fallback for v1 when keychain is unavailable. No need for keychain-only enforcement.
- **T11 (Bot account provisioning)**: Bot accounts are recommended, not required. Personal tokens work for initial adoption. The guide should document the bot account path without gating on it.

None of these require further deliberation rounds.

---

## [MOVE:CONVERGE]

The architecture is ready for RFC drafting. The seven structural resolutions form a coherent, composable design:

1. Field-level ownership matrix eliminates the sync direction debate
2. IssueTracker trait with adapter contract eliminates vendor lock-in
3. Domain-keyed credential hierarchy covers CI and interactive contexts
4. Repo-local task bindings plus per-entity PM repo files eliminate contention
5. 1:1 Epic mapping with explicit override handles diverse team semantics
6. Two-phase import preserves git authority from bootstrap
7. Progressive enforcement respects trust boundaries

No final objections. Proceed to RFC.

---

## Summary

| Marker | Tension | Move |
|--------|---------|------|
| CONVERGE | T01 | Three-tier field ownership confirmed. No objections. |
| CONVERGE | T02 | IssueTracker trait with graceful degradation confirmed. |
| CONVERGE | T03 | Domain-keyed credential hierarchy confirmed. |
| CONVERGE | T04 | Repo-local bindings + per-entity Epic files confirmed. |
| CONVERGE | T05 | 1:1 default with explicit override confirmed. |
| CONVERGE | T06 | Two-phase import producing reviewable PRs confirmed. |
| CONVERGE | T07 | Progressive three-tier enforcement confirmed. |
| NOTE | T08-T11 | Refinement-level; resolvable within RFC text. |

**Perspectives**: 0 new (all prior perspectives incorporated into resolutions)

**Tensions**: 0 new raised. T08-T11 deferred to RFC as implementation decisions.

**Moves**: [MOVE:CONVERGE] -- all 7 original tensions resolved, architecture is coherent and composable.

**Claim**: The resolved architecture achieves the workflow analyst's primary concern -- that Blue's Jira integration preserves git-as-authority for structural state while respecting the operational workflows of PMs and stakeholders who live in Jira. The field-level ownership matrix (T01) and repo-local binding model (T04) are the two load-bearing decisions that make everything else work. Ready for RFC.
