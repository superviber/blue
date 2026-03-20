# RFC 0064: Jira Test Infrastructure

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |
| **Depends on** | RFC 0063 (Jira Cloud Integration) |

---

## Summary

Set up a dedicated Jira Cloud instance for Blue's e2e test suite. Provides a real Jira environment to validate the IssueTracker adapter contract, drift detection, sync operations, and import functionality.

## Setup Checklist

### 1. Create Jira Cloud Site

- [ ] Go to https://www.atlassian.com/software/jira/free (free tier: 10 users)
- [ ] Sign up with `admin@superviber.com`
- [ ] Site name: `blue-test` → URL: `blue-test.atlassian.net`
- [ ] Select "Jira Software" (not Service Management)

### 2. Create Test Project

- [ ] Project name: **Blue E2E Tests**
- [ ] Project key: **BLUE**
- [ ] Project type: Team-managed (simpler, fewer admin requirements)
- [ ] Issue types: Epic, Task, Subtask (defaults are fine)

### 3. Create API Token

- [ ] Log in as `admin@superviber.com`
- [ ] Go to https://id.atlassian.com/manage-profile/security/api-tokens
- [ ] Create token, name: `blue-e2e-ci`
- [ ] Store token securely (needed for steps 5 and 6)

### 4. Seed Test Data

Create these artifacts in the BLUE project for import/read tests:

**Epics:**
- "Auth Overhaul" (status: In Progress)
- "API v2 Migration" (status: To Do)
- "Completed Feature" (status: Done)

**Tasks under "Auth Overhaul":**
- "Design OAuth2 flow" (Done)
- "Implement token refresh" (In Progress)
- "Write integration tests" (To Do)

**Standalone Tasks:**
- "Fix login redirect bug" (To Do)
- "Update API docs" (To Do)

### 5. Store Credentials Locally

```bash
# Install jira-cli
brew install ankitpokhrel/jira-cli/jira-cli

# Configure for test instance
jira init --server https://blue-test.atlassian.net --login admin@superviber.com

# Verify access
jira issue list -p BLUE
```

### 6. Store Credentials for CI

Add these GitHub repository secrets:

| Secret | Value |
|--------|-------|
| `BLUE_JIRA_TEST_DOMAIN` | `blue-test.atlassian.net` |
| `BLUE_JIRA_TEST_EMAIL` | `admin@superviber.com` |
| `BLUE_JIRA_TEST_TOKEN` | (API token from step 3) |
| `BLUE_JIRA_TEST_PROJECT` | `BLUE` |

## E2E Test Design

### Skip When No Credentials

```rust
fn jira_test_config() -> Option<JiraTestConfig> {
    Some(JiraTestConfig {
        domain: env::var("BLUE_JIRA_TEST_DOMAIN").ok()?,
        email: env::var("BLUE_JIRA_TEST_EMAIL").ok()?,
        token: env::var("BLUE_JIRA_TEST_TOKEN").ok()?,
        project: env::var("BLUE_JIRA_TEST_PROJECT").ok()?,
    })
}

// Tests skip gracefully when env vars aren't set
```

### Test Categories

| Category | Safety | Examples |
|----------|--------|---------|
| **Read-only** | Safe to repeat | List projects, get issue, auth status |
| **Write + cleanup** | Creates then deletes | Create issue, transition, link to epic |
| **Sync** | Uses temp git repo | Dry-run projection, drift detection |
| **Import** | Read-only scan | Discover seeded epics/tasks |

### Isolation Strategy

- Write tests prefix summaries with `blue-e2e-{run_id}-`
- Teardown deletes all `blue-e2e-*` issues
- Unique run ID per test invocation prevents parallel collisions

### CI Workflow

```yaml
# .github/workflows/jira-e2e.yml
name: Jira E2E Tests
on:
  pull_request:
    paths: ['crates/blue-core/src/tracker/**']
  schedule:
    - cron: '0 6 * * 1'  # Weekly Monday 6am

jobs:
  jira-e2e:
    runs-on: ubuntu-latest
    env:
      BLUE_JIRA_TEST_DOMAIN: ${{ secrets.BLUE_JIRA_TEST_DOMAIN }}
      BLUE_JIRA_TEST_EMAIL: ${{ secrets.BLUE_JIRA_TEST_EMAIL }}
      BLUE_JIRA_TEST_TOKEN: ${{ secrets.BLUE_JIRA_TEST_TOKEN }}
      BLUE_JIRA_TEST_PROJECT: ${{ secrets.BLUE_JIRA_TEST_PROJECT }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --features jira-e2e -p blue-core -- tracker
```

## Test Plan

- [x] Jira site created at superviber.atlassian.net
- [x] BLUE project exists with test data (e2e-created issues)
- [x] API token works locally (verified via REST API)
- [x] CI secrets configured in GitHub repo (Superviber/blue)
- [x] E2E tests pass locally (6/6)
- [x] E2E tests skip gracefully when secrets absent

---

*"Right then. Let's get to it."*

— Blue
