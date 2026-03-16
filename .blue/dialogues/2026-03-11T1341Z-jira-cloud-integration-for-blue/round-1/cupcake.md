# Cupcake -- Round 1

## Tension Engagement

### T02: jira-cli dependency model (bundle vs adapter vs trait)

[RESOLVED T02]

Round 0 produced strong convergence here. Muffin proposed an `IssueTracker` trait mirroring the existing `Forge` pattern. Beignet and Cannoli both independently argued against hard-bundling jira-cli. My own Round 0 position (jira-cli as recommended adapter, not hard dependency) aligns with all of these.

The resolution is clear: Blue defines an `IssueTracker` trait with a `JiraCloudTracker` implementation that shells out to `jira` if present, and degrades gracefully if absent. The trait surface is small -- `create_issue`, `transition_issue`, `link_issue`, `get_issue_status`. Blue tests against a mock provider in CI. jira-cli becomes a "recommended runtime dependency" documented in setup guides, exactly like `gh` is for git-based tools.

I concede to Muffin that the trait should live in a `tracker` module parallel to `forge`, not in a `jira`-specific namespace. This keeps the door open for Linear/Shortcut/Plane adapters without refactoring.

**Consensus position**: `IssueTracker` trait with `JiraCloudTracker` as first implementation; jira-cli as optional runtime dependency; mock provider for CI. No hard coupling.

---

### T03: Token storage, scoping, and rotation

[REFINEMENT T03]

Brioche's Round 0 position (dedicated bot account, least-privilege scoping) is correct for CI/production sync. Cannoli's multi-domain credential concern is operationally critical -- consultants and platform teams hit this immediately. Macaron's trust-boundary distinction (personal tokens vs CI service account tokens) is the missing piece.

I refine my Round 0 position into a concrete proposal:

1. **Credential store**: `~/.config/blue/credentials.toml`, keyed by Atlassian domain (e.g., `[myorg.atlassian.net]`). Never in any git repo.
2. **Selection**: The project-management repo's `jira.toml` declares the target domain and project key. Blue resolves credentials by matching the domain key. No global default.
3. **Auth commands**: `blue jira auth add <domain>` writes to the credential store. `blue jira auth status` checks token validity and reports expiry. `blue jira auth rotate` guides rotation.
4. **CI path**: CI uses env vars (`BLUE_JIRA_TOKEN`, `BLUE_JIRA_DOMAIN`), documented as the service-account path. Blue checks env vars first, falls back to credential store.
5. **Guardrails**: `blue lint` fails if any file matching `*credentials*`, `*token*`, or `.jira*config*` is staged in any repo. The PM repo template ships with `.gitignore` patterns.

Scone's keychain integration (macOS Keychain, Linux secret-service) is aspirational but adds platform-specific complexity. I propose we start with the TOML file (mode 0600, user-only read) and add keychain support as a later enhancement behind a `--keychain` flag.

**Remaining tension**: Agreement is needed on whether the TOML credential store is sufficient for v1 or whether keychain integration is a hard requirement. I lean toward TOML-first.

---

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[PERSPECTIVE P01: Drift detection as a CLI-native feedback loop, not a sync problem]

This is the highest-impact tension and I want to reframe it from a DevOps/CLI perspective. The question is not "how do we sync back from Jira" -- it is "how does Blue make drift visible and actionable without becoming a bidirectional sync engine."

Every expert agrees git is ground truth. The real-world problem is that PMs will edit Jira. We cannot prevent it (Jira's permission model is too coarse for field-level locks on most plans). We should not ignore it (invisible drift erodes trust). We must not sync it back (bidirectional sync is a distributed consensus problem we have no business solving in a CLI tool).

The answer is a **drift report**:

- `blue sync --dry-run` compares local RFC state against Jira state and reports divergences as a structured diff (status mismatch, assignee mismatch, missing fields).
- `blue sync` pushes git state to Jira, overwriting Jira-side edits, but logs every overwrite to a `sync-audit.log` in the PM repo.
- A `blue jira drift` command produces a human-readable report that PMs can review: "These 4 tickets were modified in Jira since last sync. The next `blue sync` will overwrite these changes."

This keeps the architecture simple (unidirectional push), makes drift visible (audit log + drift report), and gives PMs a feedback loop (they can see what will be overwritten before it happens). If a PM's Jira edit is important, the correct path is to update the RFC in git and let Blue push the change -- the drift report teaches that behavior over time.

[CONCESSION to Strudel on T01]: Strudel's immutable UUID join key is essential for this drift detection to work. Without a stable identifier that survives Jira ticket moves, the drift report cannot reliably match git state to Jira state. I fully adopt the UUID-in-frontmatter + UUID-in-Jira-custom-field approach.

---

### T04: Multi-repo fan-out and atomic consistency

[PERSPECTIVE P02: Per-entity files eliminate the coordination bottleneck]

Donut raised the real risk: if the PM repo uses a single manifest file, concurrent cross-repo pushes serialize on merge conflicts. My Round 0 position (append-only, per-entity files) directly addresses this.

Macaron's Round 0 insight -- RFC-to-Task binding lives in RFC frontmatter, not in the PM repo -- is the key refinement. If each RFC carries its own `jira_task_key` and `jira_uuid` in frontmatter, and the PM repo only declares Epic structure and repo membership, then cross-repo writes to the PM repo become rare (only Epic creation/completion, not per-RFC updates). This eliminates the fan-out coordination problem for the common case.

The PM repo structure should be:

```
domains/
  payments/
    domain.yaml          # repo list, Jira project key
    epics/
      2026-q2-checkout.yaml   # one file per Epic
```

Each Epic file lists constituent RFC IDs (not Jira keys) discovered by scanning member repos. `blue sync` reads this structure, resolves RFC-to-Jira bindings from each repo's frontmatter, and pushes the aggregate state to Jira. No shared registry file. No merge conflicts.

[CONCESSION to Macaron on T04]: RFC-to-Task binding belongs in RFC frontmatter, not the PM repo. The PM repo owns only Epic-level structure. This is the correct separation of concerns.

---

### T06: Bootstrapping from existing Jira state

[REFINEMENT T06]

Churro is right that clean-slate assumptions kill adoption. I refine the `blue jira import` proposal with a concrete CLI design:

- `blue jira import --project PROJ --epic PROJ-100` imports a single Epic and its child Tasks, creating RFC stubs in the local repo with pre-populated frontmatter (`jira_task_key`, `jira_uuid` minted at import time).
- `blue jira import --project PROJ --all-epics` bootstraps the PM repo's Epic structure from all active Epics.
- Import is a one-time operation that establishes the initial binding. After import, git is ground truth and all subsequent changes flow outward.
- The import command writes a `bootstrap-manifest.yaml` recording what was imported and when, providing an audit trail.

This is a CLI-native onramp that respects the git-first model while acknowledging existing Jira state.

---

### T07: Convention enforcement scope and progressiveness

[CONCESSION to Churro on T07]

Churro's progressive enforcement model (warn-only during onboarding, strict after opt-in) is correct. I concede that strict-from-day-one will cause teams to bypass Blue and edit Jira directly -- exactly the drift pattern we are trying to prevent.

Concrete proposal: `blue lint --jira` defaults to `warn` mode. Teams opt into `strict` mode via a `[jira.enforcement]` section in the PM repo's config. Enforcement tiers:

1. **warn**: Log convention violations, allow sync to proceed.
2. **strict**: Block sync on convention violations, require `--force` to override.

The enforcement scope should be limited to artifacts Blue controls (RFC frontmatter, PM repo manifests, Epic naming). Blue should NOT attempt to enforce Jira-side workflow configurations (custom fields, screen schemes, transitions) -- that is Jira admin territory and outside Blue's blast radius.

---

## Summary

| Marker | Tension | Position |
|--------|---------|----------|
| RESOLVED | T02 | `IssueTracker` trait, jira-cli as optional runtime dep, mock for CI |
| REFINEMENT | T03 | Domain-keyed TOML credential store, env var override for CI, `blue jira auth` commands |
| PERSPECTIVE | T01 | Drift report + audit log, unidirectional push with visibility |
| CONCESSION | T01 | Adopt Strudel's immutable UUID join key |
| PERSPECTIVE | T04 | Per-entity files in PM repo, RFC-to-Task binding in frontmatter |
| CONCESSION | T04 | Adopt Macaron's frontmatter-owns-binding model |
| REFINEMENT | T06 | `blue jira import` with single-Epic and all-Epics modes |
| CONCESSION | T07 | Progressive enforcement (warn then strict), no Jira-side enforcement |

**Claim**: The integration architecture is converging on a clean separation -- `IssueTracker` trait for pluggability, unidirectional push with drift visibility for sync direction, domain-keyed credentials for auth, per-entity files for fan-out safety, and progressive enforcement for adoption. The remaining open question is T05 (Epic cardinality across repos), which I defer to Cannoli and Donut who surfaced it.

---
