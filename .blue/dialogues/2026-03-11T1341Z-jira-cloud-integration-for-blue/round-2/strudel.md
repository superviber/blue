# Strudel -- Round 2 (Convergence)

## [MOVE:CONVERGE]

The architecture is ready for RFC. All 7 structural tensions are resolved with specific, composable designs. I have reviewed the resolutions against my three data integrity invariants from Round 1 and confirm they hold:

1. **Immutable UUID join keys** -- Covered by T04 resolution (RFC-to-Task binding in front matter with UUID) and T07 Tier 1 enforcement (UUID required from day one).
2. **Verified projections** -- My P01 (verification callback in IssueTracker trait) is compatible with the T02 resolution. The trait's adapter contract should include confirmation payloads. This is an implementation detail the RFC can specify without further deliberation.
3. **Pre-flight credential validation** -- My P02 fits cleanly within the T03 credential hierarchy. The three-tier credential store (env var, keychain, TOML fallback) provides the lookup mechanism; pre-flight auth check is an operational concern layered on top. No tension with the resolved architecture.

## Remaining Refinement Tensions (T08--T11)

These are implementation-level decisions, not architectural disagreements. My position on each:

**T08 (Drift detection frequency)**: Default to sync-on-push (run drift detection as part of `blue sync`, not on a timer). Staleness between syncs is acceptable because git is authoritative -- PMs who build on stale Jira state are building on a projection, not the source of truth. The drift report makes this visible. No architectural change needed.

**T09 (Error ergonomics when jira-cli absent)**: RFC creation should always succeed locally. If the IssueTracker adapter is absent, Blue should log a warning ("Jira projection deferred -- no tracker adapter found") and record a pending projection marker in the RFC front matter. Next `blue sync` with an available adapter picks it up. This is consistent with the T02 graceful degradation design.

**T10 (Credential TOML vs keychain-only for v1)**: Ship both. The TOML fallback is essential for headless CI environments where OS keychain is unavailable. `blue lint` already detects tokens in staged git files (T03 resolution). The risk of TOML-on-disk is mitigated by file permissions (0600) and the lint guard. No architectural tension.

**T11 (Bot account provisioning)**: Bot accounts should be recommended, not required. Personal API tokens work for individual developers and small teams. The credential hierarchy already supports both (env var for bot tokens in CI, keychain for personal tokens locally). Adoption guidance belongs in documentation, not enforcement logic.

None of these require further rounds. They are design decisions the RFC author can make, and `blue lint` plus the progressive enforcement model (T07) provide guardrails for revisiting them later.

## Final Verification

I have checked the 7 resolved tensions against the data integrity surface:

| Invariant | Covered By | Integrity Risk | Status |
|-----------|-----------|----------------|--------|
| UUID join keys | T04, T07 Tier 1 | Broken references between Blue entities and Jira issues | Mitigated |
| Verified projections | T02 (IssueTracker trait) | Silent API failures creating phantom state | Mitigated (P01 adds verification callback) |
| Pre-flight auth | T03 (credential hierarchy) | Partial-projection from mid-sync auth expiry | Mitigated (P02 adds pre-flight check) |
| Drift auditability | T01 (three-tier field ownership) | PM edits silently overwritten without trace | Mitigated (drift log) |
| Import integrity | T06 (two-phase import) | Corrupt bindings from inconsistent Jira state | Mitigated (dry-run + adoption PR review) |
| Referential integrity | T07 Tier 1 | Dangling Epic references, orphaned Tasks | Mitigated (always-enforced structural validation) |

No gaps. The architecture is sound from a data integrity perspective.

---

## Summary

| Marker | Count | Details |
|--------|-------|---------|
| PERSPECTIVE | 0 | No new perspectives -- prior positions (P01--P05) are incorporated in resolutions |
| TENSION | 0 | No new tensions raised |
| REFINEMENT | 0 | -- |
| CONCESSION | 0 | -- |
| MOVE | 1 | CONVERGE |

**Claim**: The resolved architecture satisfies all three data integrity invariants (immutable join keys, verified projections, pre-flight credential validation) and the four remaining refinement tensions (T08--T11) are implementation decisions that do not affect architectural soundness. This dialogue is ready to produce an RFC.
