# Macaron -- Round 2 (Convergence)

## [MOVE:CONVERGE]

The architecture is ready for RFC. All 7 structural tensions (T01-T07) are resolved with specific, composable designs. The 4 refinement tensions (T08-T11) are implementation decisions that belong in the RFC's design-decisions section, not in further deliberation.

## Addressing Remaining Refinement Tensions

### T08: Drift detection frequency and staleness window

This is solvable within the RFC. My round-1 incremental JQL proposal (`updatedDate > lastSyncTimestamp`) handles the performance side. For staleness: `blue sync` should write a `last_synced_at` timestamp to `.blue/jira-sync-state.json` in the PM repo. `blue lint` can warn if the sync state is older than a configurable threshold (default: 24h). PMs who build on stale Jira state between syncs are doing so at their own risk -- the drift report on the next sync will surface any divergence. This does not require architectural changes; it is a configuration knob.

### T09: Error ergonomics when jira-cli is absent

RFC creation should always succeed locally. If a tracker is configured but the provider binary is absent, `blue rfc create` should emit a warning (`tracker configured but blue-jira-provider not found on PATH; Jira sync skipped`) and proceed. The RFC file is the source of truth; Jira projection is a convenience. This is a UX polish item, not an architectural concern.

### T10: Credential storage mechanism

The three-tier hierarchy from T03 resolution already handles this. TOML fallback exists for environments where OS keychain is unavailable (containers, headless CI without secret managers, WSL edge cases). For v1, support all three tiers; document keychain as recommended, TOML as fallback. No architectural tension here.

### T11: Bot account provisioning as adoption barrier

Bot accounts should be recommended, not required. The RFC should document two CI credential paths: (1) bot account with project-scoped token (preferred), (2) personal API token with explicit scope documentation (acceptable for small teams). `blue jira auth status` validates whichever token is configured. Adoption gates on Jira admin access are a documentation problem, not a design problem.

## Final Position on Resolved Architecture

The seven resolved tensions form a coherent, layered design:

1. **Field-ownership map** (T01) -- the sync-policy.toml partitioning structural/operational/descriptive fields is the right abstraction. It avoids bidirectional sync while respecting PM workflows.

2. **IssueTracker trait** (T02) -- out-of-process adapter via PATH discovery. Clean separation of concerns. Future adapters (Linear, Shortcut) slot in without Blue core changes.

3. **Domain-keyed credentials** (T03) -- three-tier hierarchy handles CI, interactive, and multi-org consultants. No raw tokens in config files.

4. **Per-entity files** (T04) -- one YAML per Epic, RFC bindings in front matter. Eliminates cross-repo write contention entirely. This was the multi-repo coordination breakthrough.

5. **1:1 Feature Release to Epic** (T05) -- with escape hatch for legacy Epic semantics. Blue-native ID as indirection layer keeps the mapping stable even if Jira keys change.

6. **Two-phase import** (T06) -- idempotent, incremental, produces reviewable PRs. Git becomes authority post-import.

7. **Progressive enforcement** (T07) -- three tiers from always-enforced to document-only. Per-domain enforcement mode in PM repo config.

This architecture respects Blue's git-first philosophy while making Jira a useful projection layer rather than a competing source of truth. The multi-repo story is clean: each repo owns its own RFC bindings, the PM repo owns Epic structure, and there are no shared mutable files that create contention.

## No New Tensions

I have no new tensions to raise. The remaining items (T08-T11) are implementation details that the RFC author can resolve with the guidance above.

---

## Summary

| Marker | ID | Detail |
|--------|-----|--------|
| CONVERGE | all | Architecture is stable; all 7 structural tensions resolved; 4 refinement tensions addressable within RFC |
| RESOLVED | T08 | Incremental JQL + configurable staleness threshold in sync state file |
| RESOLVED | T09 | Local RFC creation always succeeds; tracker absence emits warning, not error |
| RESOLVED | T10 | Three-tier credential hierarchy from T03 already covers this; TOML is valid fallback |
| RESOLVED | T11 | Bot accounts recommended not required; two documented CI credential paths |

**Perspectives**: 0 new (architecture is settled)
**Tensions**: 0 new (T08-T11 resolved as implementation decisions)
**Moves**: [MOVE:CONVERGE]
**Claim**: The resolved architecture -- field-ownership maps, IssueTracker trait, per-entity files, and progressive enforcement -- forms a coherent design that is ready for RFC authoring. The 4 refinement tensions are implementation knobs, not architectural disagreements, and should be captured as design decisions in the RFC rather than deliberated further.
