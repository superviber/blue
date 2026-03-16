# Macaron -- Round 1

## Engaging with Open Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]
The round-0 discussion reveals two camps talking past each other. Muffin, Eclair, and Strudel want strict git-first with Jira as write-only projection. Brioche correctly warns that making PMs second-class citizens kills adoption. Here is my refined position:

Blue should implement a **drift-detection layer**, not a conflict-resolution layer. On every `blue sync`, Blue compares Jira field values against the git-authoritative state and emits a structured drift report (`blue sync --dry-run` outputs a diff). Git always wins on write, but the drift report is surfaced as a PR comment or CLI warning so PMs and engineers see what Jira-side edits will be overwritten. This avoids bidirectional sync complexity while giving PMs visibility into *why* their Jira edits disappeared.

The key design constraint: drift detection must be field-scoped. Status and Epic membership are git-authoritative (Blue overwrites Jira). But fields that Blue never writes (assignee, sprint, story points, custom fields) are Jira-owned and Blue must never touch them. This field-ownership map should be declared in the project-management repo manifest (`jira.toml` or `sync-policy.toml`), not hardcoded.

[PERSPECTIVE P04: Field-ownership map as the T01 resolution mechanism]
The project-management repo should declare a `sync-policy.toml` that partitions Jira fields into three categories: (1) git-authoritative (status, summary, epic-link) -- Blue overwrites on sync; (2) jira-owned (assignee, sprint, story-points) -- Blue never writes; (3) drift-warned (priority, labels) -- Blue warns but does not overwrite. This is reviewable, versionable, and lets each team calibrate the boundary between engineering-managed and PM-managed fields. It resolves T01 without bidirectional sync and without making PMs invisible.

### T02: jira-cli dependency model

[CONCESSION on T02]
I concede to Cupcake and Beignet. My round-0 position assumed the project-management repo was the only integration surface, but the dependency model affects every repo that runs `blue sync`. Hard-bundling jira-cli is wrong. Blue should define an `IssueTracker` trait (as Muffin proposed) with a `JiraCliAdapter` implementation that shells out to `jira` if present on `$PATH`, and a `JiraRestAdapter` as fallback using direct Atlassian REST v3. The trait interface should be minimal: `create_issue`, `transition_issue`, `get_issue`, `list_issues_by_epic`. Blue ships both adapters; users pick via config. This keeps Blue's release cycle decoupled from jira-cli while still supporting it as the preferred UX.

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]
Building on my round-0 P03 (trust-boundary-aware convention) and Croissant's multi-domain point and Cannoli's multi-instance concern:

Token storage must be domain-keyed and role-aware. Concrete proposal:

1. **Local credential store**: `~/.config/blue/jira-credentials.toml`, keyed by Atlassian domain (e.g., `[myorg.atlassian.net]`), containing `email` and `token_ref` (a keychain reference or env var name, never the raw token).
2. **CI credential store**: Tokens injected via environment variable (`BLUE_JIRA_TOKEN_<DOMAIN_SLUG>`), looked up by the domain declared in the project-management repo's `jira.toml`.
3. **Health check**: `blue jira auth status` validates token liveness against Jira's `/rest/api/3/myself` endpoint and reports expiry/scope. This resolves Scone's concern about token lifecycle being treated as a setup step.
4. **Bot account convention**: For CI sync, the guide mandates a dedicated bot account with project-scoped permissions (Brioche's position). Personal tokens are for local dev only.

This design handles Cannoli's multi-instance case (consultants with three orgs) and Brioche's least-privilege requirement without mixing trust boundaries.

### T04: Multi-repo fan-out and atomic consistency

[PERSPECTIVE P05: Per-entity files eliminate the coordination bottleneck]
Cupcake proposed append-only per-entity files to avoid merge conflicts. I want to sharpen this. The project-management repo should use one file per RFC binding (`mappings/rfc-0042.yaml`) and one file per Epic (`epics/2026-q2-auth-overhaul.yaml`). Blue's `sync` command reads the Epic file to discover member RFCs, and each repo writes only its own RFC mapping file. There is no shared registry file. This means concurrent pushes from repo A and repo B touch different files and never conflict. The Epic file is only written by the team lead or via `blue release create`, not by per-repo automation.

This resolves Beignet's T01 (coordination bottleneck) and Donut's T01 (multi-repo fan-out) simultaneously. Strudel's immutable UUID join key still applies -- each `mappings/rfc-0042.yaml` file contains the UUID, the Jira issue key, and the source repo.

### T05: Epic/Feature Release cardinality across repos

[REFINEMENT on T05 / former Macaron T01]
In round 0 I raised the tension of who creates Epics first (Jira or manifest). After reading Donut's lifecycle-gate proposal and Cannoli's cardinality-trap warning, my refined position:

Epics are created git-first, always. `blue release create` writes the Epic file to the project-management repo and pushes the Epic to Jira. The Epic file declares which repos and RFC IDs are members. An Epic can span multiple repos (this is the whole point of the project-management repo), but each RFC belongs to exactly one Epic. Cannoli's escape hatch (`jira_epic_key` override) is necessary for teams whose existing Epics do not follow Blue's naming. Donut's lifecycle gates (no empty Epics, no premature closure) should be enforced by `blue lint` on the project-management repo, not by Jira workflow restrictions, because Jira-side enforcement requires Jira admin access that Blue cannot assume.

### T06: Bootstrapping from existing Jira state

[RESOLVED -- accepting Churro's proposal with one amendment]
Churro's `blue jira import` is correct and necessary. The command should: (1) query Jira for all Epics and Tasks in the target project, (2) generate Epic files and RFC mapping stubs in the project-management repo, (3) create skeleton RFC files in each member repo (or flag them as `imported: true, stub: true` for manual authoring later). My amendment: the import must be idempotent and incremental -- running it twice should not duplicate entries, and running it after partial adoption should only import Jira artifacts that lack a UUID binding. This aligns with the idempotent sync principle everyone agreed on in round 0.

### T07: Convention enforcement scope and progressiveness

[RESOLVED -- accepting Churro's progressive model]
Churro's tiered enforcement (warn-only during onboarding, strict after opt-in) is the right answer. Implementation: `blue.toml` in the project-management repo carries `[conventions] enforcement = "warn" | "strict"`. Default is `"warn"`. `blue lint --jira` respects this flag. The transition to strict should require an explicit PR to the project-management repo, making it a team decision with an audit trail.

Croissant's T02 (how far Blue enforces Jira-side conventions) is resolved by the field-ownership map: Blue only validates fields it owns. Jira-side workflow restrictions are documented as recommended setup, not enforced by Blue.

---

## New Tension

[TENSION T08: Drift detection cost -- API rate limits and latency]
The drift-detection layer I propose for T01 requires Blue to read Jira state on every `blue sync` to compare against git. Jira Cloud's REST API has rate limits (undocumented but real, typically ~100 req/s per token) and latency (~200-500ms per call). For a project with 200 RFCs mapped to Tasks, a full drift check could take 40-100 seconds and consume a meaningful portion of the rate budget. Blue needs a caching or incremental strategy -- perhaps only checking issues modified since last sync (via Jira's `updatedDate` JQL filter) -- to keep `blue sync` fast and rate-limit-safe.

---

## Summary

| Marker | ID | Detail |
|--------|----|--------|
| REFINEMENT | T01 | Field-ownership map in sync-policy.toml; git wins on owned fields, Jira keeps unowned fields, drift-warned for middle ground |
| CONCESSION | T02 | Accept IssueTracker trait with JiraCliAdapter + JiraRestAdapter; no hard bundle |
| REFINEMENT | T03 | Domain-keyed credential store with keychain refs, CI env var convention, health check command, bot account mandate |
| PERSPECTIVE | T04 | Per-entity files (one per RFC mapping, one per Epic) eliminate merge conflicts across repos |
| REFINEMENT | T05 | Epics created git-first via `blue release create`; one RFC to one Epic; escape hatch for legacy Epics |
| RESOLVED | T06 | Accept Churro's `blue jira import` with idempotent incremental amendment |
| RESOLVED | T07 | Accept progressive enforcement via `enforcement = "warn" | "strict"` in project-management repo config |
| TENSION | T08 | Drift detection API rate limits and latency need incremental/cached strategy |

**Moves**: 2 RESOLVED, 3 REFINEMENT, 1 CONCESSION, 1 new PERSPECTIVE, 1 new TENSION

**Claim**: The field-ownership map (`sync-policy.toml`) is the linchpin that resolves T01 without bidirectional sync, and per-entity files resolve the multi-repo coordination tensions (T04/T05). The remaining architectural risk is drift-detection performance (T08), which is solvable with incremental JQL queries.
