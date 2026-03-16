[PERSPECTIVE P01: T01 resolves through "git-wins-with-drift-detection" -- not ignore, not block, but warn-and-overwrite]

The panel has near-unanimous agreement that git is ground truth. The remaining T01 tension is *what happens when PMs edit Jira directly*. I propose a concrete resolution: Blue's `blue sync` command overwrites Jira state to match the project-management repo (git wins), but before overwriting, it emits a structured drift report listing every field where Jira diverged. This drift report is logged locally (`.blue/jira/drift-log.yaml`) and optionally posted as a Jira comment on the affected ticket. The key insight is that "warn" and "overwrite" are not mutually exclusive -- Blue can do both. PMs who need to influence state learn to submit PRs to the project-management repo (or request them from developers), and the drift log provides an audit trail that surfaces habitual Jira-side editing so teams can address the social problem. Blocking sync on drift (as Brioche's "refuse to proceed until reconciled" implies) would grind adoption to a halt in any org where PMs live in Jira. Silently ignoring drift (pure push-only) means PMs never learn the system boundary. Warn-and-overwrite is the pragmatic middle.

[REFINEMENT on T01: Croissant's "explicit conflict resolution order" refines to a single rule]

Croissant proposed field-level conflict resolution (git wins for metadata, Jira wins for workflow status). I concede this sounds flexible but argue it creates a hybrid authority that is harder to reason about than a single rule. If Jira wins for *any* field, Jira becomes a partial source of truth, and Blue must implement bidirectional sync for those fields -- exactly the complexity everyone agreed to avoid. The cleaner design: git wins for everything Blue manages (status, epic membership, labels, summary). Jira-only fields that Blue does not map (assignee, sprint, story points, custom fields) are left untouched by sync. This gives PMs freedom to use Jira for operational fields Blue does not care about, while Blue maintains strict authority over structural fields.

[RESOLVED on T01: Git-wins-with-drift-detection for all Blue-managed fields; Jira-only fields left untouched]

This resolves T01 by defining a clear boundary: Blue manages a declared set of fields (status, epic link, summary, labels), overwrites them on sync, and logs drift. Jira-native fields (assignee, sprint, story points) are outside Blue's scope and PMs can edit them freely. No bidirectional sync needed. No blocking on divergence.

[PERSPECTIVE P02: T02 resolves through the IssueTracker trait with jira-cli as the default adapter]

Muffin proposed the IssueTracker trait; Cupcake proposed jira-cli as recommended-not-required behind a provider interface; Beignet argued bundling is a liability. These three positions converge naturally. The IssueTracker trait (matching the existing Forge pattern) is the right abstraction. The `JiraCloudTracker` implementation shells out to `jira-cli` if available and returns a clear error with install instructions if not. No bundling, no hard dependency, no silent no-op. Blue ships zero Jira binaries. The trait interface means Linear or Shortcut adapters can be added later without touching core. This is the Forge pattern applied consistently.

[CONCESSION on T02: Cupcake is right that direct REST is a future escape hatch, not a launch requirement]

Cupcake raised that Blue could bypass jira-cli entirely with direct REST calls. I concede this is the right long-term direction (eliminates the third-party dependency entirely), but for launch, shelling out to jira-cli is pragmatic -- it handles OAuth, pagination, and Atlassian's API quirks. The trait boundary ensures the swap from jira-cli to direct REST is a single-module change with no API surface impact on Blue's commands.

[RESOLVED on T02: IssueTracker trait with JiraCloudTracker adapter shelling out to jira-cli; no bundling]

[PERSPECTIVE P03: T03 resolves through platform keychain with domain-keyed lookup]

Combining my Round 0 position (platform keychain with expiry checks), Croissant's multi-domain requirement, and Brioche's bot-account recommendation: tokens live in OS keychain (macOS Keychain, Linux secret-service, Windows Credential Manager), keyed by Atlassian domain. The project-management repo declares the target domain in `jira.toml` (domain only, never credentials). Blue resolves the credential by looking up `blue-jira/{domain}` in the platform keychain. For CI, the same lookup falls through to environment variables (`BLUE_JIRA_TOKEN_{DOMAIN_HASH}`). `blue jira auth status` reports token health per domain. Personal tokens for development; bot account tokens for CI. The project-management repo ships with `.gitignore` patterns blocking credential files, and `blue lint` checks for token-shaped strings in staged files.

[REFINEMENT on T03: Brioche's bot account recommendation becomes a convention, not a requirement]

Brioche argued for dedicated bot accounts with least-privilege scoping. This is the right convention for production CI sync, but mandating it at onboarding time adds friction (you need Atlassian admin access to create service accounts). Blue should document bot accounts as the recommended pattern and support personal tokens as a valid starting point, with `blue jira auth status` surfacing a warning when a personal token is used in a CI context.

[RESOLVED on T03: Platform keychain, domain-keyed, with CI env-var fallback; bot accounts recommended for CI]

[PERSPECTIVE P04: T07 converges with Churro's progressive enforcement model]

Churro's Round 0 contribution on progressive convention enforcement is the right answer for T07. Blue should ship with a `jira.conventions.mode` config flag: `warn` (default for first 30 days or until explicitly changed) and `strict`. In `warn` mode, `blue sync` reports convention violations (wrong Epic naming, missing labels) but completes the sync. In `strict` mode, it refuses to sync until violations are fixed. This mirrors how `blue lint` works today and respects that convention buy-in is a social adoption curve, not a switch.

[REFINEMENT on T07: Scoping enforcement to Blue-managed conventions only]

Croissant raised T07 asking which conventions are enforced repo-side vs Jira-side. The answer: Blue enforces conventions only on artifacts it writes (Epic names, Task summaries, labels it manages, status transitions). It does not attempt to validate or enforce Jira workflow configurations, custom field schemas, or board layouts -- those are Jira admin concerns outside Blue's authority. This avoids the impossible task of programmatically auditing Jira's admin settings.

[RESOLVED on T07: Progressive enforcement (warn then strict) scoped to Blue-managed artifacts only]

[TENSION T01-R1: Drift detection frequency and staleness window]

The warn-and-overwrite model I propose for T01 only works if sync runs frequently enough to catch drift before PMs build decisions on stale Jira state. If a team runs `blue sync` once a week, a PM could spend five days working from a Jira status that Blue is about to overwrite. Should Blue recommend a minimum sync cadence, or should the CI integration auto-sync on every merge to main? This is a deployment convention question the panel has not addressed.

[TENSION T02-R1: IssueTracker trait error ergonomics when jira-cli is absent]

If `blue rfc create` triggers a Jira sync and jira-cli is not installed, does the RFC creation succeed (with a warning that Jira sync was skipped) or fail entirely? The offline-first principle (Eclair) suggests RFC creation must always succeed locally, with Jira sync deferred. But this means Blue accumulates a sync queue that a user without jira-cli installed may never drain, silently degrading the projection. The trait needs a clear distinction between "tracker not configured" (silent, no queue) and "tracker configured but unavailable" (queue, warn).

---
