# Cupcake -- Round 2 (Convergence)

## [MOVE:CONVERGE]

The architecture is ready for RFC. All 7 structural tensions are resolved with specific, composable designs. I have no final objections.

---

## Refinement Tension Positions (T08--T11)

These are implementation decisions, not architectural disagreements. I offer brief positions so the RFC author has concrete starting points.

### T08: Drift detection frequency and staleness window

The right default is **on-demand** (`blue sync` and `blue jira drift`), not polling. Blue is a CLI tool, not a daemon. For teams that want periodic detection, a cron-friendly `blue sync --dry-run --json` exit code (0 = clean, 1 = drift detected) is sufficient. CI pipelines can run this as a scheduled job and post drift reports to Slack or PR comments. No built-in scheduler needed for v1.

Staleness is a display concern: the drift report should include the last-sync timestamp from the PM repo's `sync-metadata.yaml` so users can judge freshness themselves. A configurable `stale_after` threshold (default: 24h) can emit a warning in `blue status` output.

### T09: Error ergonomics when jira-cli is absent

When a tracker is configured in the PM repo but the provider binary is not found on PATH, Blue should:
1. **Allow local operations** (RFC create, lint, status) to succeed with a single-line warning: `warn: jira provider not found; sync commands unavailable`.
2. **Fail fast on sync operations** (`blue sync`, `blue jira drift`) with a clear error message naming the expected binary and a link to installation docs.

This matches the existing `gh` pattern in Blue's forge integration -- local-first, remote-when-available.

### T10: Credential storage mechanism (TOML vs keychain-only)

TOML fallback is correct for v1. Keychain integration adds platform-conditional code paths (macOS Keychain, libsecret on Linux, Windows Credential Manager) that are high-effort, low-urgency for an initial release. The TOML file at `~/.config/blue/jira-credentials.toml` with `0600` permissions is a well-understood pattern (cf. `~/.docker/config.json`, `~/.npmrc`). The three-tier hierarchy from T03 resolution already accommodates keychain as tier 2 -- it can be implemented after v1 without breaking the credential resolution chain.

### T11: Bot account provisioning as adoption barrier

Bot accounts should be **recommended, not required**. For v1, personal API tokens work for both interactive and CI use. The RFC should document the bot-account path as a best practice for production CI, with a concrete setup guide, but `blue jira auth add` should accept any valid Atlassian API token without checking whether it belongs to a bot or human account. This removes the Jira-admin gate from initial adoption.

---

## Final Assessment of Resolved Architecture

The design that emerged across Rounds 0-1 is sound from a DevOps and CLI tooling perspective:

- **IssueTracker trait** (T02): Clean extension point. The out-of-process adapter contract means Blue's core never takes a compile-time dependency on any tracker. Testable via mock provider.
- **Unidirectional push with drift visibility** (T01): Avoids the distributed-consensus trap of bidirectional sync. The drift report teaches PMs the correct workflow without silently losing their edits.
- **Domain-keyed credentials** (T03): Handles multi-tenant consultancies and monorepo-with-multiple-Jira-projects. The env-var-first resolution order is CI-native.
- **Per-entity files + frontmatter bindings** (T04, T05): Eliminates cross-repo merge contention. The 1:1 Feature Release to Epic default with explicit override handles the 90% case cleanly.
- **Two-phase import** (T06): Reviewable adoption PRs, not silent state injection. Critical for trust.
- **Progressive enforcement** (T07): Warn-first prevents the bypass-Blue-entirely failure mode.

No gaps remain that would block RFC authorship.

---

## Summary

| Marker | Tension | Position |
|--------|---------|----------|
| CONVERGE | T01--T07 | All 7 original tensions resolved; architecture is sound |
| PERSPECTIVE | T08 | On-demand drift detection, cron-friendly dry-run, configurable staleness threshold |
| PERSPECTIVE | T09 | Local ops succeed with warning; sync ops fail fast with actionable error |
| PERSPECTIVE | T10 | TOML fallback for v1; keychain as post-v1 enhancement |
| PERSPECTIVE | T11 | Bot accounts recommended, not required; no Jira-admin gate for adoption |

**Tensions**: None raised. T08--T11 are implementation details addressable within the RFC.

**Moves**: [MOVE:CONVERGE] -- ready for RFC.

**Claim**: The Jira Cloud integration architecture is converged. The IssueTracker trait provides pluggability, unidirectional push with drift reports provides sync safety, domain-keyed credentials handle multi-tenancy, per-entity files eliminate fan-out contention, and progressive enforcement protects adoption. The 4 remaining refinement tensions (T08--T11) are implementation decisions that belong in the RFC's design details section, not in further deliberation. Ship the RFC.

---
