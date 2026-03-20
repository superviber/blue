# RFC 0038: SDLC Workflow Discipline

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-27 |
| **Source Spike** | 2026-01-26T1500Z-formalize-sdlc-workflow-and-release-process |
| **Source Dialogue** | 2026-01-27T0058Z-sdlc-workflow-discipline-rfc |
| **ADRs** | 0009 (Courage), 0011 (Freedom Through Constraint) |

---

## Summary

Enforce SDLC workflow discipline through mechanical gates rather than documented aspirations. PreToolUse hooks block code edits outside worktrees, spike auto-close fires when source RFCs ship, and ADR suggestions surface at implementation boundaries. Deploy all gates simultaneously — they target different workflow moments and don't compound friction.

## Problem

Four workflow gaps persist despite existing tooling:

1. **Work outside worktrees**: `blue_worktree_create` exists but enforcement is soft (warning only). Code gets committed directly to `develop`.
2. **Missing PRs**: `blue_pr_create` exists but nothing requires its use. Work merges without review.
3. **Stale spikes**: Spikes that produce RFCs stay `.wip.md` forever. No backlink from "RFC implemented" to "close source spike."
4. **Absent ADRs**: `blue_adr_relevant` provides AI-powered semantic matching but is never invoked during any workflow step. Architectural decisions go undocumented.

### Root Cause (Dialogue Consensus — 6/6)

Worktree isolation is the root problem. Missing PRs, stale spikes, and absent ADRs are symptoms. Documented process lives in markdown while actual process lives in tool defaults. Soft warnings get ignored. Mechanical enforcement is required.

## Goals

1. Code changes require active worktrees — mechanically enforced, not aspirational
2. Spikes auto-close when their source RFCs reach `implemented`
3. ADR suggestions surface at the right moment without blocking workflow
4. PRs become natural consequences of worktree discipline, not separate gates
5. Connected RFCs across multiple repos can be tracked and coordinated

## Non-Goals

- Release process formalization (separate RFC per existing spike Phase 2)
- Doc-writer agent formalization (separate RFC per spike Phase 3)
- CI/CD pipeline integration
- Branch protection rules at forge level
- Auto-transition of RFC status across repo boundaries (notification only)

## Proposal

### 1. PreToolUse Hooks for Worktree Enforcement

A `PreToolUse` hook intercepts `Write`, `Edit`, and `Bash` (file-writing commands) before execution. The hook calls the compiled `blue guard` command to verify:

1. Is the target file inside a Blue worktree?
2. Does the worktree correspond to an RFC in `accepted` or `in-progress` status?
3. Is the file within the worktree's directory tree?

If any check fails and the file is NOT on the allowlist, the hook **blocks the tool call**.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "blue guard --tool=$TOOL_NAME --path=$INPUT_FILE_PATH"
          }
        ]
      }
    ]
  }
}
```

#### Allowlist (No Worktree Required)

| Path Pattern | Rationale |
|---|---|
| `.blue/docs/**` | Spikes, RFCs, ADRs, dialogues are pre-implementation |
| `.claude/**` | Agent definitions are configuration |
| `*.md` (repo root) | README, CHANGELOG, CONTRIBUTING |
| `/tmp/blue-dialogue/**` | Dialogue agent output files |
| `.gitignore` | Repository configuration |

Everything else — `crates/**`, `src/**`, `tests/**`, `Cargo.toml`, `Cargo.lock` — requires an active worktree.

#### Emergency Bypass

Set `BLUE_BYPASS_WORKTREE=1` for legitimate exceptions (hotfixes, cross-branch investigations). All bypasses are logged to an audit trail file at `.blue/audit/guard-bypass.log` with timestamp, file path, and reason.

### 2. Spike Auto-Close on RFC Implementation

#### Schema Changes

RFC frontmatter gains optional field:
```markdown
| **Source Spike** | 2026-01-26T1500Z-formalize-sdlc-workflow |
```

Spike frontmatter gains optional field:
```markdown
| **Produces RFCs** | 0038 |
```

For multi-RFC spikes:
```markdown
| **Produces RFCs** | 0038, 0039, 0040 |
```

#### Mechanism

When `blue_rfc_update_status` transitions an RFC to `implemented`:

1. Parse `Source Spike` from RFC frontmatter
2. If present, locate the spike file
3. If spike has `Produces RFCs` listing multiple RFCs, check if ALL have reached `implemented` or `rejected`
4. If all resolved: rename spike from `.wip.md` to `.done.md`, append resolution note with RFC IDs and date
5. If not all resolved: emit conversational hint noting partial completion

Manual `.done.md` transition remains available for scope-changed investigations.

### 3. ADR Suggestion at Implementation Boundary

When `blue_rfc_update_status` transitions an RFC to `in-progress` (the implementation boundary):

1. Call `blue_adr_relevant` with the RFC title and problem statement as context
2. If relevant ADRs found with confidence > 0.7, emit conversational hint:
   ```
   "This RFC may relate to ADRs: 0009 (Courage), 0011 (Freedom Through Constraint).
   Consider citing them or documenting new architectural decisions."
   ```
3. If RFC title contains keywords `breaking`, `redesign`, `architectural`, `migration`: additionally hint "This appears to be an architectural decision — consider creating an ADR."

This is a **suggestion only**. It does not block any workflow step. Per RFC 0004: "Guide, don't block."

### 4. PRs as Isolation Boundaries

Worktree enforcement makes branch-based development mandatory. PRs become the natural merge path:

1. `blue_worktree_create` creates a feature branch from `develop`
2. Code changes happen in the worktree (enforced by PreToolUse hooks)
3. When work is complete, `blue_pr_create` creates a PR from the feature branch to `develop`
4. PR serves as isolation verification — confirming work was done in the correct context
5. Zero-reviewer merges are acceptable for AI-only development (PRs verify isolation, not code review)
6. All PRs squash-merge to `develop` (no override)

### 5. Agent Incentive Alignment

PreToolUse hooks make `blue_worktree_create` the ONLY path to code modification. AI agents optimize for task completion; when task completion becomes impossible without a worktree, the agent optimizes FOR worktree creation. This mechanically aligns agent behavior with process compliance without requiring the agent to "understand" or "remember" the rules.

This embodies ADR 0011 (Freedom Through Constraint): the constraint of worktree-only editing enables the freedom of reliable, isolated, reviewable changes.

### 6. Deployment Strategy: All-at-Once

Deploy all four mechanisms simultaneously. Rationale (4-2 dialogue supermajority):

- PreToolUse hooks operate at **write-time**
- Spike auto-close operates at **status-change time**
- ADR suggestions operate at **implementation-boundary time**
- PRs operate at **merge-time**

These are orthogonal workflow moments. Simultaneous deployment distributes friction across independent decision points rather than compounding it. A single behavioral boundary event creates stronger habit formation than gradual tightening.

The comprehensive allowlist ensures documentation workflows remain frictionless while code workflows gain mechanical discipline.

### 7. Realm Coordination for Cross-Repo RFCs

When architectural changes span multiple repositories (a "realm"), connected RFCs need tracking and coordination. The design follows a **federated storage + runtime discovery** model (5-1 dialogue supermajority).

#### Metadata Format

Each repo has `.blue/realm.toml` declaring its outbound dependencies:

```toml
# Realm membership (optional, for discovery)
[realm]
name = "blue-ecosystem"

# This repo's outbound RFC dependencies
[rfc.0038]
depends_on = ["blue-web:0015", "blue-cli:0008"]

[rfc.0040]
depends_on = ["blue-web:0018"]
```

RFC frontmatter gains optional field:
```markdown
| **Realm Dependencies** | blue-web:0015, blue-cli:0008 |
```

#### Qualified RFC Identifiers

Cross-repo references use the format `repo:rfc-number`:
- `blue:0038` — RFC 0038 in the `blue` repository
- `blue-web:0015` — RFC 0015 in the `blue-web` repository

This extends the spike `Produces RFCs` field:
```markdown
| **Produces RFCs** | blue:0038, blue-web:0015, blue-cli:0008 |
```

#### Validation Tool

`blue_rfc_validate_realm [--strict]`:

1. Reads local `.blue/realm.toml`
2. For each dependency, fetches status from target repo:
   - Git clone to `/tmp/blue-realm-cache/<repo>/` (cached, refreshed on query)
   - Or GitHub API for lighter queries
3. Reports status matrix of all connected RFCs
4. Default: **warn** on unresolved dependencies (consuming repo accepts risk)
5. `--strict` or config `strict_realm_validation = true`: **fail** on unresolved

#### Cross-Repo Spike Auto-Close

When spike `Produces RFCs` contains qualified identifiers spanning repos:

1. Parse all qualified RFC IDs from spike frontmatter
2. For each remote repo, query RFC status via cached clone or API
3. Auto-close (`.wip.md` → `.done.md`) only when **ALL** listed RFCs reach `implemented` or `rejected`
4. If partial: emit hint noting which remote RFCs are pending

#### Governance Model

**Initiating repo authority** (5-1 dialogue supermajority):
- The repo that initiates a cross-repo dependency owns that declaration
- Each repo declares its own `depends_on` entries — no centralized registry
- No repo can block another repo's workflow
- Notification only, not auto-transition: when an RFC transitions, connected repos are notified but not forced to change

This follows the Kubernetes/Terraform pattern: components declare dependencies, tooling validates at runtime, deployments don't block each other.

## Implementation Plan

### Phase 1: Single-Repo Workflow Discipline
1. Add `blue guard` command to compiled binary with worktree detection, RFC status validation, path-in-worktree verification, and allowlist
2. Install PreToolUse hook in `.claude/settings.json` (project level) and `~/.claude/settings.json` (user level for cross-repo)
3. Add `Source Spike` field to RFC template and `Produces RFCs` field to spike template
4. Extend `blue_rfc_update_status` handler: on `implemented` transition, check for linked spike and auto-close
5. Extend `blue_rfc_update_status` handler: on `in-progress` transition, call `blue_adr_relevant` and emit hint
6. Remove `squash` parameter from `blue_pr_merge` — always squash for feature PRs
7. Add audit trail logging for `BLUE_BYPASS_WORKTREE` usage
8. Update MCP server instructions to describe new enforcement behavior

### Phase 2: Realm Coordination
9. Define `.blue/realm.toml` schema and parser
10. Add `Realm Dependencies` field to RFC frontmatter schema
11. Extend spike `Produces RFCs` to support qualified identifiers (`repo:rfc-number`)
12. Implement `blue_rfc_validate_realm` tool with repo caching and GitHub API fallback
13. Extend spike auto-close to poll cross-repo RFC statuses
14. Add `strict_realm_validation` config option for fail-on-unresolved mode
15. Emit cross-repo status hints during RFC transitions

## Alternatives Considered

### A. Phased Rollout (Worktree-First)
Implement only worktree enforcement in Phase 1, measure friction for 2 weeks, then add spike/ADR automation. Rejected (4-2 supermajority): gates target different workflow moments, so simultaneous deployment is safe. Partial enforcement trains agents to game gaps.

### B. Documentation-Only Approach
Add stronger language to MCP instructions about workflow discipline. Rejected (6/6 unanimous): "documented process lives in markdown while actual process lives in tool defaults" (Brioche). Soft guidance has proven insufficient.

### C. Spike-First Investigation
Conduct a spike measuring which interventions change agent behavior before committing to an RFC. Rejected (5/6): the root cause (lack of mechanical enforcement) and the mechanism (PreToolUse hooks) are both well-understood from the existing formalize-sdlc spike.

## Test Plan

### Single-Repo Tests
- [ ] `blue guard` blocks Write to `crates/` without active worktree
- [ ] `blue guard` allows Write to `.blue/docs/spikes/` without worktree
- [ ] `blue guard` allows Write with `BLUE_BYPASS_WORKTREE=1` and logs bypass
- [ ] `blue guard` allows Write inside active worktree directory
- [ ] Allowlist covers all documented paths (`.blue/docs/**`, `*.md` root, `/tmp/blue-dialogue/**`, `.claude/**`)
- [ ] RFC with `Source Spike` metadata auto-closes spike on `implemented` transition
- [ ] Multi-RFC spike stays `.wip.md` until all listed RFCs reach `implemented`
- [ ] Manual spike `.done.md` transition still works
- [ ] ADR hint emits on RFC transition to `in-progress`
- [ ] ADR hint includes keyword detection for "breaking", "redesign", "architectural"
- [ ] ADR hint does NOT block workflow
- [ ] `blue_pr_merge` always squashes (no override parameter)
- [ ] Audit trail file records bypass events with timestamp and path

### Realm Coordination Tests
- [ ] `.blue/realm.toml` parses `depends_on` arrays with qualified identifiers
- [ ] `blue_rfc_validate_realm` resolves local RFC statuses correctly
- [ ] `blue_rfc_validate_realm` fetches remote repo status via cached clone
- [ ] `blue_rfc_validate_realm` warns on unresolved dependencies (default mode)
- [ ] `blue_rfc_validate_realm --strict` fails on unresolved dependencies
- [ ] Spike with cross-repo `Produces RFCs` stays `.wip.md` until all reach `implemented`
- [ ] Spike with cross-repo `Produces RFCs` auto-closes when all reach `implemented`
- [ ] Cross-repo status hint emits during RFC transitions with realm dependencies
- [ ] `strict_realm_validation = true` in config enables fail mode without CLI flag
- [ ] Repo cache at `/tmp/blue-realm-cache/<repo>/` refreshes on query

---

*"The constraint of worktree-only editing enables the freedom of reliable, isolated, reviewable changes."*

— Converged from 6-expert alignment dialogue (395 ALIGNMENT, 14/14 tensions resolved, 4 rounds)
