# RFC 0065: Jira Phase 1 Completion

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |
| **Depends on** | RFC 0063 (Jira Cloud Integration), RFC 0064 (Jira Test Infrastructure) |

---

## Summary

Complete Jira Cloud integration Phase 1 by implementing: credential storage with three-tier fallback, RFC stub generation from Jira import scans, CLI commands (`doctor`, `auth`, `import --dry-run`), and CI/CD pipeline for e2e tests. Also migrate the repo to GitHub and configure secrets.

## What Exists

- `IssueTracker` trait + `JiraCloudTracker` implementation (`crates/blue-core/src/tracker/`)
- 6 passing e2e tests against `superviber.atlassian.net` project `BLUE`
- Credentials in `.env.local` (gitignored)
- Tests skip gracefully when env vars absent

## Architecture

### 1. Credential Storage Layer

Three-tier hierarchy (checked in order per RFC 0063):

| Priority | Source | Lookup | Use Case |
|----------|--------|--------|----------|
| 1 | Env var `BLUE_JIRA_TOKEN_{DOMAIN_SLUG}` | `SUPERVIBER_ATLASSIAN_NET` | CI/CD |
| 2 | OS keychain `blue:jira:{domain}` | macOS Keychain / Linux secret-service | Interactive |
| 3 | `~/.config/blue/jira-credentials.toml` (chmod 0600) | TOML file | Fallback |

**Domain slug**: replace `.` and `-` with `_`, uppercase. `superviber.atlassian.net` → `SUPERVIBER_ATLASSIAN_NET`.

**Credential struct:**
```rust
// crates/blue-core/src/tracker/credentials.rs
pub struct CredentialStore {
    domain: String,
}

impl CredentialStore {
    pub fn get_credentials(&self) -> Result<TrackerCredentials, TrackerError>;
    pub fn store_credentials(&self, creds: &TrackerCredentials) -> Result<(), TrackerError>;
    pub fn clear_credentials(&self) -> Result<(), TrackerError>;
}
```

**Keychain**: Use `keyring` crate (cross-platform: macOS Keychain, Windows Credential Manager, Linux secret-service). Service name: `blue-jira`, username: `{email}@{domain}`.

**TOML fallback** (`~/.config/blue/jira-credentials.toml`):
```toml
[[credentials]]
domain = "superviber.atlassian.net"
email = "test@superviber.com"
token = "ATATT3x..."
```

### 2. CLI Commands

All commands go under `blue jira` subcommand group.

**`blue jira doctor`**
- Checks: credential resolution, API auth (`/myself`), project access, issue type availability
- Output: checklist with pass/fail per check
- Uses existing `auth_status()` + `list_projects()`

**`blue jira auth login --domain <domain>`**
- Prompts for email and token (or accepts `--email` / `--token` flags)
- Stores via `CredentialStore::store_credentials()`
- Verifies with `auth_status()` before storing

**`blue jira auth status`**
- Reports per-domain token health (valid/expired/missing)
- Never prints the token itself

**`blue jira import --dry-run --project <KEY> --domain <domain>`**
- Scans Jira project: lists epics and tasks
- Outputs proposed RFC stubs + epic YAML files
- Dry-run only in Phase 1 (no file writes)

### 3. RFC Stub Generation

When `blue jira import` scans a project, it generates RFC stubs from Jira tasks:

```markdown
# RFC NNNN: {Jira summary}

| | |
|---|---|
| **Status** | Imported |
| **Date** | {today} |
| **Jira** | {PROJ-123} |

---

## Summary

Imported from Jira: {description excerpt}

## Test Plan

- [ ] TBD
```

**Mapping rules:**
- Jira Epic → proposed epic YAML file
- Jira Task → proposed RFC stub
- Tasks under an Epic → RFC stubs reference the epic
- Status mapping: `To Do` → `draft`, `In Progress` → `draft`, `Done` → `accepted`

### 4. CI/CD Pipeline

**GitHub Actions workflow** (`.github/workflows/jira-e2e.yml`):

```yaml
name: Jira E2E Tests
on:
  pull_request:
    paths: ['crates/blue-core/src/tracker/**']
  schedule:
    - cron: '0 6 * * 1'  # Weekly Monday 6am UTC

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
      - uses: Swatinem/rust-cache@v2
      - run: cargo test -p blue-core -- tracker
```

**Main CI workflow** (`.github/workflows/ci.yml`):
- `cargo check`, `cargo test` (without Jira e2e — those skip without env vars)
- `cargo clippy`, `cargo fmt --check`
- Runs on all PRs to `main` and `develop`

**GitHub secrets needed** (4 values from `.env.local`):
- `BLUE_JIRA_TEST_DOMAIN`
- `BLUE_JIRA_TEST_EMAIL`
- `BLUE_JIRA_TEST_TOKEN`
- `BLUE_JIRA_TEST_PROJECT`

## Implementation Plan

### Task 1: Add GitHub remote + push
- [x] Add `git@github.com:Superviber/blue.git` as remote
- [x] Push `develop` and `main` branches

### Task 2: Credential storage layer
- [x] Add `keyring` crate to workspace dependencies
- [x] Implement `crates/blue-core/src/tracker/credentials.rs`
- [x] Wire into CLI via `CredentialStore` (env → keychain → TOML)
- [x] Unit tests for env var and TOML round-trip

### Task 3: CLI commands
- [x] Add `blue jira doctor` subcommand
- [x] Add `blue jira auth login` + `blue jira auth status`
- [x] Add `blue jira import --dry-run`

### Task 4: RFC stub generation
- [x] Implement `ImportScan` struct with stub templates
- [x] Map Jira epics/tasks to RFC stubs + epic YAML
- [x] Output formatted plan to stdout (dry-run mode)

### Task 5: CI/CD setup
- [x] Create `.github/workflows/ci.yml`
- [x] Create `.github/workflows/jira-e2e.yml`
- [x] Add GitHub repo secrets
- [ ] Verify pipelines pass

## Test Plan

- [x] Credential resolution: env var takes priority over keychain over TOML
- [ ] Keychain store/retrieve round-trip (keyring crate needs platform testing)
- [x] TOML fallback write + read with correct permissions
- [x] `blue jira doctor` output format with valid/invalid credentials
- [x] `blue jira import --dry-run` produces valid RFC stub markdown
- [ ] CI workflow triggers on tracker path changes
- [ ] E2E tests pass in GitHub Actions with secrets configured
- [ ] E2E tests skip in GitHub Actions without secrets (fork PRs)

---

*"Right then. Let's get to it."*

— Blue
