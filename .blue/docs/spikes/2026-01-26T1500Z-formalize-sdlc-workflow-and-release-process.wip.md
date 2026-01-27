# Spike: Formalize SDLC Workflow and Release Process

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 2 hours |

---

## Question

What gaps exist between our current implicit workflow practices and a formalized SDLC? Specifically: (1) Are docs committed upon RFC approval? (2) Is worktree usage enforced for all implementation? (3) Is worktree cleanup enforced post-merge? (4) Is squash merge enforced? (5) Is direct-to-main merge properly blocked? (6) Is the release process with semver fully formalized?

---

## Investigation

### Current Enforcement Audit

| Area | Enforcement | Location |
|------|-------------|----------|
| Doc commit on acceptance | **MISSING** | `server.rs:2684-2783` |
| Worktree required for implementation | **SOFT** (warning only) | `server.rs:2864-2955` |
| Worktree cleanup after merge | **SOFT** (suggestion only) | `pr.rs:341-441` |
| Squash merge | **SOFT** (default, overridable) | `pr.rs:344-413` |
| Direct-to-main blocking | **HARD** (error, no bypass) | `pr.rs:98-107` |
| Release process + semver | **PARTIAL** (blocks in-progress, manual commands) | `release.rs:28-98` |
| Specialized agent roles | **AD HOC** (alignment-expert exists, doc-writer informal) | `.claude/agents/` |

### Gap Analysis

#### 1. Doc Commit on Approval — MISSING

When `blue_rfc_update_status` transitions an RFC to `accepted`, the handler updates the markdown file's status field via `update_markdown_status()` but does **not** git-commit the change. The document sits modified but uncommitted.

**What should happen:** On acceptance, the RFC document (and any companion plan file) should be committed to the current branch with a message like `docs: accept RFC NNNN - Title`.

#### 2. Worktree Enforcement — SOFT

`blue_rfc_update_status` emits a warning when transitioning to `in-progress` without a worktree, but `blue_rfc_task_complete` has **no worktree check at all** — tasks can be marked complete from any branch. There's no enforcement that implementation happens in isolation.

**What should happen:** `blue_rfc_task_complete` should verify the current working directory is inside a worktree for that RFC, or at minimum warn. `blue_worktree_create` should remain the gate for transitioning to `in-progress` (which it already enforces via plan file requirement).

#### 3. Worktree Cleanup After Merge — SOFT

`blue_pr_merge` returns `"next_steps": ["Run blue_worktree_cleanup..."]` but takes no action. Cleanup is entirely manual.

**What should happen:** After a successful merge, `blue_pr_merge` should either auto-invoke cleanup or emit a stronger signal (e.g., a reminder that surfaces in `blue_next`/`blue_status`).

#### 4. Squash Merge — SOFT

`squash` defaults to `true` but accepts `squash=false`. No validation rejects non-squash merges.

**What should happen:** Remove the `squash` parameter entirely. All PR merges should be squash-only. The `MergeStrategy::Merge` path should only be available to `blue_release_create` when merging develop into main.

#### 5. Direct-to-Main Blocking — HARD (Already correct)

`blue_pr_create` rejects `base="main"` or `base="master"` with an error. No bypass exists. This is the only fully enforced constraint.

**No change needed.** This is correct as-is.

#### 6. Release Process — PARTIAL

`blue_release_create` blocks if RFCs are in-progress (good), analyzes implemented RFCs for version bump type (good), generates changelog entries (good), but:
- Returns shell commands instead of executing them
- Version is hardcoded to `"0.1.0"` instead of reading from `Cargo.toml`
- Does not create the release PR, tag, or push
- Does not update version files

**What should happen:** The release handler should:
1. Read current version from `Cargo.toml`
2. Calculate next version from implemented RFCs
3. Allow version override
4. Create a release branch from develop
5. Update `Cargo.toml` version (all workspace members)
6. Generate/update `CHANGELOG.md`
7. Commit version bump
8. Create PR targeting main (the ONE exception to the base-branch rule)
9. After merge: tag `vX.Y.Z`, push tag
10. Mark included RFCs as `released`

#### 7. Specialized Agent Roles — AD HOC

The project already uses `.claude/agents/alignment-expert.md` for dialogue participants. The `technical-writer` built-in subagent type has been used ad hoc for writing spike documentation, producing well-structured output that follows Blue's document formats. But there's no formalization of which agent types to use for which SDLC activities.

**Current state:**
- `alignment-expert` — custom agent in `.claude/agents/`, used by dialogue orchestration
- `technical-writer` — built-in subagent, used informally for spike writeups
- No custom agent for Blue-specific doc writing (spikes, RFCs, ADRs)
- No mapping of SDLC phase → agent type

**What should happen:** Create a `doc-writer` custom agent in `.claude/agents/` that knows Blue's document formats (spike, RFC, ADR), voice rules (2 sentences before action, no hedging, evidence over opinion), and writing conventions (tables for comparisons, code blocks for examples, direct conclusions). Map each SDLC activity to a recommended agent:

| SDLC Activity | Agent | Rationale |
|----------------|-------|-----------|
| Spike investigation writeup | `doc-writer` | Structured findings, consistent format |
| RFC drafting | `doc-writer` | Knows RFC template, voice rules |
| Alignment dialogues | `alignment-expert` | Bounded output, marker format |
| Code implementation | default (opus) | Full tool access needed |
| Code review / analysis | `Explore` subagent | Read-only, thorough search |

---

## Proposed SDLC Workflow (Formalized)

### Branch Model

```
main ──────────────────────────────── (releases only, tagged)
  │
  └── develop ─────────────────────── (integration branch, all PRs target here)
        │
        ├── feature-branch-1 ──────── (worktree, squash-merged to develop)
        ├── feature-branch-2 ──────── (worktree, squash-merged to develop)
        └── release/vX.Y.Z ────────── (release prep, merged to main + tagged)
```

### Lifecycle

```
RFC Draft
  │
  ├─ Dialogue/Review
  │
  ▼
RFC Accepted ──────── git commit docs (auto)
  │
  ├─ Plan file created
  │
  ▼
Worktree Created ──── branch from develop, isolated workspace
  │
  ├─ Implementation (tasks completed in worktree only)
  │
  ▼
RFC Implemented ───── ≥70% plan progress gate
  │
  ├─ PR created (squash-only, base=develop)
  ├─ Test plan verified
  ├─ Approvals checked
  │
  ▼
PR Merged (squash) ── worktree cleaned up (auto or prompted)
  │
  ├─ ... repeat for other RFCs ...
  │
  ▼
Release ───────────── version bump, changelog, PR to main, tag
```

### Semver Rules

| RFC Title Keywords | Bump | Example |
|--------------------|------|---------|
| "breaking", "remove", "deprecate", "redesign" | **Major** | 1.0.0 → 2.0.0 |
| "add", "implement", "feature", "new", "support" | **Minor** | 1.0.0 → 1.1.0 |
| "fix", "patch", "docs", "refactor", "test" | **Patch** | 1.0.0 → 1.0.1 |

Pre-1.0: Breaking changes bump minor, features bump patch.

### Release Checklist

1. No RFCs in `in-progress` status
2. All worktrees cleaned up
3. Version calculated from implemented RFCs since last release
4. `Cargo.toml` version updated across workspace
5. `CHANGELOG.md` generated from RFC titles
6. Release PR: `develop` → `main` (only valid main-targeting PR)
7. After merge: `git tag vX.Y.Z && git push origin vX.Y.Z`
8. Implemented RFCs marked as `released`

---

## Findings Summary

### Already Working
- Direct-to-main blocking (HARD gate)
- Worktree creation requires plan file
- PR merge defaults to squash
- Release blocks on in-progress work
- Branch naming convention (RFC 0007)

### Needs Hardening
1. **Doc commit on acceptance** — Add git add/commit in `blue_rfc_update_status`
2. **Squash-only merge** — Remove `squash` parameter from `blue_pr_merge`, always squash for feature PRs
3. **Worktree cleanup** — Auto-cleanup after merge or surface as blocking reminder in `blue_status`
4. **Release execution** — Read real version, create release branch, update files, create PR, tag

### Needs New Implementation
5. **Release branch flow** — `release/vX.Y.Z` branch that's the one exception to the main-targeting PR rule
6. **Version file updates** — Automated `Cargo.toml` workspace version bump
7. **CHANGELOG generation** — Append to `CHANGELOG.md` from implemented RFC list
8. **RFC `released` status** — New terminal status after release ships
9. **`doc-writer` custom agent** — `.claude/agents/doc-writer.md` with Blue format/voice knowledge
10. **Agent-to-phase mapping** — Formalize which agent handles which SDLC activity

---

## Recommendation

Create an RFC to implement these changes. The work naturally splits into two phases:

**Phase 1 — Workflow Hardening (enforce what we already have):**
- Auto-commit docs on acceptance
- Remove squash override (always squash feature PRs)
- Auto-cleanup or remind after merge
- Worktree presence check in task completion

**Phase 2 — Release Formalization:**
- Read version from Cargo.toml
- Release branch creation
- Version bump + changelog generation
- Tag creation and push
- RFC `released` status lifecycle

**Phase 3 — Agent Formalization:**
- Create `doc-writer` custom agent (Blue format/voice knowledge, sonnet model)
- Define agent-to-phase mapping (doc-writer for spikes/RFCs, alignment-expert for dialogues, Explore for review)
- Install at both project (`.claude/agents/`) and user (`~/.claude/agents/`) level for cross-repo use

---

## Cross-Spike Note: Fixing "Edit Before RFC" via Plugin Architecture

The core bug — Claude jumping into code edits before an RFC is approved and a worktree is active — has a mechanical fix when combined with the plugin architecture spike and the thin-plugin/fat-binary spike.

### The Problem

MCP instructions tell Claude about voice and ADRs, but say nothing about workflow discipline. The worktree check is a warning, not a gate. Claude's default behavior is to be helpful, so it edits files directly on `develop` without an RFC or worktree. No amount of conversational guidance fixes this because Claude doesn't reliably follow soft suggestions across sessions.

### The Fix: PreToolUse Hooks (Mechanical Gate)

The plugin architecture spike documents that hooks can intercept `PreToolUse` events. A `PreToolUse` hook on `Write`, `Edit`, and `Bash` (for file-writing commands) can call the compiled binary to check:

1. Is the current working directory inside a Blue worktree?
2. Does the worktree correspond to an RFC in `accepted` or `in-progress` status?
3. Are the files being modified within the worktree's directory tree?

If any check fails, the hook **blocks the tool call** before it executes. Claude cannot bypass this — hooks run before the tool, not after.

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

### Why This Fits Thin Plugin / Fat Binary

Per the thin-plugin/fat-binary spike, the hook file is a one-liner that calls the compiled binary. The `blue guard` command contains all the logic in compiled Rust:

- **Static (user sees):** `"command": "blue guard --tool=$TOOL_NAME"` — meaningless without knowing what `guard` checks
- **Runtime (compiled):** Worktree detection, RFC status validation, path-in-worktree verification, allowlist for non-code files (docs, spikes, ADRs that don't require worktrees)

### Allowlist: What Can Be Edited Without a Worktree

Not all edits require an RFC. The guard command needs an allowlist:

| Path Pattern | Requires Worktree? | Rationale |
|---|---|---|
| `.blue/docs/spikes/**` | No | Spikes are investigation, not implementation |
| `.blue/docs/adrs/**` | No | ADRs are philosophical, not code |
| `.blue/docs/rfcs/**` | No | RFC drafts are pre-implementation |
| `.blue/docs/dialogues/**` | No | Dialogues are discussion artifacts |
| `.claude/agents/**` | No | Agent definitions are config |
| `crates/**`, `src/**`, `tests/**` | **Yes** | Code requires RFC + worktree |
| `Cargo.toml`, `Cargo.lock` | **Yes** | Dependency changes are implementation |
| Any other file | **Yes** (default) | Safe default |

### Combined Enforcement Stack

With all three spikes folded into one RFC, the enforcement becomes layered:

1. **MCP instructions** (fat binary) — Tell Claude the rules: "Do not edit code without an approved RFC and active worktree"
2. **PreToolUse hook** (plugin) — Mechanically block edits that violate the rules before they execute
3. **Status transition gates** (MCP tools) — Prevent RFC status from advancing without prerequisites (plan, worktree, 70% progress)
4. **`blue_next` / `blue_status`** (MCP tools) — Surface violations as the top priority action

Layer 1 is aspirational (Claude may ignore it). Layer 2 is mechanical (Claude cannot bypass it). Layers 3-4 are structural (the workflow itself prevents skipping steps). Together they close the gap.
