# Eclair -- Round 1

## Addressing Open Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]

In Round 0, I proposed the offline-first pending-sync queue (`pending-sync.yaml`). After reading Strudel, Brioche, and Cannoli's contributions, I want to sharpen the proposal into a concrete policy rather than leaving it as "detect and warn."

**Proposed resolution: Last-write-from-git-wins with drift reporting.**

Blue should adopt a three-tier policy for Jira-side edits:

1. **Projection-only fields** (status, summary, description, epic link): Blue overwrites on every `blue sync`. Jira edits to these fields are ephemeral. This is the default and the only sane choice for fields that derive from git-controlled artifacts.
2. **Jira-native fields** (assignee, sprint membership, story points, priority): Blue never writes these. They are Jira-side operational metadata that PMs own. Blue does not read them back either -- they exist outside the git truth boundary.
3. **Drift-detected fields** (labels, components): Blue writes them on creation, then logs a warning if Jira values diverge on subsequent syncs but does not overwrite. A `blue sync --force` flag allows explicit overwrite.

This partitioning acknowledges Brioche's concern (T02 in her R0) that "git-as-ground-truth vs Jira-as-ground-truth" is not a binary choice -- it is a per-field decision. It also addresses Strudel's fear that PMs become "second-class participants": they retain full ownership of operational fields, while structural fields stay git-authoritative.

Cannoli's push for "push-only sync" is preserved: Blue never reads Jira state to update git state. The drift detection on tier-3 fields is informational only, never mutates local state.

[PERSPECTIVE P03: The drift report should be a first-class CLI artifact]

`blue sync` should output a structured drift report (YAML or JSON) that lists every field where Jira diverges from the git-declared state. This report can feed into CI checks, Slack notifications, or dashboards. Without a machine-readable drift report, teams will discover divergence only when something breaks, not when it drifts.

### T02: jira-cli dependency model

[RESOLVED on T02 -- adopting Muffin's IssueTracker trait with Cupcake's adapter model]

Round 0 surfaced three positions: hard bundle (nobody wants this), recommended adapter (Cupcake), and IssueTracker trait (Muffin). These are not in conflict -- the trait defines the contract, the adapter implements it via jira-cli.

The resolution: Blue ships an `IssueTracker` trait (following the `Forge` pattern Muffin identified). The default `JiraCloudTracker` implementation shells out to `jira-cli` if present. If `jira-cli` is absent, `blue sync` fails with a clear message ("Jira sync requires jira-cli; install it via ...") rather than silently no-oping. Beignet's concern about "fragmented user experiences where some commands silently no-op" is addressed by making the failure explicit and actionable.

From a DX perspective, I want to add one refinement: `blue jira doctor` should validate the full chain -- jira-cli installed, correct version, authenticated, target project accessible -- in one command. Developers should never debug sync failures by guessing which layer broke.

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]

Brioche's bot-account recommendation and Croissant's multi-domain keying are both correct and complementary. I concede my Round 0 position did not address multi-domain support.

**Concrete proposal for credential architecture:**

- Credentials stored in `~/.config/blue/jira-credentials.toml`, keyed by Atlassian domain (per Croissant P03).
- The project-management repo declares its target domain in `jira.toml` (domain URL + project key only, never tokens).
- `blue jira auth` writes credentials to the local TOML. `blue jira auth status` reports token health, expiry, and permission scope (per Scone P02).
- For CI: tokens come from environment variables (`BLUE_JIRA_TOKEN`, `BLUE_JIRA_DOMAIN`), overriding the TOML file. Macaron's trust-boundary distinction (personal vs CI tokens) is enforced by convention: the CI env var path skips the TOML entirely.
- `blue lint` checks that no credential-like strings appear in staged files (per Brioche P01).

I am not yet convinced OS keychain integration is worth the cross-platform complexity for v1. Plaintext TOML with restrictive file permissions (0600) and `.gitignore` enforcement is a pragmatic starting point. Keychain support can be a Phase 2 enhancement.

[TENSION T08: Credential TOML vs OS keychain -- pragmatism vs security posture]

Storing tokens in a plaintext TOML (even with 0600 permissions) is a weaker security posture than OS keychain integration. For teams with strict compliance requirements (SOC2, FedRAMP), this may be a blocker. But keychain integration across macOS, Linux (secret-service/kwallet), and WSL is a significant engineering effort. Should v1 ship with the simpler model and accept the security trade-off, or should keychain be a hard requirement from day one?

### T04: Multi-repo fan-out and atomic consistency

[CONCESSION on T04 -- adopting Macaron's repo-local binding model]

Macaron's Round 0 proposal (P02) that RFC-to-Task bindings live in RFC front matter within each repo, not in the central project-management repo, is the correct resolution. I initially assumed the project-management repo would hold all bindings, but Macaron is right: centralizing per-RFC bindings in a shared repo creates O(n) merge conflicts as repos scale. Beignet's worry about "cross-repo write dependency that serializes parallel work" (T01) is solved by this model.

The project-management repo should declare only:
- Domain membership (which repos belong to the domain)
- Epic/Feature Release structure (which Epics exist, their lifecycle state)
- The Jira project key and domain URL

Per-RFC Jira bindings live in each RFC's front matter (`jira_task_key: PROJ-142`, `jira_uuid: <stable-uuid>`). The `blue sync` reconciliation reads these from member repos (or their latest `main` branch via git archive) to build the full picture.

This eliminates the atomic consistency problem for RFC-level bindings. Epic-level mutations remain centralized but are low-frequency enough that merge conflicts are manageable.

### T05: Epic/Feature Release cardinality across repos

[PERSPECTIVE P04: Epics are domain-scoped, not repo-scoped -- enforce 1:1 Feature Release to Epic]

Cannoli raised the cardinality trap (R0 P03): orgs use Epics inconsistently. Blue should enforce that one Feature Release in the project-management repo maps to exactly one Jira Epic, period. If a team's existing Epic semantics differ, they use the `jira_epic_key` override in the manifest (as Cannoli suggested) to bind to an existing Epic rather than creating a new one. But Blue should never create multiple Epics for one Feature Release or map one Epic to multiple Feature Releases. Keeping this 1:1 prevents the combinatorial explosion that makes reconciliation intractable.

### T06: Bootstrapping from existing Jira state

[REFINEMENT on T06 -- supporting Churro's `blue jira import`]

Churro is right that clean-slate assumptions kill adoption. The `blue jira import` command should:

1. Query the target Jira project for active Epics and their child Tasks.
2. Generate a project-management repo manifest with Epic entries and RFC stubs.
3. For each Task, create a minimal RFC file in the appropriate repo with `jira_task_key` and `jira_uuid` pre-populated.
4. Run in `--dry-run` mode by default, outputting the proposed manifest to stdout before writing.

This is an inversion of the normal flow (Jira-to-git instead of git-to-Jira) and is acceptable as a one-time bootstrapping operation, not ongoing sync. After import, the git repo becomes authoritative and the normal push-only model applies.

### T07: Convention enforcement scope and progressiveness

[RESOLVED on T07 -- adopting Churro's progressive enforcement with schema validation]

Churro's tiered enforcement (warn-then-strict) and my Round 0 schema contract (P02) combine cleanly:

- Blue ships a `jira-conventions.schema.json` in the project-management repo template.
- `blue lint` validates against this schema. In `--warn` mode (default for first 30 days or until `strict: true` in config), violations are warnings. In `--strict` mode, they are errors that block `blue sync`.
- The schema covers repo-side conventions only (field mappings, Epic naming, required front matter). Blue does not attempt to enforce Jira-side workflow configuration -- that is the Jira admin's domain.

Croissant's concern (T02) about "how far should Blue enforce Jira-side conventions" is answered: Blue validates its own output, not Jira's input. If Blue's projected state does not match Jira's accepted schema, that is a Jira admin configuration issue surfaced as a sync error, not a Blue lint violation.

---

## Summary

| Marker | Count | Details |
|--------|-------|---------|
| PERSPECTIVE | 2 | P03 (drift report as CLI artifact), P04 (1:1 Epic cardinality) |
| TENSION | 1 | T08 (credential TOML vs OS keychain for v1) |
| RESOLVED | 2 | T02 (IssueTracker trait + adapter), T07 (progressive schema enforcement) |
| REFINEMENT | 3 | T01 (three-tier field ownership), T03 (concrete credential architecture), T06 (import command spec) |
| CONCESSION | 1 | T04 (adopting Macaron's repo-local binding model) |

**Claim**: The three-tier field ownership model (projection / Jira-native / drift-detected) resolves T01 without forcing a binary "git wins" or "Jira wins" choice, and the combination of IssueTracker trait + repo-local RFC bindings + progressive schema enforcement resolves T02, T04, and T07 with concrete, implementable designs.
