# Eclair -- Round 2 (Convergence)

## [MOVE:CONVERGE]

The architecture is ready for RFC. All 7 structural tensions are resolved with specific, composable designs. I am signaling convergence.

## Assessment of Remaining Tensions (T08--T11)

These are implementation-level decisions that belong in the RFC as design choices, not open architectural questions requiring further deliberation. Here is my position on each, for the RFC author's reference:

### T08: Drift detection frequency and staleness window

**Position**: `blue sync` is explicitly invoked, not scheduled. Staleness is the caller's problem, not Blue's. CI pipelines can run `blue sync --check` (dry-run mode) on a cron to surface drift without mutating Jira. The staleness window is whatever the team's CI schedule dictates. Blue should not embed an opinion about polling frequency -- that couples Blue to infrastructure assumptions it cannot control. Document the `--check` flag pattern in the integration guide and move on.

### T09: Error ergonomics when jira-cli is absent

**Position**: `blue rfc create` succeeds locally regardless of tracker availability. The RFC is a git artifact first. If `jira.toml` declares a tracker but the provider binary is not on PATH, Blue emits a warning ("Jira sync configured but blue-jira-provider not found; RFC created locally, run `blue sync` when provider is available") and writes the pending-sync intent to the local queue. This preserves offline-first semantics (my R0 P01) while surfacing the gap. `blue sync` itself fails hard if the provider is absent -- that is the appropriate failure boundary.

### T10: Credential storage mechanism

**Position**: I raised this tension in R1 (as T08 in my numbering, now T10). After reflection, I concede to the three-tier hierarchy the panel converged on (env var > keychain > TOML fallback). The TOML fallback with 0600 permissions is acceptable for v1 because: (a) it provides a works-everywhere baseline, (b) teams with compliance requirements can disable it via config (`credential_store: keychain-only`), and (c) `blue lint` already guards against credential leakage into git. Ship all three tiers in v1; let teams choose their posture.

### T11: Bot account provisioning as adoption barrier

**Position**: Bot accounts are recommended, not required. The RFC should document a "personal token" quick-start path (5-minute setup) alongside the "bot account" production path (requires Jira admin). This mirrors how GitHub Apps work -- you can use a PAT to get started and migrate to an App later. The `blue jira auth` flow should handle both token types identically; the distinction is organizational, not technical.

## Final Perspectives

[PERSPECTIVE P05: The RFC should include a `blue jira doctor` command spec]

I proposed this in R1 but want to elevate it as a concrete RFC deliverable. `blue jira doctor` validates the entire integration chain in one command: provider binary present and correct version, credentials valid and not expired, target project accessible, field mappings resolvable. This is the single most important DX investment for the integration. Every hour spent on `doctor` saves a hundred hours of "why is sync broken?" debugging across the user base. The RFC should specify its output contract (structured JSON with per-check pass/fail/skip) so CI can consume it.

[PERSPECTIVE P06: The RFC phasing should front-load the read-only path]

Phase 1 of the RFC should deliver `blue jira import` (read from Jira) and `blue sync --check` (dry-run drift detection) before any write path. This lets teams adopt incrementally: first validate that Blue's model of their Jira state is correct, then enable writes. Shipping writes before reads inverts the trust-building sequence.

## Summary

| Marker | Count | Details |
|--------|-------|---------|
| MOVE | 1 | CONVERGE |
| PERSPECTIVE | 2 | P05 (`blue jira doctor` as RFC deliverable), P06 (phase read-only before writes) |
| TENSION | 0 | -- |
| CONCESSION | 1 | T10 (accepting three-tier credential hierarchy including TOML fallback for v1) |

**Claim**: The resolved architecture -- IssueTracker trait with out-of-process adapters, three-tier field ownership, repo-local RFC bindings, domain-keyed credentials, two-phase import, and progressive enforcement -- is a coherent, implementable design. The 4 remaining tensions are implementation choices that the RFC can resolve inline. I endorse moving to RFC authoring with two DX recommendations: front-load the read-only path (Phase 1 = import + dry-run) and include `blue jira doctor` as a first-class deliverable.
