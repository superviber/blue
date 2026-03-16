# Beignet -- Round 1

## Engaging with Open Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]

Round 0 produced near-unanimous agreement that git is authority and Jira is projection. The real tension is not technical direction but organizational power: PMs who live in Jira will edit it, full stop. Strudel's "detect and flag as drift" and Muffin's "warn on divergence" both dodge the question of what happens *after* detection.

I am refining my position: Blue should overwrite Jira on every sync, unconditionally. No reconciliation, no prompts, no merge. If a PM edits a Jira field that Blue manages (status, epic linkage, summary), Blue stomps it on the next `blue sync`. The PM repo is canonical; anything else is advisory graffiti. This is the only model that avoids building a distributed consensus engine into a CLI tool.

The organizational cost is real -- PMs lose direct editing power on managed fields. The mitigation is field scoping: Blue should declare which Jira fields it manages (status, summary, epic link, labels) and leave everything else (assignee, priority, sprint, story points, comments) untouched. PMs keep their operational fields; Blue keeps its structural fields. This is a partition, not a hierarchy.

### T02: jira-cli dependency model

[RESOLVED on T02]

Round 0 converged hard on this. Cupcake, Cannoli, and I all independently arrived at "recommended adapter, not hard dependency." Muffin's IssueTracker trait formalization is the right shape. The remaining question is whether the trait's first implementation shells out to `jira-cli` or calls the Jira REST API directly.

I concede ground here: shelling out to `jira-cli` is simpler for a first cut, but Blue should own the HTTP calls long-term. The trait interface lets us start with the shell-out and migrate without touching consumers. This tension is resolved by the trait pattern -- the dependency model becomes an implementation detail behind the interface.

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]

Brioche's "dedicated bot account" and Cannoli's "multi-domain keyed credentials" are both correct but apply to different contexts. Cannoli identified the real constraint: consultants and platform engineers hit multiple Jira instances. Scone's keychain-backed `blue jira auth status` command is the right UX.

My refinement: Blue should not invent a credential store. It should delegate to the OS keychain (macOS Keychain, freedesktop secret-service, Windows Credential Manager) via a library like `keyring`, keyed by `(atlassian-domain, user-email)`. No plaintext files. No env vars as primary path (env vars are acceptable as CI override only). The PM repo declares the target domain in `jira.toml`; Blue looks up the matching credential at runtime. If the credential is missing, `blue sync` fails with a setup instruction, not a silent no-op.

This is simpler than what several experts proposed (Macaron's profile system, Croissant's multi-domain isolation conventions). The OS already solved credential storage. Blue should not re-solve it.

### T04: Multi-repo fan-out and atomic consistency

[PERSPECTIVE P01: Fan-out is a self-inflicted wound from centralizing RFC-to-Task bindings]

Macaron identified the escape hatch in Round 0: RFC-to-Task binding belongs in the RFC's own front matter, not in the PM repo. If each repo owns its own bindings, there is no cross-repo write contention on the PM repo. The PM repo only declares Epic structure and domain membership -- data that changes infrequently and is authored by a single team (platform/PM), not by every repo in the domain.

Donut's and Strudel's concerns about "stale-read race" and "atomic cross-repo state transitions" dissolve under this model. There are no concurrent writes to a shared manifest because each repo writes to itself. The PM repo's sync job reads from all member repos -- it is a reader, not a write target for RFC state.

This does not fully resolve T04 because Epic-level state (release readiness, aggregated status) still lives in the PM repo and could be updated by multiple sync processes. But that surface area is tiny compared to per-RFC bindings.

### T05: Epic/Feature Release cardinality across repos

[TENSION T01: Epic creation authority remains unresolved]

Macaron's T01 from Round 0 nailed this: who creates Epics first -- a human in Jira or a PR to the PM repo manifest? Cannoli raised the cardinality trap (one Epic per release vs. teams' existing Epic semantics).

I do not think this is resolvable by convention alone. Different organizations use Epics differently. Blue should enforce exactly one rule: an Epic declared in the PM repo manifest maps to exactly one Jira Epic, identified by a UUID minted at manifest creation time (per Strudel's join-key proposal). Beyond that, Blue should not impose cardinality constraints. If a team wants one Epic per repo per quarter, fine. If they want one Epic spanning five repos for a release, fine. The manifest declares it; Blue creates or binds it.

The creation direction should be PM-repo-first by default (`blue release create` mints the Epic), with an import path for existing Epics (`blue jira import --epic PROJ-42` writes the binding into the manifest). Churro's bootstrapping concern feeds directly into this.

### T06: Bootstrapping from existing Jira state

[CONCESSION on T06]

Churro is right and I did not address this in Round 0. A greenfield-only integration is dead on arrival. `blue jira import` is essential. I concede that this is a first-class command, not a migration afterthought.

However, import must be scoped and explicit. `blue jira import --project PROJ` should create RFC stubs from existing Jira Tasks and Epic bindings from existing Jira Epics, writing them into the PM repo manifest. It should not attempt to import sprint state, story points, or workflow customizations. The import is a one-time bootstrap, not an ongoing pull. After import, git becomes authority and Jira becomes projection.

### T07: Convention enforcement scope and progressiveness

[REFINEMENT on T07]

Churro's progressive enforcement (warn then enforce) is pragmatic. Croissant's question about repo-side vs. Jira-side enforcement scope is the real issue.

My position: Blue enforces conventions only on its own artifacts. It validates RFC front matter, PM repo manifest structure, and the mapping schema. It does not attempt to enforce Jira-side conventions (workflow configurations, required fields, screen schemes). That is Jira admin territory. If the Jira project is misconfigured (e.g., missing a required custom field for Blue's UUID), `blue sync` should fail with a diagnostic message pointing at the Jira admin setup guide, not attempt to auto-configure Jira.

Progressive enforcement applies to Blue-side conventions: warn-only for the first N syncs or until an explicit `strict: true` in config. This respects onboarding friction without giving up long-term rigor.

---

## Summary

| Marker | Tension | Move |
|--------|---------|------|
| REFINEMENT | T01 (sync direction) | Unconditional overwrite on managed fields; partition managed vs. unmanaged fields |
| RESOLVED | T02 (jira-cli dependency) | IssueTracker trait resolves this; implementation detail behind interface |
| REFINEMENT | T03 (token storage) | Delegate to OS keychain, keyed by (domain, email); no custom credential store |
| PERSPECTIVE | T04 (multi-repo fan-out) | Dissolves if RFC-to-Task bindings are repo-local per Macaron's proposal |
| TENSION | T05 (Epic cardinality) | Epic creation authority still unresolved; propose PM-repo-first with import escape hatch |
| CONCESSION | T06 (bootstrapping) | `blue jira import` is first-class, scoped, one-time bootstrap |
| REFINEMENT | T07 (convention enforcement) | Blue enforces only its own artifacts; progressive warn-then-strict for onboarding |

**Claim**: The integration surface is smaller than Round 0 suggested. If we commit to (1) unconditional overwrite on managed fields, (2) repo-local RFC-to-Task bindings, and (3) OS keychain delegation, three of the seven tensions collapse and two more shrink to narrow scope.

---
