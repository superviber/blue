# RFC 0063: Jira Cloud Integration

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |
| **Dialogue** | [2026-03-11T1341Z-jira-cloud-integration-for-blue](../dialogues/2026-03-11T1341Z-jira-cloud-integration-for-blue.dialogue.recorded.md) |
| **ALIGNMENT** | 101 (3 rounds, 12 experts, 7/7 tensions resolved) |

---

## Summary

Integrate Blue with Jira Cloud so that RFCs and Feature Releases project into Jira as Tasks and Epics. The git project-management repo is the sole authority; Jira is a write-through projection. Blue provides an IssueTracker trait with a pluggable adapter contract, domain-keyed credential management, and progressive convention enforcement.

## Problem

Blue manages the engineering lifecycle (RFCs, worktrees, PRs) but has no bridge to organizational project management tools. Teams using Jira Cloud must manually create and synchronize Jira tickets for each RFC, leading to drift between engineering state and PM-visible state. There is no convention for how repos under a domain relate to Jira projects, how credentials are managed, or how conventions are enforced.

## Architecture

### Core Principle: Git as Authority, Jira as Projection

The external project-management git repo is the single source of truth (ADR 0005). Jira Cloud is a write-through projection — Blue pushes state to Jira idempotently via `blue sync` and never pulls Jira state back to override local state.

### 1. IssueTracker Trait (Adapter Contract)

Blue defines an out-of-process adapter contract. No CLI tool is bundled or hard-required.

```
blue-jira-provider <command> [args] → exit-code + JSON stdout
```

**Commands:**

| Command | Input | Output |
|---------|-------|--------|
| `create-issue` | `{project, type, summary, epic_key, blue_uuid}` | `{key: "PROJ-142"}` |
| `transition-issue` | `{key, status}` | `{ok: true}` |
| `get-issue` | `{key}` | `{key, status, summary, assignee, ...}` |
| `list-issues` | `{project, epic_key?}` | `[{key, status, ...}]` |
| `link-issue` | `{key, link_type, target_key}` | `{ok: true}` |

**Provider discovery:** Blue looks for `blue-jira-provider` on `$PATH`. The recommended implementation wraps `jira-cli` (ankitpokhrel/jira-cli), but any binary conforming to the contract works. Blue ships no provider — users install one.

**Graceful degradation:** Local operations (`blue rfc create`, `blue rfc complete`) succeed with a single-line warning when no provider is found. Only `blue sync` and `blue jira *` commands require the provider.

**Provider declaration:** The PM repo's `jira.toml` declares:
```toml
provider = "jira-cloud"    # future: "linear", "shortcut"
domain = "myorg.atlassian.net"
project_key = "PROJ"
```

### 2. Three-Tier Field Ownership

Instead of binary "git wins" or "Jira wins", fields are partitioned by ownership:

| Tier | Fields | Owner | Sync Behavior |
|------|--------|-------|---------------|
| **Structural** | Status, Epic membership, RFC linkage, issue type | Git | Overwritten on `blue sync` with logged warning |
| **Operational** | Assignee, sprint, priority, labels | Jira | Blue never reads or writes these |
| **Descriptive** | Summary, description | Drift-warned | `sync_hash` in RFC front matter detects Jira-side edits; surfaced as drift report, not overwritten |

**Drift policy** (configurable per domain in PM repo):

| Mode | Behavior | Use Case |
|------|----------|----------|
| `overwrite` | Silently overwrite structural fields | Teams where PMs use Blue exclusively |
| `warn` (default) | Detect divergence, log drift report, push git state | Teams with active PM Jira usage |
| `block` | Refuse to push if structural drift detected | High-compliance environments |

**Drift detection:** Compares last-pushed projection state (stored in `.blue/jira/last-sync.yaml` per entity) against current Jira state. Joins on immutable `blue_uuid`, not Jira issue keys.

### 3. Credential Management

**Three-tier hierarchy** (checked in order):

| Priority | Source | Use Case |
|----------|--------|----------|
| 1 | `BLUE_JIRA_TOKEN_{DOMAIN_SLUG}` env var | CI/CD |
| 2 | OS keychain entry `blue:jira:{domain}` | Interactive (macOS Keychain, Linux secret-service) |
| 3 | `~/.config/blue/jira-credentials.toml` (chmod 0600) | Systems without keychain support |

**Commands:**
- `blue jira auth login --domain myorg.atlassian.net` — stores token in keychain
- `blue jira auth status` — reports per-domain token health (valid/expired/missing) without printing token
- `blue jira doctor` — pre-flight validation: provider on PATH, token valid, project accessible

**Security constraints:**
- `blue lint` checks staged files for Atlassian API token patterns
- PM repo template ships with `.gitignore`: `*.credentials.toml`, `.jira-cli/`, `*.secret`
- Bot accounts recommended for CI (documented in setup guide), personal tokens accepted
- `blue lint` warns if personal token detected in CI environment (`CI=true`)

### 4. Project-Management Repo Structure

The PM repo is a declarative manifest, not a sync mirror. It declares domain membership, Epic structure, and conventions.

```
domains/{domain-name}/
  domain.yaml              # repo membership, Jira project key, drift policy
  jira.toml                # provider config, domain, project key
  conventions.toml         # enforcement tiers
  epics/
    {epic-id}.yaml         # one file per Epic — name, Jira key, status
  releases/
    {release-id}.yaml      # Feature Release grouping Epic references
```

**`domain.yaml` example:**
```yaml
domain: myorg.atlassian.net
project_key: PROJ
drift_policy: warn
repos:
  - name: frontend
    url: git@github.com:myorg/frontend.git
  - name: backend
    url: git@github.com:myorg/backend.git
```

**Epic file example** (`epics/user-auth-overhaul.yaml`):
```yaml
epic_id: user-auth-overhaul
jira_key: PROJ-50
status: active
created: 2026-03-11
repos:
  - frontend
  - backend
```

### 5. RFC-to-Jira Artifact Mapping

**Binding lives in RFC front matter** (repo-local, not centralized):

```yaml
# In .blue/docs/rfcs/NNNN-title.draft.md front matter
jira:
  blue_uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890   # immutable, minted at creation
  task_key: PROJ-142                                    # mutable, written on first sync
  epic_id: user-auth-overhaul                           # references PM repo Epic
```

**Mapping rules:**
- RFC → Jira Task (1:1)
- Feature Release → Jira Epic (1:1 default)
- `blue_uuid` is minted at RFC creation, never changes, survives Jira ticket renames/moves
- `task_key` is a denormalized convenience; `blue sync` can re-derive it from `blue_uuid`
- `epic_id` references the PM repo's Epic file, not a Jira key directly (indirection layer)

**Explicit override:** Teams whose Epic semantics differ can set `jira_epic_key: PROJ-42` directly in the release YAML, bypassing Blue's naming convention. `blue lint` warns on non-standard cardinality.

### 6. Bootstrapping: `blue jira import`

For teams with existing Jira boards, a two-phase import:

**Phase 1 — Discovery:**
```
blue jira import --project PROJ --domain myorg.atlassian.net --dry-run
```
Scans Jira Epics and Tasks, outputs proposed PM repo structure and RFC stubs.

**Phase 2 — Adoption:**
```
blue jira import --project PROJ --domain myorg.atlassian.net
```
Creates a PR to the PM repo with Epic files and RFC stubs. Imported artifacts get status `imported` until a human promotes them to `draft` or `active`. Writes a `bootstrap-manifest.yaml` recording what was imported.

**Post-import:** Git becomes the sole authority. The import is a one-time on-ramp, not an ongoing sync direction.

### 7. Progressive Convention Enforcement

Three tiers, configurable per domain in `conventions.toml`:

**Tier 1 — Always enforced (repo-side):**
- RFC front matter must include `jira.blue_uuid` before `blue rfc complete`
- No credentials in staged files (`blue lint`)
- PM repo schema validation (Epic files match declared schema)

**Tier 2 — Warn-then-enforce (configurable):**
- Epic naming conventions (warn for configurable period, then enforce)
- Label taxonomy compliance
- Status transition ordering
- Enforcement mode: `warn` → `gate` with per-domain auto-escalation config

**Tier 3 — Document-only (Jira-side):**
- Jira workflow configuration (required fields, allowed transitions)
- Jira project permission schemes
- Custom field setup for `blue_uuid`
- Blue validates and reports; humans fix Jira configuration

## Setup Guide

### For Users (per-machine)

1. **Install a Jira provider:**
   ```
   brew install ankitpokhrel/jira-cli/jira-cli
   # Or install any blue-jira-provider conforming binary
   ```

2. **Create an API token:**
   - Go to https://id.atlassian.com/manage-profile/security/api-tokens
   - Create token with descriptive name (e.g., "blue-cli-macbook")
   - For CI: create a dedicated bot account with least-privilege permissions

3. **Authenticate:**
   ```
   blue jira auth login --domain myorg.atlassian.net
   ```

4. **Verify:**
   ```
   blue jira doctor
   ```

### For Repos (per-project)

1. **Create or clone the project-management repo:**
   ```
   blue jira init --domain myorg.atlassian.net --project PROJ
   ```

2. **Link code repos:**
   Add repos to `domains/{domain}/domain.yaml`

3. **Import existing Jira state (if brownfield):**
   ```
   blue jira import --project PROJ --domain myorg.atlassian.net
   ```

4. **Configure conventions:**
   Edit `domains/{domain}/conventions.toml` for enforcement tiers

## Implementation Plan

### Phase 1: Read-Only Foundation
- [x] Define IssueTracker adapter contract (CLI interface spec)
- [x] Implement `blue jira doctor` (pre-flight validation)
- [x] Implement `blue jira auth` (credential management)
- [x] Implement `blue jira import --dry-run` (discovery scan)
- [x] PM repo template with domain.yaml, jira.toml schema

### Phase 2: Write Path
- [x] Implement `blue sync` Jira projection (create/transition issues)
- [x] UUID minting in `blue sync` (auto-mints blue_uuid on first sync)
- [x] Drift detection and reporting
- [x] `blue jira import` full (writes RFC stubs and epic YAML files)

### Phase 3: Enforcement
- [x] `blue lint` Jira credential checks (ATATT tokens, token assignments, TOML creds, Basic auth, env vars)
- [x] Progressive enforcement engine (warn → gate via drift_policy config)
- [x] Tier 1 enforcement: `blue sync --drift-policy block` refuses sync on drift

### Phase 4: Polish
- [x] `blue jira status` (cross-repo Jira state overview with sync/drift/unsync reporting)
- [x] Drift report formatting in `blue sync` output
- [x] Setup guide documentation (in RFC)

## Design Decisions (from Dialogue T08-T11)

| Decision | Resolution |
|----------|-----------|
| **Drift detection frequency** | Event-driven on `blue sync`; no built-in scheduler. `stale_after` config knob (default 24h) for `blue status` warnings. |
| **Error ergonomics** | Local ops (rfc create/complete) succeed with warning when provider absent. Sync ops fail fast with actionable error. |
| **Credential mechanism** | Ship all three tiers for v1. TOML with chmod 0600. OS keychain as primary interactive path. |
| **Bot account provisioning** | Recommended in docs, not required. Personal tokens accepted. No bot-vs-human detection. |

## Test Plan

- [ ] IssueTracker adapter contract: mock provider binary for unit tests
- [ ] Credential hierarchy: test env var → keychain → TOML fallback order
- [ ] Drift detection: test structural/operational/descriptive field classification
- [ ] Import: test Jira API mock → PM repo artifact generation
- [ ] Lint: test credential pattern detection in staged files
- [ ] Progressive enforcement: test warn → gate escalation
- [ ] Multi-domain: test credential isolation across domains
- [ ] Offline-first: verify RFC operations succeed without provider

---

*"Right then. Let's get to it."*

— Blue
