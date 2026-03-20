---
name: wt
description: Manage git worktree lifecycle for isolated feature development against approved RFCs
allowed-tools: Read, Write, Edit, Glob, Bash, AskUserQuestion
---

# /wt: Worktree Workflow Skill

Manage the full lifecycle of feature worktrees: start work from an approved RFC, stay in sync with develop, and land completed work cleanly.

## Context

- Worktrees are isolated sandboxes for implementation only.
- Doc changes (RFCs, decisions, etc.) belong on `develop`, not in worktrees.
- Branch naming: `feature/{slug}` where `{slug}` matches the RFC filename.
- RFC naming: `NNNN-{D|A|I|S}-slug.md` in `.blue/docs/rfcs/`.
- The `blue` CLI is installed and available for project commands.

## Commands

### /wt start

Detect worktree context, find the associated RFC, read the plan, run dev setup if needed, and show the task checklist.

**Steps:**

1. **Detect worktree context.** Check if `.git` is a file (not a directory). If it is a directory, this is the main repo -- inform the user and stop.

   ```bash
   test -f .git && echo "worktree" || echo "main-repo"
   ```

2. **Extract the slug from the branch name.**

   ```bash
   git branch --show-current
   ```

   The branch should be `feature/{slug}`. Parse out `{slug}`. If the branch does not match `feature/*`, warn the user and stop.

3. **Find the RFC file.** Glob for the RFC in `.blue/docs/rfcs/`:

   ```bash
   ls .blue/docs/rfcs/*-A-{slug}.md .blue/docs/rfcs/*-I-{slug}.md 2>/dev/null
   ```

   - If a matching `*-A-{slug}.md` file is found, proceed (RFC is Approved, ready for implementation).
   - If only `*-I-{slug}.md` is found, inform the user the RFC is already marked Implemented.
   - If only `*-D-{slug}.md` is found, inform the user the RFC is still in Draft and must be approved on `develop` before implementation can begin. Stop.
   - If no match is found, warn that no RFC was found for this slug. Stop.

4. **Read the RFC.** Read the full RFC file and extract:
   - Title (first `# ` heading)
   - Status
   - Phases and task checklists (look for `### Phase N:` sections with `- [ ]` / `- [x]` items)

5. **Run dev setup if needed.** Check whether dependencies are installed:

   ```bash
   # Check if setup script exists
   test -f scripts/setup-worktree.sh && echo "setup-script-exists"
   ```

   If `scripts/setup-worktree.sh` exists and this appears to be a fresh worktree (e.g., no `target/` directory for Rust projects, no `node_modules/` for Node projects), run the setup script:

   ```bash
   bash scripts/setup-worktree.sh
   ```

   If no setup script exists, check for common project indicators and suggest appropriate setup:
   - `Cargo.toml` present: suggest `cargo build`
   - `package.json` present: suggest `npm install`
   - Ask the user before running anything.

6. **Display the plan.** Show a summary:

   ```
   WORKTREE: feature/{slug}
   RFC: {NNNN} - {Title}
   Status: Approved

   PLAN

   Phase 1: {Name}
     [ ] Task 1
     [ ] Task 2

   Phase 2: {Name}
     [ ] Task 1
     [ ] Task 2

   {N} tasks remaining across {M} phases.

   Ready to start implementing. What would you like to work on first?
   ```

### /wt sync

Rebase the current worktree branch from develop and report any conflicts.

**Steps:**

1. **Verify worktree context.** Confirm `.git` is a file. If not, inform the user this command is for worktrees only.

2. **Check for uncommitted changes.** Run `git status --porcelain`. If there are changes, inform the user:

   ```
   You have uncommitted changes. They will be auto-stashed during rebase
   (rebase.autoStash is enabled).

   Proceeding with sync...
   ```

3. **Fetch and rebase from develop.**

   ```bash
   git fetch origin develop
   git rebase origin/develop
   ```

4. **Report the result.**

   - On success:
     ```
     Synced with develop. Branch is up to date.

     {N} new commits from develop applied.
     ```

   - On conflict:
     ```
     CONFLICT during rebase.

     Conflicting files:
       - {file1}
       - {file2}

     Options:
       [1] Show conflict details (git diff)
       [2] Abort rebase (git rebase --abort)
       [3] I will resolve manually

     Which option?
     ```

   If the user chooses to resolve manually, remind them to run `git rebase --continue` after resolving each conflict.

### /wt done

Complete the feature work: verify clean state, rebase, fast-forward merge to develop, rename RFC to Implemented, and clean up the worktree.

**Pre-flight checks (all must pass before proceeding):**

1. **Verify worktree context.** Confirm `.git` is a file. Stop if not a worktree.

2. **Check for uncommitted changes.**

   ```bash
   git status --porcelain
   ```

   If there are uncommitted changes, warn and stop:

   ```
   ABORT: Uncommitted changes detected.

   Uncommitted files:
     - {file1}
     - {file2}

   Please commit or stash your changes before running /wt done.
   ```

3. **Extract slug and locate RFC.** Same as `/wt start` steps 2-3. The RFC must be `*-A-{slug}.md`.

**Merge flow:**

4. **Rebase onto develop.**

   ```bash
   git fetch origin develop
   git rebase origin/develop
   ```

   If conflicts occur, report them and stop. The user must resolve conflicts and re-run `/wt done`.

5. **Determine the worktree path and the main repo path.**

   ```bash
   # Get the main repo (common dir)
   git worktree list --porcelain
   ```

   Parse the output to find the main worktree path (the first entry is always the main worktree).

6. **Fast-forward merge into develop.** From the main repo:

   ```bash
   # In the main repo directory
   git -C {main_repo_path} checkout develop
   git -C {main_repo_path} merge --ff-only feature/{slug}
   ```

   If fast-forward fails, inform the user. This should not happen after a successful rebase, but if it does, suggest running `/wt sync` and retrying.

7. **Rename RFC from A to I on develop.** Now on the develop branch in the main repo:

   ```bash
   cd {main_repo_path}
   git mv .blue/docs/rfcs/NNNN-A-{slug}.md .blue/docs/rfcs/NNNN-I-{slug}.md
   ```

   Also update the `**Status**:` line inside the RFC file from `Approved` to `Implemented`.

8. **Commit the RFC status change.**

   ```bash
   git -C {main_repo_path} add .blue/docs/rfcs/NNNN-I-{slug}.md
   git -C {main_repo_path} commit -m "docs: mark RFC {NNNN} as implemented ({slug})"
   ```

9. **Push develop.**

   ```bash
   git -C {main_repo_path} push origin develop
   ```

10. **Clean up the worktree and branch.**

    ```bash
    # From the main repo
    git -C {main_repo_path} worktree remove {worktree_path}
    git -C {main_repo_path} branch -d feature/{slug}
    ```

11. **Report completion.**

    ```
    DONE: feature/{slug} merged to develop.

    RFC {NNNN}-I-{slug}.md marked as Implemented.
    Worktree removed.
    Branch feature/{slug} deleted.

    To release develop to main, run: blue release
    ```

12. **Post-cleanup warning.** If the user was running inside the worktree terminal:

    ```
    Note: The worktree directory has been deleted. Run cd to navigate
    to the main repo at {main_repo_path}.
    ```

### /wt status

Show the current state of the worktree, its RFC, plan progress, and position relative to develop.

**Steps:**

1. **Verify worktree context.** Confirm `.git` is a file.

2. **Gather information.** Run these commands:

   ```bash
   # Branch name
   git branch --show-current

   # Commits ahead of develop
   git rev-list --count origin/develop..HEAD

   # Commits behind develop
   git rev-list --count HEAD..origin/develop

   # Uncommitted changes count
   git status --porcelain | wc -l
   ```

3. **Find and read the RFC.** Same as `/wt start` steps 2-4. Extract title and task checklist.

4. **Calculate progress.** Count `- [x]` (done) and `- [ ]` (remaining) across all phases.

5. **Display status.**

   ```
   WORKTREE STATUS

   RFC: {NNNN} - {Title}
   Branch: feature/{slug}
   Status: {Approved|Implemented}

   Progress: {done}/{total} tasks complete
     Phase 1: {Name} -- {done}/{total}
     Phase 2: {Name} -- {done}/{total}

   Commits: {N} ahead, {M} behind develop
   Uncommitted changes: {count}

   {Contextual suggestion based on state:}
   ```

   Contextual suggestions (pick the most relevant one):
   - Behind develop: `You are behind develop. Run /wt sync to rebase.`
   - All tasks complete and clean: `All tasks complete. Run /wt done to merge.`
   - Clean and up to date: `Clean and up to date. Keep cooking.`
   - Uncommitted changes: `You have uncommitted changes.`

## Error Handling

**Not in a worktree:**
```
This is not a worktree. The /wt skill is for use inside feature worktrees.

To create a worktree for an approved RFC, run on develop:
  blue worktree create {slug}
```

**Branch name does not match `feature/*`:**
```
Current branch "{branch}" does not follow the feature/{slug} convention.

Worktrees should use feature/{slug} branches that correspond to an approved RFC.
```

**No RFC found for slug:**
```
No RFC found matching slug "{slug}" in .blue/docs/rfcs/.

Expected file: .blue/docs/rfcs/*-A-{slug}.md

Check that the RFC exists and has been approved on develop, then run /wt sync
to pull the latest.
```

**RFC is still Draft:**
```
RFC {NNNN}-D-{slug}.md is still in Draft status.

The RFC must be approved before implementation can begin. Switch to develop
and approve the RFC first.
```

## Notes

- The guard blocks `.blue/docs/` writes in worktrees. Never attempt to modify RFC files, decisions, or other docs from a worktree. The RFC rename in `/wt done` happens on develop after the merge.
- `pull.rebase=true` and `rebase.autoStash=true` are set by `blue init` and worktree creation, so `git pull` always rebases.
- After `/wt done`, the worktree directory is deleted. If running inside the worktree terminal, the user's shell will be in a deleted directory. Remind them to `cd` to the main repo.
