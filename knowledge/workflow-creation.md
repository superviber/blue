# Creating Project Workflows

When a user asks to set up workflow, or `blue_status` indicates `.blue/workflow.md` is missing, help them create one.

## Step 1: Analyze the Project

Look for:
- **Build system**: `Cargo.toml` (Rust), `package.json` (Node), `pyproject.toml` (Python), `go.mod` (Go)
- **Existing branches**: Check `git branch -a` for patterns
- **CI config**: `.github/workflows/`, `.gitlab-ci.yml`, `Jenkinsfile`
- **Test setup**: How are tests run? What coverage is expected?
- **Existing docs**: `CONTRIBUTING.md`, `README.md` development sections

## Step 2: Ask Clarifying Questions

Use AskUserQuestion to gather:

1. **Branching strategy**
   - Trunk-based (main only)
   - Feature branches off main
   - Gitflow (develop, release branches)

2. **RFC conventions**
   - Where do RFCs live? (`.blue/docs/rfcs/` is default)
   - Naming pattern? (`NNNN-title.md` is default)
   - Approval process?

3. **Pre-commit requirements**
   - Run tests?
   - Lint checks?
   - Type checking?

4. **CI/CD expectations**
   - What must pass before merge?
   - Deployment process?

## Step 3: Generate workflow.md

Use the Write tool to create `.blue/workflow.md`:

```markdown
# Project Workflow

## Branching Strategy

{Based on user answers}

## Development Flow

1. Create RFC via `blue_rfc_create`
2. Create worktree via `blue_worktree_create`
3. Implement in isolation
4. {Pre-commit checks}
5. Create PR via `blue_pr_create`

## Pre-Commit Checklist

- [ ] {test command}
- [ ] {lint command}
- [ ] {type check if applicable}

## RFC Conventions

- Location: `.blue/docs/rfcs/`
- Format: `NNNN-title.md`
- {Additional conventions}

## CI Requirements

{What must pass before merge}
```

## Step 4: Confirm and Refine

After creating, ask:
- "I've created `.blue/workflow.md`. Take a look and let me know if anything needs adjustment."

## Example Conversation

**User**: "Help me set up the workflow for this project"

**Claude**:
1. Reads project structure (Cargo.toml found → Rust project)
2. Checks existing CI (.github/workflows/ci.yml found)
3. Asks: "I see this is a Rust project with GitHub Actions. A few questions..."
4. Generates workflow.md based on answers
5. "Created `.blue/workflow.md`. This will be injected into future sessions automatically."
