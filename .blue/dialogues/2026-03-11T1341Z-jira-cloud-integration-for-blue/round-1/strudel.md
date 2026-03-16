# Strudel -- Round 1

## Addressing Open Tensions

### T01: Sync direction -- git-first vs Jira-side edits by PMs

[REFINEMENT on T01]

Round 0 established near-unanimous agreement that git is the sole authority. The remaining tension is organizational, not architectural: PMs will edit Jira. My Round 0 position (P01) was that Jira-side edits should be detected and flagged as drift. I refine this into a concrete mechanism:

Blue should implement a **drift detection digest** as part of `blue sync`. On every sync run, Blue reads the current Jira state for each bound issue (status, assignee, priority -- the fields Blue projects) and compares against the git repo's declared state. Divergences are emitted as a structured report (`blue sync --drift-report`) with three severity levels:

1. **Cosmetic drift** (Jira description edited, comments added): Ignored. Jira is the collaboration surface for discussion; Blue does not own prose.
2. **Status drift** (Jira status moved ahead of or behind git state): Warning emitted. Git state wins on next sync push. The overwritten Jira state is logged in a `.blue/jira/drift-log.yaml` for auditability.
3. **Structural drift** (issue moved to different Epic, reparented, or deleted in Jira): Error. `blue sync` refuses to proceed until the PM repo is updated to reflect the intended structure, or the Jira change is reverted.

This preserves git-as-authority while giving PMs a clear, auditable signal that their Jira-side edits on structural/status fields will be overwritten. It addresses Brioche's concern (P03 in Round 0) about reconciliation being security-relevant: the drift log creates an audit trail.

I concur with Muffin's position that Jira-side status changes should never flow back into Blue's local state. The drift report is read-only observation, not bidirectional sync.

[RESOLVED on T01 -- proposing resolution]

**Proposed resolution**: Git always wins for status and structure. Blue detects drift per the three-tier model above. Cosmetic Jira edits are out of scope. Drift is logged, never silently absorbed. This gives PMs visibility without introducing bidirectional sync complexity.

---

### T02: jira-cli dependency model

[CONCESSION on T02]

In Round 0, I did not directly address the dependency model. After reading Cupcake's position (jira-cli as recommended adapter, not hard dependency), Cannoli's position (shell out to `jira` if present, degrade gracefully), and Muffin's IssueTracker trait proposal, I concede that the trait-based approach is the correct architecture for data integrity reasons:

A hard jira-cli dependency means Blue's data pipeline (RFC-to-Jira projection) is coupled to a single provider's binary stability. If jira-cli introduces a breaking change to its output format or auth flow, Blue's sync becomes unreliable, and unreliable sync is worse than no sync -- it creates phantom state where Blue believes it has projected to Jira but the projection is corrupt.

The IssueTracker trait should define a **verified projection contract**: after any `create_issue` or `transition_issue` call, the trait implementation must return a confirmation payload that Blue can validate (e.g., the created issue key, the resulting status). This verification step is missing from Muffin's original proposal and is essential for data integrity -- without it, Blue cannot distinguish between a successful projection and a silent failure.

[PERSPECTIVE P01: The IssueTracker trait must include a verification callback to confirm projection integrity]

Every IssueTracker implementation must expose a `verify_projection(entity_id: UUID, expected_state: ProjectedState) -> VerificationResult` method. After Blue pushes state to Jira, it calls verify to confirm the projection landed correctly. Failed verifications are recorded in the drift log and surfaced on the next `blue sync` or `blue status`. This catches silent API failures, rate-limit truncations, and auth expiry mid-sync -- all of which produce corrupt projections that are invisible without explicit verification.

---

### T03: Token storage, scoping, and rotation

[REFINEMENT on T03]

My Round 0 position (P03) was that tokens must never touch the git repo. I align with Brioche's dedicated bot account recommendation and Macaron's multi-domain credential keying. I refine with a concrete data integrity constraint:

**Token validity must be verified before any projection operation, not after failure.** Blue should run a lightweight auth check (`GET /rest/api/3/myself` equivalent) at the start of every `blue sync` invocation. If the token is invalid, expired, or lacks the required project permissions, Blue must fail fast with an actionable error message before it begins mutating Jira state. Half-completed sync runs (where auth expires mid-batch) are the most dangerous data integrity failure mode because they leave Jira in a partially-projected state that does not match either the git repo's intent or Jira's prior state.

[PERSPECTIVE P02: Credential validation must be a pre-flight check, not a post-failure discovery]

The credential store (OS keychain or `~/.config/blue/jira/credentials.toml`, keyed by Atlassian domain per Macaron's proposal) must support a `blue jira auth check` command and an implicit pre-flight validation in every sync operation. The pre-flight should verify: (a) token exists for the target domain, (b) token is not expired, (c) token has at minimum `write:jira-work` scope on the target project. This prevents the partial-projection failure mode entirely.

---

### T04: Multi-repo fan-out and atomic consistency

[REFINEMENT on T04]

I raised this tension in Round 0 (my T02). Macaron's proposal -- RFC-to-Task binding lives in RFC front matter within each repo, not centralized in the PM repo -- significantly reduces the merge conflict surface. I refine my position:

If RFC-to-Task binding is repo-local (stored in RFC front matter as `jira_task_key: PROJ-142`), then the PM repo only needs to declare Epic-level structure and domain membership. This means concurrent RFC creations across repos never conflict on the PM repo. The remaining atomic consistency risk is Epic-level operations: creating a new Feature Release (Epic) or closing one. These are low-frequency operations that can safely require a sequential PR to the PM repo.

[PERSPECTIVE P03: Repo-local RFC-to-Task binding plus PM-repo-only Epic declarations eliminates fan-out conflicts for the common case]

The fan-out consistency problem is solved by layering authority: each repo owns its own RFC-to-Jira-Task bindings (in front matter), while the PM repo owns only Epic structure and repo membership. Blue's `sync` command reads RFC front matter from member repos (via git clone/fetch of their latest default branch) and reconciles Jira Tasks accordingly. Epic mutations require a PR to the PM repo and are serialized through normal git merge semantics. This is not perfectly atomic, but the failure mode (a brief window where a new RFC's Task exists in Jira but the Epic has not yet been updated) is benign and self-healing on the next sync cycle.

---

### T05: Epic/Feature Release cardinality across repos

[PERSPECTIVE P04: One Epic per Feature Release, with an explicit override escape hatch]

I align with Cannoli's position that Blue should enforce a 1:1 mapping between Feature Release and Jira Epic as the default, but provide an `epic_key` override in the PM repo manifest for teams whose Epic semantics differ. From a data integrity standpoint, the 1:1 default is critical because it makes the join unambiguous -- there is exactly one Epic to query for each Feature Release's status. Many-to-many mappings between Feature Releases and Epics would make drift detection intractable: you cannot determine if a Feature Release is "done" if its constituent Tasks are spread across multiple Epics with independent lifecycle states.

---

### T06: Bootstrapping from existing Jira state

[CONCESSION on T06]

Churro is correct that the clean-slate assumption is an adoption killer. I had not addressed this in Round 0. From a data integrity perspective, `blue jira import` is essential but dangerous: it must create RFC stubs with the immutable UUID join key (my Round 0 P02) and bind them to existing Jira issue keys, but it must not assume that the existing Jira state is consistent or well-structured. The import command should run in a dry-run mode first, producing a report of what it would create, and require explicit confirmation before writing to the PM repo. Imported RFCs should carry a `source: jira-import` marker in their front matter so that `blue lint` can apply relaxed validation rules during an onboarding grace period.

---

### T07: Convention enforcement scope and progressiveness

[REFINEMENT on T07]

I align with Churro's progressive enforcement proposal and Croissant's concern about scope. My refinement from a data integrity angle:

Blue should enforce conventions that protect **referential integrity** strictly from day one -- e.g., every RFC must have a valid UUID, every Epic reference must point to a declared Epic in the PM repo, every Jira Task binding must reference a reachable Jira issue. These are not style conventions; they are structural invariants whose violation causes corrupt projections.

Conventions that are **stylistic or organizational** -- naming patterns, label taxonomies, required fields beyond the structural minimum -- should follow Churro's progressive model (warn-only for a configurable grace period, then enforce).

[PERSPECTIVE P05: Distinguish structural integrity conventions (always enforced) from organizational conventions (progressively enforced)]

Structural conventions protect join keys, referential integrity, and projection correctness. They must be enforced unconditionally because relaxing them creates data corruption that is expensive to repair. Organizational conventions (naming, labels, workflow customization) are team preferences that benefit from a grace period. Blue's `jira.toml` configuration should expose an `enforcement_level` setting with values `structural-only` (default for first 30 days or until explicitly changed) and `full`.

---

## Summary

| Marker | Count | Details |
|--------|-------|---------|
| PERSPECTIVE | 5 | P01: Verification callback in IssueTracker trait; P02: Credential pre-flight check; P03: Layered authority for fan-out; P04: 1:1 Epic-to-Feature-Release default; P05: Structural vs organizational convention enforcement |
| TENSION | 0 | No new tensions raised |
| REFINEMENT | 3 | T01 (drift detection tiers), T03 (pre-flight auth), T04 (repo-local binding), T07 (structural vs organizational) |
| CONCESSION | 2 | T02 (trait-based dependency model), T06 (bootstrapping is essential) |
| RESOLVED | 1 | T01 proposed resolution (git always wins, three-tier drift model) |

**Claim**: The data integrity requirements for Jira integration reduce to three invariants: (1) immutable UUID join keys minted at creation time, (2) verified projections with post-write confirmation, and (3) pre-flight credential validation before any mutation. If these three invariants hold, the remaining design decisions (dependency model, Epic cardinality, convention enforcement) have safe defaults that teams can customize without risking data corruption.
