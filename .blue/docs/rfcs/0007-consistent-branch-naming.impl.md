# RFC 0007: Consistent Branch Naming

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |

---

## Summary

Branch names and worktrees for RFC implementation are inconsistent. Some use the full RFC name with number prefix, others use arbitrary names. This makes it hard to correlate branches with their source RFCs and clutters the git history.

## Problem

Currently when implementing an RFC:
- Branch names vary: `rfc-0005`, `feature/local-llm`, `0005-local-llm-integration`, etc.
- Worktree directories follow no convention
- No clear way to find which branch implements which RFC
- PR titles don't consistently reference the RFC number

## Proposal

### Naming Convention

For an RFC file named `NNNN-feature-description.md`:

| Artifact | Name |
|----------|------|
| RFC file | `NNNN-feature-description.md` |
| Branch | `feature-description` |
| Worktree | `feature-description` |
| PR title | `RFC NNNN: Feature Description` |

### Examples

| RFC File | Branch | Worktree |
|----------|--------|----------|
| `0005-local-llm-integration.md` | `local-llm-integration` | `local-llm-integration` |
| `0006-document-deletion-tools.md` | `document-deletion-tools` | `document-deletion-tools` |
| `0007-consistent-branch-naming.md` | `consistent-branch-naming` | `consistent-branch-naming` |

### Rationale

**Why strip the number prefix?**
- Branch names stay short and readable
- The RFC number is metadata, not the feature identity
- `git branch` output is cleaner
- Tab completion is easier

**Why keep feature-description?**
- Direct correlation to RFC title
- Descriptive without being verbose
- Consistent kebab-case convention

### Implementation

1. Update `blue_worktree_create` to derive branch name from RFC title (strip number prefix)
2. Update `blue_pr_create` to include RFC number in PR title
3. ~~Add validation to reject branches with number prefixes~~ (deferred - convention is enforced by tooling)
4. Document convention in CLAUDE.md

### Migration

Existing branches don't need to change. Convention applies to new work only.

## Test Plan

- [x] `blue worktree create` uses `feature-description` format
- [x] Branch name derived correctly from RFC title
- [x] PR title includes RFC number when `rfc` parameter provided
- [ ] ~~Validation rejects `NNNN-*` branch names with helpful message~~ (deferred)

## Implementation Plan

- [x] Update worktree handler to strip RFC number from branch name
- [x] Update PR handler to format title as `RFC NNNN: Title`
- [x] Add `strip_rfc_number_prefix` helper function with tests
- [ ] Update documentation (CLAUDE.md)

---

*"Names matter. Make them count."*

— Blue
