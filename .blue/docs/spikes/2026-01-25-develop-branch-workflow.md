# Spike: Develop Branch Workflow

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-25 |

---

## Question

How should we implement the develop branch workflow for Blue, and what's needed to enforce it consistently?

---

## Findings

### Current State

| Branch | Location | Purpose |
|--------|----------|---------|
| `main` | forgejo, origin | Only branch - contains all production code |
| `git-forge-integration` | forgejo | RFC 0013 feature branch |
| `mcp-workflow-guidance` | forgejo | RFC 0011 feature branch |

**Problem:** `blue_pr_create` defaults to `develop` base and rejects `main`/`master`, but no `develop` branch exists. Feature branches can't be merged.

### The Workflow Model

```
main (production releases only)
  ↑ merge when releasing
develop (integration branch)
  ↑ PRs merge here
feature branches (rfc/*, feature/*)
```

**Why this matters:**
- `main` stays stable - only receives tested, integrated code
- `develop` is where features integrate before release
- Prevents accidental direct commits to production
- Enables release management (develop → main when ready)

### What's Needed

1. **Create `develop` branch** from current `main`
2. **Push to forgejo**
3. **Update forgejo settings** - set default branch to `develop`
4. **Rebase existing feature branches** onto `develop`
5. **Add `blue_release_create`** tool for develop → main merges

### Tool Enforcement (Already Done)

`blue_pr_create` in `pr.rs:59-71`:
- Defaults base to `develop`
- Rejects `main`/`master` with helpful message
- This is correct - just needs the branch to exist

## Recommendation

**Proceed.** Create the `develop` branch now. This is a one-time setup:

```bash
# From main branch
git checkout main
git checkout -b develop
git push forgejo develop

# On Forgejo: Settings → Branches → Default Branch → develop
```

Then existing feature branches can create PRs targeting `develop`.

## Outcome

Proceed with implementation - create develop branch and update forgejo settings.
