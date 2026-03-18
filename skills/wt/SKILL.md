---
name: wt
description: Manage worktree lifecycle — start work from an RFC, sync with develop, complete and merge back.
---

# Worktree Management Skill

Manage the lifecycle of git worktrees tied to RFCs. Each worktree corresponds to a `feature/{slug}` branch and an RFC document.

## Usage

```
/wt              # defaults to "start"
/wt start        # detect worktree, find RFC, set up environment
/wt sync         # rebase from develop to get latest changes
/wt done         # complete work, merge to develop, mark RFC implemented
/wt status       # show current worktree state
```

## Subcommands

### `/wt start`

Set up the worktree for work. This is the default when no subcommand is given.

---LC|COOK|DEV|START|wt-start|setting up worktree---

**Steps:**

1. **Confirm worktree**: Check that `.git` is a file (not a directory). If `.git` is a directory, abort with: "Not in a worktree. Use `git worktree add` to create one first."

2. **Extract slug**: Run `git branch --show-current` to get the branch name. Extract the slug from the `feature/{slug}` pattern. If the branch does not match `feature/*`, abort with: "Branch does not follow `feature/{slug}` convention."

3. **Find the RFC**: Glob for `.blue/docs/rfcs/*-A-{slug}.md` or `*-I-{slug}.md` in the repo root. The RFC filename format is `NNNN-{status}-{slug}.md` where status is `A` (accepted) or `I` (implemented).
   - If no RFC found, warn: "No RFC found for slug `{slug}`. Proceeding without RFC context."
   - If found, read the RFC and display its title, number, and status.

4. **Environment setup**: Check if `scripts/setup-worktree.sh` exists in the repo root.
   - If it exists and this looks like a first-time setup (e.g., no `target/` or `node_modules/`), run it.
   - If no setup script, auto-detect the project type and suggest the install command:
     - `Cargo.toml` present -> `cargo build`
     - `package.json` present -> `npm install`
     - `requirements.txt` present -> `pip install -r requirements.txt`
     - `go.mod` present -> `go mod download`
   - Ask before running install commands.

5. **Show plan**: If the RFC contains task checkboxes (`- [ ]` / `- [x]`), display progress as a summary (e.g., "3/7 tasks complete").

6. **Git config**: Set local git config for clean rebasing if not already set:
   ```
   git config pull.rebase true
   git config rebase.autoStash true
   ```

---LC|COOK|DEV|END|wt-start|setting up worktree---

---

### `/wt sync`

Rebase the current branch onto the latest develop.

---LC|COOK|DEV|START|wt-sync|syncing with develop---

**Steps:**

1. **Fetch**: Run `git fetch origin`.

2. **Rebase**: Run `git rebase origin/develop`.

3. **Handle conflicts**: If the rebase fails due to conflicts:
   - Run `git rebase --abort` to restore clean state.
   - Report the conflicting files clearly.
   - Abort with: "Rebase conflicts detected. Resolve manually or try again after adjusting your changes."

4. **Report**: Show what changed:
   - Number of new commits pulled in from develop.
   - Run `git log --oneline ORIG_HEAD..HEAD` to show the rebased range if applicable.

---LC|COOK|DEV|END|wt-sync|syncing with develop---

---

### `/wt done`

Complete the worktree work and merge back to develop.

---LC|COOK|DEV|START|wt-done|completing worktree---

**Steps:**

1. **Check clean state**: Run `git status --porcelain`. If there is output, abort with: "Uncommitted changes found. Commit or stash before completing."

2. **Sync first**: Run the full `/wt sync` flow (fetch + rebase onto develop). If rebase fails, abort.

3. **Switch to develop**:
   ```
   git checkout develop
   git pull --rebase origin develop
   ```

4. **Fast-forward merge**: Run `git merge --ff-only feature/{slug}`.
   - If fast-forward is not possible, abort with: "Cannot fast-forward merge. The branch has diverged from develop. Switch back to your feature branch and run `/wt sync` first."

5. **Mark RFC as implemented**: Find the RFC file matching `*-A-{slug}.md` in `.blue/docs/rfcs/`.
   - Rename from `NNNN-A-{slug}.md` to `NNNN-I-{slug}.md`.
   - Inside the file, update the `**Status**:` line to `**Status**: Implemented`.
   - If the RFC is already `*-I-{slug}.md`, skip this step.

6. **Commit RFC change**:
   ```
   git add .blue/docs/rfcs/
   git commit -m "docs: mark RFC NNNN as implemented"
   ```

7. **Push**: Run `git push origin develop`.

8. **Clean up worktree**: Determine the worktree path (current working directory or from `git worktree list`).
   - Switch out of the worktree directory first if needed.
   - Run `git worktree remove {worktree_path}`.
   - Run `git branch -d feature/{slug}`.

9. **Report**: Confirm completion with:
   - RFC number and title
   - Branch removed
   - Worktree removed
   - Develop pushed to origin

---LC|COOK|DEV|END|wt-done|completing worktree---

---

### `/wt status`

Show the current state of the worktree.

---LC|COOK|DEV|START|wt-status|checking worktree status---

**Steps:**

1. **RFC info**: Find and display the RFC title, number, and status (Accepted/Implemented).

2. **Branch**: Show the current branch name from `git branch --show-current`.

3. **Ahead/behind**: Run `git rev-list --left-right --count origin/develop...HEAD`.
   - Display as: "{N} commits ahead, {M} commits behind develop".

4. **Plan progress**: If the RFC has task checkboxes, show completion (e.g., "5/12 tasks complete").

5. **Working tree state**: Run `git status --short` and show:
   - Number of modified files
   - Number of untracked files
   - Number of staged files
   - Or "Clean working tree" if nothing to report.

---LC|COOK|DEV|END|wt-status|checking worktree status---

## Error Handling

- Always check command exit codes before proceeding to the next step.
- On any git failure, report the error clearly and stop. Do not attempt to recover automatically from merge conflicts or rebase failures.
- If the user is not in a worktree (`.git` is a directory), suggest how to create one.

## Notes

- The slug is the portion after `feature/` in the branch name. For example, branch `feature/branch-workflow-enforcement` has slug `branch-workflow-enforcement`.
- RFC filenames follow the pattern `NNNN-{A|I}-{slug}.md` where `A` = Accepted, `I` = Implemented.
- All git operations use explicit remotes (`origin`) rather than relying on tracking branch defaults.
- The `/wt done` flow is destructive (removes worktree and branch). Confirm with the user before executing step 8 (cleanup).
