# Cannoli -- Round 1

## Addressing Open Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]

In Round 0 I flagged that push-only sync leaves PMs who edit Jira directly in the cold. After reading Strudel, Eclair, and Muffin's positions, I'm refining my stance: **git must win for structural state (RFC status, Epic membership, task-to-RFC bindings), but Jira should be permitted to own ephemeral operational fields (assignee, sprint membership, story points)**. These ephemeral fields have no canonical representation in Blue today and never will -- they are Jira-native concerns that PMs legitimately manage in the Jira UI.

Concrete proposal:

- `blue sync` pushes structural state (status, Epic linkage, labels) from the project-management repo to Jira, overwriting Jira-side values.
- `blue sync` does NOT touch assignee, sprint, story points, or any Jira custom field not declared in the project-management repo schema.
- If a PM changes a *structural* field in Jira directly, `blue sync --check` reports drift as a warning. `blue sync` then overwrites it. No merge. No prompt. Git wins.
- The project-management repo schema (per Scone's P01 and Eclair's P02) explicitly declares which fields are "projected" (git-owned) vs "Jira-native" (Jira-owned). This is the escape hatch that prevents organizational friction.

This resolves Brioche's P03 concern about split-brain by making the split explicit and field-scoped rather than system-scoped. PMs still use Jira for sprint planning. Blue still owns the lifecycle.

[PERSPECTIVE P01: Field-ownership partitioning eliminates the bidirectional sync trap]
The real problem behind T01 is not "which system wins" but "which system owns which fields." By partitioning field ownership declaratively in the project-management repo schema, Blue avoids bidirectional sync entirely. Structural fields flow git-to-Jira. Operational fields stay in Jira. Neither system overwrites the other's owned fields. This is the pattern Terraform uses for `lifecycle { ignore_changes }` and it works because it makes the boundary explicit and auditable.

### T02: jira-cli dependency model

[RESOLVED on T02]

Round 0 achieved strong convergence here: Muffin proposed an `IssueTracker` trait, Cupcake proposed jira-cli as a recommended adapter behind a provider interface, Beignet explicitly warned against hard bundling. I agree with this consensus and consider T02 resolvable now.

The resolution: Blue defines an `IssueTracker` trait (mirroring the `Forge` pattern). The default implementation shells out to `jira-cli` if present on `$PATH`. If absent, Blue degrades gracefully -- RFC creation works, sync commands emit a warning ("Jira sync skipped: no issue tracker configured"), and `blue lint` can optionally flag the absence as a warning (not an error). A mock implementation ships for CI/testing.

The key product constraint I want to add: `blue jira setup` must validate that jira-cli is installed, authenticated, and can reach the target Jira instance *before* writing any config to the project-management repo. Fail fast, not at sync time.

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]

Brioche's bot-account recommendation and Croissant's multi-domain credential keying are both correct and complementary. I'm synthesizing into a concrete credential model:

1. **Local-only, domain-keyed credential store**: `~/.config/blue/jira-credentials.toml` keyed by Atlassian domain (e.g., `myorg.atlassian.net`). Each entry holds user email + API token reference. Blue selects credentials by matching the domain declared in the project-management repo's `jira.toml`.

2. **OS keychain preferred, env var fallback**: Token values should be stored in the OS keychain (macOS Keychain, Linux secret-service) with the TOML file holding only the keychain reference. For CI, `BLUE_JIRA_TOKEN` env var is the documented fallback. Plaintext in TOML is a last resort with a `blue lint` warning.

3. **Bot account for CI, personal tokens for local dev**: The project-management repo's `jira.toml` declares the domain and project key but never credentials. CI pipelines use a dedicated bot account. Local dev uses personal tokens. Blue does not conflate these trust boundaries.

4. **`blue jira auth status`**: A command that reports token health per domain -- valid, expiring soon, expired, missing. Surfaces in `blue health-check`.

This addresses Scone's point about token lifecycle being a governance problem. The rotation story: when a token expires, `blue sync` fails with a clear message directing the user to `blue jira auth refresh`, which re-prompts for the token and updates the keychain entry. No silent degradation.

### T04: Multi-repo fan-out and atomic consistency

[PERSPECTIVE P02: Macaron's repo-local binding solves the fan-out write contention problem]

Macaron's Round 0 insight is the key unlock here: RFC-to-Task bindings live in each repo's RFC front matter, NOT in the central project-management repo. The project-management repo only declares Epic-level structure and domain membership. This means multiple repos never compete to write to the same manifest file.

The remaining coordination problem is Epic lifecycle transitions (when does an Epic move to "Done"?). This requires the project-management repo to *read* RFC statuses from member repos during `blue sync`, not to be the write target. The sync command in the project-management repo aggregates state; it does not receive concurrent writes.

Donut's concern about stale-read races is valid but manageable: `blue sync` in the project-management repo should `git pull --rebase` each member repo before aggregating, and the aggregated state is committed as a single atomic commit. If a member repo changes between pull and commit, the next sync run self-corrects because the operation is idempotent.

### T05: Epic/Feature Release cardinality across repos

[REFINEMENT on T05]

From my Round 0 P03, I flagged the cardinality trap between Feature Releases and Epics. After reading Macaron and Donut, I'm refining:

- **One Feature Release maps to exactly one Epic**. This is the default, enforced by schema validation.
- **One Epic can span multiple repos**. The project-management repo declares the Epic and lists its member repos. Each repo contributes RFCs to the Epic via front matter bindings.
- **`jira_epic_key` override**: If a team's existing Epic semantics differ (per-quarter, per-team), the project-management repo manifest allows an explicit `jira_epic_key` that maps a Feature Release to a pre-existing Epic. This is the escape hatch. Schema validation warns but does not block.
- **Epic creation direction**: Epics are always created git-first. `blue sync` creates the Epic in Jira if it doesn't exist, using the Feature Release metadata from the project-management repo. If a team wants to bind to a pre-existing Jira Epic, they use the override key. This resolves Macaron's T01 about who creates Epics first.

### T06: Bootstrapping from existing Jira state

[CONCESSION on T06]

Churro is right and Round 0 underweighted this. Most real-world teams will not be greenfield. A `blue jira import` command is not a nice-to-have -- it is a hard adoption requirement.

However, import should be scoped and safe:

- `blue jira import --project PROJ --epic EPIC-123` imports a single Epic and its child issues as RFC stubs in the project-management repo.
- Imported RFCs get `status: imported` (a new lifecycle state) and require explicit `blue rfc adopt` to enter the normal lifecycle.
- The import is additive only -- it never modifies existing project-management repo state.
- Strudel's UUID join key is minted at import time, written into both the RFC stub and the Jira issue's custom field, establishing the durable binding.

This is Phase 2 work (after the core sync loop is stable), but it must be in the RFC scope from day one so the schema and identity model accommodate it.

### T07: Convention enforcement scope and progressiveness

[PERSPECTIVE P03: Progressive enforcement is a product requirement, not a technical preference]

Churro's Round 0 P02 is correct: strict enforcement from day one will cause teams to route around Blue. But Croissant's concern about scope is also valid -- Blue cannot enforce Jira-side workflow rules.

Resolution:

- **Blue-side enforcement** (RFC schema, Epic naming, required front matter fields): progressive. `warn` mode by default, `strict` mode opt-in via `blue.toml` config flag. First N syncs (configurable, default 10) run in warn mode regardless of config.
- **Jira-side validation** (workflow transitions, required fields, allowed statuses): Blue validates what it can via API before pushing, but does NOT attempt to configure Jira workflows. `blue jira setup` documents the expected Jira project configuration and offers a `--validate` flag that checks whether the Jira project's workflow matches Blue's expectations.
- **Schema-as-contract** (per Eclair P02 and Scone P01): The project-management repo ships a JSON Schema that defines valid mappings. `blue lint` validates against it. This is the enforcement mechanism -- reviewable, versionable, evolvable via PR.

---

## Summary

| Marker | Count |
|--------|-------|
| PERSPECTIVE | 3 (P01: field-ownership partitioning, P02: repo-local binding solves fan-out, P03: progressive enforcement) |
| TENSION | 0 new |
| REFINEMENT | 3 (T01 sync direction, T03 token model, T05 Epic cardinality) |
| CONCESSION | 1 (T06 bootstrapping is a hard requirement) |
| RESOLVED | 1 (T02 dependency model) |

**Claim**: 5 of 7 tensions (T01, T02, T03, T05, T07) have actionable resolution proposals. T04 is addressed by Macaron's repo-local binding pattern. T06 is acknowledged as a hard requirement with a scoped import design. Remaining work is validating these proposals against implementation cost in Round 2.
