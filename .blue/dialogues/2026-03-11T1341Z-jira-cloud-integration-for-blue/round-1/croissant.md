# Round 1 -- Croissant (Convention & Standards Designer)

## Engaging with Round 0 Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]

Round 0 established near-unanimous agreement that git is ground truth, but the unresolved question is what happens when PMs edit Jira directly. I refine my Round 0 position: Blue should define **three explicit drift policies** as a convention, selectable per-domain in the project-management repo manifest:

1. **`drift: overwrite`** -- `blue sync` silently overwrites Jira with git state. Suitable for teams where PMs commit to using Blue exclusively.
2. **`drift: warn`** -- `blue sync` detects divergence, logs it, and continues pushing git state. A drift report is written to `.blue/jira/drift-report.yaml` for human review.
3. **`drift: block`** -- `blue sync` refuses to push if Jira state has diverged from the last known projection. Requires explicit `blue sync --force` or manual reconciliation.

The default should be `drift: warn` -- progressive, not gatekeeping (aligning with Churro's P02). The key convention: **drift detection compares the last-pushed projection state** (stored in a `.blue/jira/last-sync.yaml` per-entity) against current Jira state, not against git state directly. This avoids needing to model Jira's full state machine and keeps the comparison scoped to fields Blue owns.

Strudel's UUID join key (P02) is essential here -- drift detection must join on an immutable identifier, not Jira issue keys. I concur fully.

Brioche raised the concern that non-engineering stakeholders become second-class citizens under git-first. The drift policy convention addresses this: teams with active PM Jira usage can choose `drift: warn` to preserve visibility into Jira-side changes without losing git authority. The convention makes the trade-off explicit rather than implicit.

[RESOLVED on T01 -- contingent on drift policy convention being adopted]

---

### T02: jira-cli dependency model

[CONCESSION on T02]

In Round 0 I focused on schema enforcement and did not engage T02 directly. After reading Cupcake's adapter proposal (P01), Muffin's IssueTracker trait (P02), Beignet's "shell out if present" pattern (P02), and Cannoli's concern about jira-cli coupling (T02), I concede that **the trait interface approach is the correct convention**.

The convention I propose:

- Blue defines an `IssueTracker` trait (Muffin's design) with operations: `create_issue`, `transition_issue`, `link_issue`, `get_issue_status` (read-only, for drift detection only).
- The `JiraCloudTracker` implementation shells out to `jira-cli` if available, with graceful degradation (Beignet's pattern).
- Blue ships **no bundled binary**. `blue jira auth check` verifies that `jira` is on PATH and authenticated.
- The project-management repo's `jira.toml` declares the provider type (`provider = "jira-cloud"`), so future Linear/Shortcut providers are convention-compatible without schema changes.

This resolves T02 by making the dependency a convention ("install jira-cli to enable Jira features") rather than a coupling ("Blue requires jira-cli").

[RESOLVED on T02]

---

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]

Round 0 surfaced strong agreement on several points: tokens never in git (Brioche, Strudel, Macaron, myself), OS keychain preferred (Eclair P02), multi-domain keying needed (myself P03, Cannoli P02). I refine with a concrete convention:

**Credential hierarchy** (checked in order):
1. Environment variable `BLUE_JIRA_TOKEN_{DOMAIN_SLUG}` (CI-friendly, domain-scoped)
2. OS keychain entry keyed by `blue:jira:{domain}` (interactive use)
3. `~/.config/blue/jira-credentials.toml` with domain-keyed sections (fallback for systems without keychain support)

**Convention constraints:**
- `blue lint` checks staged files for patterns matching Atlassian API tokens (regex: `[A-Za-z0-9]{24}` adjacent to `atlassian` or `jira` context).
- The project-management repo template ships with `.gitignore` entries: `*.credentials.toml`, `.jira-cli/`, `*.secret`.
- `blue jira auth status` reports per-domain token health (valid / expired / missing) without printing the token itself.
- Bot account recommendation (Brioche P02) is documented as a convention, not enforced -- enforcement would require querying Jira's user type, which is outside Blue's reasonable scope.

**Multi-domain support:** The `jira.toml` in each project-management repo declares `domain = "myorg.atlassian.net"`, and Blue routes to the matching credential entry. No global default domain -- explicit declaration per repo.

[RESOLVED on T03 -- contingent on credential hierarchy and lint convention being adopted]

---

### T04: Multi-repo fan-out and atomic consistency

[PERSPECTIVE P04: Per-entity files eliminate the coordination bottleneck]

Cupcake (P02) identified that concurrent PRs to shared manifest files cause merge conflicts. Macaron (P02) proposed that RFC-to-Task bindings live repo-local in RFC front matter, with the PM repo only declaring Epic structure. I strongly endorse this and formalize the convention:

**File structure convention for the project-management repo:**
```
domains/{domain-name}/
  domain.yaml          # repo membership, Jira project key
  epics/
    {epic-id}.yaml     # one file per Epic -- name, Jira key, status
  releases/
    {release-id}.yaml  # Feature Release grouping Epic references
```

**RFC-to-Task binding convention (repo-local):**
```yaml
# In RFC front matter within each code repo
jira:
  task_key: PROJ-142        # mutable, updated by blue sync
  blue_uuid: a1b2c3d4-...   # immutable, minted at creation
```

The PM repo never contains per-RFC entries. It only declares Epic-level structure. This means multiple repos can create RFCs concurrently without ever writing to the same PM repo file. The only shared write target is Epic files, which change infrequently (Epic creation/closure, not per-RFC updates).

Donut's concern (T01) about cross-repo write dependencies is addressed: repos write to themselves, not to the PM repo, for RFC-level state. Beignet's concern about stale-read races (T01) is addressed: the PM repo's Epic files are rarely mutated, and when they are, it is a deliberate human action (creating a new Epic), not automated fan-out.

[TENSION T04-R1: Epic creation still requires a PM repo write]
When a new Epic is needed, some agent (human or CI) must create the `epics/{epic-id}.yaml` file in the PM repo. If two repos simultaneously trigger Epic creation for the same Feature Release, a merge conflict is still possible on this single file. Convention: Epic creation is always a human-initiated PR to the PM repo, never automated by `blue sync`. This keeps the write surface narrow.

---

### T05: Epic/Feature Release cardinality across repos

[PERSPECTIVE P05: One Feature Release = exactly one Epic, with explicit override escape hatch]

Cannoli (P03) identified the cardinality trap. I formalize the convention:

- Default: one Feature Release in the PM repo maps to exactly one Jira Epic. The `epic-id` in the release YAML is the join key.
- Override: teams whose Epic semantics differ (per-team, per-quarter) can set `jira_epic_key: PROJ-42` directly in the release YAML, bypassing Blue's naming convention for that specific release.
- `blue lint` warns when a Feature Release has zero linked RFCs (orphaned Epic) or when an Epic key is shared across multiple Feature Releases (ambiguous cardinality).

This respects Cannoli's observation that orgs use Epics differently while keeping the default convention clean.

---

### T06: Bootstrapping from existing Jira state

[CONCESSION on T06]

Churro (P01) is correct that greenfield assumptions kill adoption. I concede that the convention framework must include an import path. However, the convention constraint is: **`blue jira import` creates PM repo artifacts from Jira state, but the moment import completes, git becomes ground truth.** The import is a one-time bootstrap, not an ongoing sync direction.

Convention for the import command:
- `blue jira import --project PROJ --domain myorg.atlassian.net` scans Jira Epics and Tasks, creates `epics/*.yaml` files and RFC stubs with `jira.task_key` bindings.
- Imported RFCs are created with status `imported` (a new lifecycle state) until a human reviews and promotes them to `draft` or `active`.
- The import writes a `bootstrap-manifest.yaml` recording what was imported and when, so `blue sync` can distinguish imported-but-unreviewed artifacts from Blue-native ones.

---

### T07: Convention enforcement scope and progressiveness

[RESOLVED on T07]

Churro's progressive enforcement (P02) and my Round 0 concern (T02) are reconciled by defining enforcement tiers as a convention:

**Tier 1 -- Always enforced (repo-side):**
- RFC front matter must include `jira.blue_uuid` before `blue rfc complete` succeeds
- No credentials in staged files (`blue lint`)
- PM repo schema validation (Epic files must match declared schema)

**Tier 2 -- Warn-then-enforce (configurable):**
- Epic naming conventions (warn for 30 days / N syncs, then enforce)
- Label taxonomy compliance
- Status transition ordering (warn if Jira status skips expected transitions)

**Tier 3 -- Document-only (Jira-side):**
- Jira workflow configuration (required fields, allowed transitions)
- Jira project permission schemes
- Custom field setup for `blue_uuid`

The tier is declared per convention in a `conventions.toml` in the PM repo. Teams opt into stricter enforcement as they mature. Tier 3 is explicitly outside Blue's enforcement scope -- Blue documents the expected Jira setup but never attempts to configure Jira programmatically.

[RESOLVED on T07]

---

## Summary

| Marker | Tension | Move |
|--------|---------|------|
| RESOLVED | T01 (Sync direction) | Drift policy convention with three modes; default `warn` |
| RESOLVED | T02 (jira-cli dependency) | IssueTracker trait, shell-out pattern, no bundled binary |
| RESOLVED | T03 (Token storage) | Three-tier credential hierarchy, domain-keyed, lint enforcement |
| PERSPECTIVE | T04 (Multi-repo fan-out) | Per-entity files + repo-local RFC bindings eliminate shared writes |
| PERSPECTIVE | T05 (Epic cardinality) | 1:1 default with explicit override escape hatch |
| CONCESSION | T06 (Bootstrapping) | Import command as one-time bootstrap, git authority post-import |
| RESOLVED | T07 (Enforcement scope) | Three-tier progressive enforcement with PM repo config |

**New tension raised:** T04-R1 (Epic creation write coordination in PM repo).

**Claim:** The convention framework proposed here -- drift policies, credential hierarchy, per-entity file structure, progressive enforcement tiers, and IssueTracker trait contract -- provides the standards backbone for Blue's Jira integration. Every convention is declared in repo-local config files (not hardcoded), making them reviewable, evolvable, and auditable via git blame. The remaining work is implementation sequencing, not architectural disagreement.

ALIGNMENT: 78
