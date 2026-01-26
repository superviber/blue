# RFC 0001: Cross-Repo Coordination with Realms

| | |
|---|---|
| **Status** | Accepted |
| **Created** | 2026-01-24 |
| **Implemented** | 2026-01-24 |
| **CLI Docs** | [docs/cli/realm.md](../cli/realm.md) |
| **Source** | [Spike: cross-repo-coordination](../spikes/cross-repo-coordination.md) |
| **Dialogue** | [cross-repo-realms.dialogue.md](../dialogues/cross-repo-realms.dialogue.md) |
| **Refinement** | [cross-repo-realms-refinement.dialogue.md](../dialogues/cross-repo-realms-refinement.dialogue.md) |

---

## Problem

We have repositories under different ownership that depend on each other:
- `aperture` (training-tools webapp) needs S3 access to data in another AWS account
- `fungal-image-analysis` grants that access via IAM policies

When aperture adds a new S3 path, fungal's IAM policy must update. Currently:
1. No awareness - Blue in aperture doesn't know fungal exists
2. No coordination - Changes require manual cross-repo communication
3. No tracking - No record of cross-repo dependencies

## Goals

1. **Awareness** - Blue sessions know about related repos and their dependencies
2. **Coordination** - Changes in one repo trigger notifications in dependent repos
3. **Trust boundaries** - Different orgs can coordinate without shared write access
4. **Auditability** - All cross-repo coordination is version-controlled

## Non-Goals

- Automatic code changes across repos (manual review required)
- Public realm discovery (future scope)
- Monorepo support (for MVP, each repo is a distinct participant)
- Cyclic dependencies between domains

---

## Proposal

### Hierarchy

```
Daemon (per-machine)
  └── Realm (git repo in Forgejo)
        └── Domain (coordination context)
              ├── Repo A (participant)
              └── Repo B (participant)
```

| Level | Purpose | Storage |
|-------|---------|---------|
| **Daemon** | Manages realms, sessions, notifications | `~/.blue/daemon.db` |
| **Realm** | Groups related coordination domains | Git repo in Forgejo, cloned to `~/.blue/realms/` |
| **Domain** | Coordination context between repos | Directory in realm repo |
| **Repo** | Actual code repository (can participate in multiple domains) | `.blue/config.yaml` declares membership |

**Key insight:** A domain is the *relationship* (edge), not the *thing* (node). Repos are nodes; domains are edges connecting them.

```
         ┌─────────────────────────────────────────┐
         │           Realm: letemcook              │
         │                                         │
         │  ┌─────────────────────────────────┐   │
         │  │    Domain: s3-access            │   │
         │  │                                 │   │
         │  │   ┌──────────┐   ┌──────────┐  │   │
         │  │   │ aperture │◄─►│  fungal  │  │   │
         │  │   │ (export) │   │ (import) │  │   │
         │  │   └──────────┘   └──────────┘  │   │
         │  │                                 │   │
         │  └─────────────────────────────────┘   │
         │                                         │
         │  ┌─────────────────────────────────┐   │
         │  │    Domain: training-pipeline    │   │
         │  │                                 │   │
         │  │   ┌──────────┐   ┌──────────┐  │   │
         │  │   │  fungal  │◄─►│ ml-infra │  │   │
         │  │   │ (export) │   │ (import) │  │   │
         │  │   └──────────┘   └──────────┘  │   │
         │  │                                 │   │
         │  └─────────────────────────────────┘   │
         │                                         │
         └─────────────────────────────────────────┘
```

Note: `fungal` participates in both domains with different roles.

---

## Architecture

### Daemon

Blue runs as a per-machine daemon that manages realm state, git operations, and session coordination.

```
┌─────────────────────────────────────────────────────────────┐
│                      Blue Daemon                             │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ HTTP Server  │  │ Git Manager  │  │ Session Mgr  │      │
│  │ localhost:   │  │ (git2 crate) │  │              │      │
│  │ 7865         │  │              │  │              │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         │                 │                 │                │
│         └─────────────────┼─────────────────┘                │
│                           │                                  │
│                    ┌──────┴──────┐                          │
│                    │ daemon.db   │                          │
│                    │ (SQLite)    │                          │
│                    └─────────────┘                          │
└─────────────────────────────────────────────────────────────┘
          ▲                              ▲
          │ HTTP                         │ HTTP
          │                              │
    ┌─────┴─────┐                 ┌──────┴──────┐
    │ blue CLI  │                 │ Blue GUI    │
    │           │                 │ (future)    │
    └───────────┘                 └─────────────┘
```

**Key properties:**
- Runs as system service (launchd/systemd)
- Auto-starts on first CLI invocation
- HTTP API on `localhost:7865` (GUI-friendly)
- All git operations via `git2` crate (no subprocess)
- Single-user assumption (multi-user is future work)

### Directory Structure

```
~/.blue/
├── daemon.db                     # Daemon state (realms, sessions, notifications)
├── realms/                       # Managed realm repo clones
│   └── {realm-name}/             # Cloned from Forgejo
└── credentials.yaml              # Optional, prefer git credentials

/var/run/blue/                    # Or $XDG_RUNTIME_DIR/blue/ on Linux
└── blue.pid                      # Daemon PID file

realm-{name}/                     # Git repository (in Forgejo)
├── realm.yaml                    # Metadata, governance, trust
├── repos/
│   └── {repo}.yaml               # Registered repos
└── domains/
    └── {domain}/
        ├── domain.yaml           # Coordination context
        ├── governance.yaml       # Optional domain-level governance
        ├── contracts/
        │   └── {name}.yaml       # Schema, version, validation, owner
        └── bindings/
            └── {repo}.yaml       # Export/import declarations

{repo}/.blue/
├── config.yaml                   # Realm membership (name + Forgejo URL)
└── cache.db                      # SQLite cache for exports, contracts
```

### realm.yaml

```yaml
name: letemcook
version: "1.0.0"
created_at: 2026-01-24T10:00:00Z

governance:
  admission: approval  # open | approval | invite-only
  approvers:
    - eric@example.com
  breaking_changes:
    require_approval: true
    grace_period_days: 14

trust:
  mode: collaborative  # collaborative | vendor-customer | federation
  require_signed_commits: false

  permissions:
    repos/{repo}.yaml: [repo_maintainers]
    domains/{domain}/domain.yaml: [domain_owners]
    domains/{domain}/contracts/{name}.yaml: [contract_owner]
    domains/{domain}/bindings/{repo}.yaml: [repo_maintainers]
```

### repos/{repo}.yaml

```yaml
name: aperture
org: cultivarium  # Optional org prefix
path: /Users/ericg/letemcook/aperture
# Or remote:
# url: git@github.com:cultivarium/aperture.git

maintainers:
  - eric@example.com

joined_at: 2026-01-24T10:00:00Z
```

### domains/{domain}/domain.yaml

```yaml
name: s3-access
description: Coordinates S3 bucket access between aperture and fungal

created_at: 2026-01-24T10:00:00Z

members:
  - aperture
  - fungal-image-analysis
```

### domains/{domain}/contracts/{contract}.yaml

```yaml
name: s3-permissions
version: "1.4.0"
owner: aperture  # Only this repo can modify the contract

compatibility:
  backwards: true   # New version readable by old importers
  forwards: false   # Old version NOT readable by new importers

schema:
  type: object
  required: [read]
  properties:
    read:
      type: array
      items: { type: string }
    write:
      type: array
      items: { type: string }
      default: []
    delete:
      type: array
      items: { type: string }
      default: []

value:
  read:
    - "jobs/*/masks/*"
    - "jobs/*/*/config.json"
    - "training-runs/*"
    - "training-metrics/*"
  write:
    - "jobs/*/*/manifest.json"
    - "training-metrics/*"
  delete: []

validation:
  exporter: scripts/validate-s3-paths.sh
  importer: scripts/validate-iam-policy.sh
  ci_only:
    - scripts/integration-test.sh

evolution:
  - version: "1.0.0"
    changes: ["Initial with read array"]
    compatible: true
  - version: "1.3.0"
    changes: ["Added write array"]
    compatible: true
  - version: "1.4.0"
    changes: ["Added delete array"]
    compatible: true
```

### domains/{domain}/bindings/{repo}.yaml

**Export binding:**
```yaml
repo: aperture
role: provider

exports:
  - contract: s3-permissions
    source_files:
      - models/training/s3_paths.py
      - models/shared/data/parquet_export.py
```

**Import binding:**
```yaml
repo: fungal-image-analysis
role: consumer

imports:
  - contract: s3-permissions
    version: ">=1.0.0, <2.0.0"  # Semver range
    binding: cdk/training_tools_access_stack.py
    status: current
    resolved_version: "1.4.0"
    resolved_at: 2026-01-24T12:00:00Z
```

### Local Configuration

```yaml
# aperture/.blue/config.yaml
realm:
  name: letemcook
  url: https://git.example.com/realms/letemcook.git

repo: aperture

# Domains and contracts are defined in the realm repo (single source of truth).
# This config just declares membership. The daemon resolves the rest.
```

The realm repo is authoritative for what repos exist and their roles. Local config only declares "I belong to realm X at URL Y." The daemon clones and manages the realm repo automatically.

---

## Coordination Model

Blue uses a **hybrid coordination model**:

1. **Real-time awareness (Daemon):** Fast session tracking and notifications
2. **Durable changes (Git PRs):** Source of truth, auditable, PR-based

```
┌──────────────────────────────────────────────────────────────┐
│                    Coordination Layers                        │
├──────────────────────────────────────────────────────────────┤
│  Real-time Awareness (Daemon)                                 │
│  ┌─────────────┐                     ┌─────────────┐         │
│  │ CLI/GUI A   │◄───────────────────►│ CLI/GUI B   │         │
│  │ (aperture)  │    Blue Daemon      │ (fungal)    │         │
│  └─────────────┘    localhost:7865   └─────────────┘         │
│                                                               │
│  Instant notifications, session tracking, auto-cleanup       │
├──────────────────────────────────────────────────────────────┤
│  Durable Changes (Git via Forgejo)                           │
│  ┌─────────────┐                     ┌─────────────┐         │
│  │ Repo        │────sync branch─────►│ Realm Repo  │         │
│  │             │◄───PR review────────│ (Forgejo)   │         │
│  └─────────────┘                     └─────────────┘         │
│                                                               │
│  Source of truth, PR-based, auditable                        │
└──────────────────────────────────────────────────────────────┘
```

### Session Coordination (Daemon-Managed)

The daemon manages sessions and notifications in `~/.blue/daemon.db`:

```sql
-- ~/.blue/daemon.db

CREATE TABLE realms (
    name TEXT PRIMARY KEY,
    forgejo_url TEXT NOT NULL,
    local_path TEXT NOT NULL,
    last_sync TEXT,
    status TEXT DEFAULT 'active'
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,
    realm TEXT NOT NULL,
    client_id TEXT,           -- CLI instance or GUI window
    started_at TEXT,
    last_activity TEXT,
    active_rfc TEXT,
    active_domains JSON DEFAULT '[]',
    exports_modified JSON DEFAULT '[]',
    imports_watching JSON DEFAULT '[]'
);

CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    realm TEXT NOT NULL,
    domain TEXT NOT NULL,
    contract TEXT NOT NULL,
    from_repo TEXT NOT NULL,
    change_type TEXT NOT NULL,  -- 'updated', 'breaking', 'new'
    changes JSON,
    created_at TEXT,
    acknowledged_by JSON DEFAULT '[]'
);
```

**Session lifecycle:**
- CLI registers session on command start, deregisters on exit
- Daemon tracks activity via API calls (no heartbeat polling needed)
- Orphaned sessions cleaned on daemon restart
- GUI clients maintain persistent sessions

### Daemon API

The daemon exposes an HTTP API on `localhost:7865`:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Daemon health check |
| `/realms` | GET | List tracked realms |
| `/realms/{name}` | GET | Realm details, domains, repos |
| `/realms/{name}/sync` | POST | Trigger sync for realm |
| `/sessions` | GET | List active sessions |
| `/sessions` | POST | Register new session |
| `/sessions/{id}` | DELETE | Deregister session |
| `/notifications` | GET | List pending notifications |
| `/notifications/{id}/ack` | POST | Acknowledge notification |

**Auto-start:** If CLI calls daemon and it's not running, CLI spawns daemon as background process before proceeding.

### Sync Protocol (Git PRs)

Contract changes go through a PR workflow for durability:

1. `blue realm sync` creates branch `sync/{repo}/{timestamp}`
2. Pushes changes to realm repo
3. Creates PR with affected parties as reviewers
4. Merges when:
   - All affected importers acknowledge, OR
   - Grace period expires (for non-breaking changes)
5. On merge, notification broadcast to active sessions

---

## Commands

### MVP Commands (5)

| Command | Purpose |
|---------|---------|
| `blue realm status` | Show realm state, domains, sessions, notifications |
| `blue realm sync` | Push local changes, pull remote changes via PR |
| `blue realm worktree` | Create linked worktrees across repos in domain |
| `blue realm pr` | Create coordinated PRs with merge order |
| `blue realm check` | Validate contracts (for CI) |

### Admin Commands

Under `blue realm admin`:

| Command | Purpose |
|---------|---------|
| `blue realm admin init` | Create a new realm |
| `blue realm admin join` | Register repo in realm |
| `blue realm admin domain` | Create/manage domains |
| `blue realm admin leave` | Remove repo from realm |
| `blue realm admin cache` | Cache management |

### Command Details

#### blue realm status

```bash
$ blue realm status
📊 aperture (in realm: letemcook)

Domains:
  s3-access (provider)
    Contract: s3-permissions v1.4.0

Active sessions:
  • aperture (you) - working on training-metrics-v2
  • fungal-image-analysis - idle

Notifications:
  ⚠️  5 min ago: fungal acknowledged s3-permissions 1.4.0
```

#### blue realm sync

```bash
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Creating sync branch: sync/aperture/20260124-103000
Contract changes detected:
  s3-permissions: 1.4.0 → 1.5.0
  + training-metrics/experiments/*

Opening PR for review...
  ✓ PR #12: "aperture: s3-permissions 1.5.0"
  Reviewers: fungal-image-analysis maintainers

Notifying active sessions...
  ✓ fungal-image-analysis notified

Waiting for acknowledgment or grace period (14 days for non-breaking).
```

#### blue realm check

```bash
# Validate exports match code
$ blue realm check --mode=exporter
✓ s3-permissions: code matches contract

# Validate imports are current
$ blue realm check --mode=importer
✓ s3-permissions: binding at 1.4.0 (current)

# Validate contract schema
$ blue realm check --mode=contract
✓ s3-permissions: valid schema, version bump correct

# Check compatibility between versions
$ blue realm check --mode=compatibility --from=1.4.0 --to=1.5.0
Checking s3-permissions 1.4.0 → 1.5.0
  ✓ read: unchanged
  ✓ write: unchanged
  ✓ delete: unchanged
  + experiments: added (optional, default=[]) - COMPATIBLE

Result: COMPATIBLE (minor version bump OK)
```

---

## Caching

Blue uses SQLite for caching to handle concurrent access properly:

```sql
-- .blue/cache.db

CREATE TABLE export_cache (
    file_path TEXT PRIMARY KEY,
    mtime INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    exports JSON,
    cached_at INTEGER NOT NULL
);

CREATE TABLE contract_cache (
    domain TEXT NOT NULL,
    contract TEXT NOT NULL,
    version TEXT NOT NULL,
    schema JSON,
    realm_commit TEXT,
    cached_at INTEGER NOT NULL,
    PRIMARY KEY (domain, contract, version)
);

CREATE TABLE validation_cache (
    contract TEXT NOT NULL,
    version TEXT NOT NULL,
    binding_hash TEXT NOT NULL,
    valid INTEGER NOT NULL,
    output TEXT,
    validated_at INTEGER NOT NULL,
    PRIMARY KEY (contract, version, binding_hash)
);
```

**Invalidation rules:**
- Exports: Invalidate when source file mtime changes
- Contracts: Invalidate when realm repo pulls new commits
- Validation: Invalidate when contract version or binding changes

**Escape hatches:**
```bash
blue realm sync --no-cache    # Force fresh detection
blue realm admin cache clear  # Wipe cache
blue realm admin cache stats  # Show cache hit rates
```

---

## CI/CD Integration

### Forgejo Actions Example

```yaml
name: Realm Contract Check

on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/cache@v3
        with:
          path: .blue/cache.db
          key: blue-cache-${{ hashFiles('.blue/config.yaml') }}

      - name: Install Blue
        run: cargo install blue-cli

      - name: Check exports
        run: blue realm check --mode=exporter

      - name: Check imports
        run: blue realm check --mode=importer
        env:
          BLUE_REALM_TOKEN: ${{ secrets.REALM_TOKEN }}

      - name: Check compatibility
        if: gitea.event_name == 'pull_request'
        run: blue realm check --mode=compatibility
```

**Note:** Forgejo Actions uses `gitea.*` context variables. The workflow syntax is otherwise compatible with GitHub Actions.

### Validation Hooks

Contracts can define validation scripts:

```yaml
validation:
  exporter: scripts/validate-s3-paths.sh    # Run on export
  importer: scripts/validate-iam-policy.sh  # Run on import
  ci_only:
    - scripts/integration-test.sh           # Only in CI
```

**Exit codes:**
- 0: Valid
- 1: Invalid (hard fail)
- 2: Warning (soft fail, can be overridden)

---

## Credentials

Blue uses a layered credential approach:

```
Priority order (first found wins):
1. Environment: BLUE_REALM_TOKEN, BLUE_FORGEJO_TOKEN
2. Git credential helper: git credential fill
3. Keychain: macOS Keychain, Linux secret-service
4. Config file: ~/.blue/credentials.yaml (discouraged)
```

**For CI with Forgejo:**
```yaml
env:
  BLUE_FORGEJO_URL: https://git.example.com
  BLUE_FORGEJO_TOKEN: ${{ secrets.FORGEJO_TOKEN }}
```

**Default behavior:** Uses existing git credentials. No additional setup for basic usage.

**Note:** GitHub/GitLab support is future work. MVP targets Forgejo only.

---

## Conflict Resolution

| Conflict Type | Detection | Resolution |
|---------------|-----------|------------|
| Concurrent sync | Git merge conflict | Rebase and retry |
| Same contract, two editors | Ownership check | Reject non-owner |
| Stale binding | Version mismatch | Soft warning, CI flag |
| Version incompatibility | Semver check | Hard error |
| Session collision | Unique ID + heartbeat | Auto-cleanup stale |

### Contract Ownership

Each contract has an `owner` field. Only the owner repo can modify the contract:

```yaml
name: s3-permissions
owner: aperture  # Only aperture can update this contract
```

Enforced at:
1. `blue realm sync`: Rejects if non-owner tries to modify
2. Realm repo CI: CODEOWNERS enforces in PRs

---

## Cycle Prevention

Cycles between domains are detected and prevented at domain creation:

```bash
$ blue realm admin domain create feedback-loop --repos a,b
Error: Adding this domain creates a cycle:
  a → (s3-access) → b → (feedback-loop) → a

Consider:
  - Merging domains into a single coordination context
  - Restructuring the dependency direction
  - Using a hub-and-spoke pattern
```

Detection uses topological sort on the domain dependency graph.

---

## Workflow

### Initial Setup

```bash
# 1. Create realm (one-time, creates repo in Forgejo)
$ blue realm admin init --name letemcook --forgejo https://git.example.com
Starting daemon...
✓ Daemon running on localhost:7865
✓ Created realm repo in Forgejo: git.example.com/realms/letemcook
✓ Cloned to ~/.blue/realms/letemcook/
✓ Created realm.yaml

# 2. Register aperture in realm
$ cd aperture
$ blue realm admin join letemcook
✓ Created repos/aperture.yaml in realm
✓ Created .blue/config.yaml locally
✓ Pushed to Forgejo

# 3. Create the s3-access domain
$ blue realm admin domain create s3-access \
    --repos aperture,fungal-image-analysis \
    --contract s3-permissions
✓ Created domains/s3-access/domain.yaml
✓ Created domains/s3-access/contracts/s3-permissions.yaml
✓ Created domains/s3-access/bindings/aperture.yaml (provider)
✓ Created domains/s3-access/bindings/fungal-image-analysis.yaml (consumer)
✓ Pushed to Forgejo

# 4. Register fungal in realm
$ cd ../fungal-image-analysis
$ blue realm admin join letemcook
✓ Created repos/fungal-image-analysis.yaml in realm
✓ Detected import: s3-permissions in domain s3-access
✓ Created .blue/config.yaml locally
✓ Pushed to Forgejo
```

### Daily Development

```bash
# Developer in aperture adds new S3 path
$ cd aperture
$ vim models/training/metrics_exporter.py
# Added: s3://bucket/training-metrics/experiments/*

# Check status
$ blue realm status
📊 aperture (in realm: letemcook)

⚠️  Contract change detected:
   s3-permissions has local changes:
   + training-metrics/experiments/*

   Affected repos:
   - fungal-image-analysis (imports >=1.0.0 <2.0.0)

   Run 'blue realm sync' to update realm

# Sync changes
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Creating branch: sync/aperture/20260124-150000
Contract update: s3-permissions 1.4.0 → 1.5.0 (compatible)

✓ PR #15 created for review
✓ fungal-image-analysis session notified
```

### Consumer Response

```bash
# Maintainer in fungal sees notification
$ blue realm status
📊 fungal-image-analysis (in realm: letemcook)

🔔 Notification from aperture (5 min ago):
   Contract 's3-permissions' updated to 1.5.0
   + training-metrics/experiments/*

   Your import (>=1.0.0 <2.0.0) is still satisfied.
   Update binding when ready.

# Update binding
$ vim cdk/training_tools_access_stack.py
# Add new path to IAM policy

# Sync to acknowledge
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Binding updated:
  s3-permissions: resolved_version 1.4.0 → 1.5.0

✓ Acknowledged PR #15
✓ Realm synced
```

### Coordinated Changes (Unified Worktrees)

```bash
# Start cross-repo change
$ blue realm worktree --rfc training-metrics-v2

Creating unified worktree 'feat/training-metrics-v2':
  ✓ aperture: .worktrees/feat-training-metrics-v2/
  ✓ fungal-image-analysis: .worktrees/feat-training-metrics-v2/

Branch 'feat/training-metrics-v2' created in both repos.

# Make changes in both worktrees...

# Create coordinated PRs
$ blue realm pr --title "feat: training metrics v2"

Creating coordinated PRs:
  ✓ aperture PR #45 (merge first - exports)
  ✓ fungal PR #23 (merge second - imports)
    Blocked by: aperture#45

Merge order: aperture#45 → fungal#23
```

---

## Implementation

### Phase Overview

| Phase | Scope |
|-------|-------|
| 0 | Daemon infrastructure (HTTP server, auto-start, git2 integration) |
| 1 | Data model + SQLite schemas |
| 2 | `blue realm admin init/join` (creates realm in Forgejo, manages clones) |
| 3 | `blue realm status` with realm info |
| 4 | `blue realm sync` with PR workflow |
| 5 | `blue realm check` for CI |
| 6 | Session coordination (daemon-managed) |
| 7 | `blue realm worktree` |
| 8 | `blue realm pr` |
| 9 | Caching layer |
| 10 | Polish + docs |

### Dependencies

- **git2** crate for all git operations
- **axum** or **actix-web** for daemon HTTP server
- **rusqlite** for daemon.db and cache.db
- **reqwest** for Forgejo API calls

### Phase 0: Data Model

```rust
// blue-core/src/realm.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealmConfig {
    pub name: String,
    pub version: String,
    pub governance: Governance,
    pub trust: TrustConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustConfig {
    pub mode: TrustMode,
    pub require_signed_commits: bool,
    pub permissions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub name: String,
    pub version: String,
    pub owner: String,
    pub compatibility: Compatibility,
    pub schema: serde_json::Value,
    pub value: serde_json::Value,
    pub validation: Option<ValidationConfig>,
    pub evolution: Vec<EvolutionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Compatibility {
    pub backwards: bool,
    pub forwards: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub exporter: Option<String>,
    pub importer: Option<String>,
    pub ci_only: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub repo: String,
    pub role: BindingRole,
    pub exports: Vec<ExportBinding>,
    pub imports: Vec<ImportBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBinding {
    pub contract: String,
    pub version: String,  // Semver range
    pub binding: String,
    pub status: ImportStatus,
    pub resolved_version: Option<String>,
}
```

---

## Test Plan

### Core Operations
- [ ] `blue realm admin init` creates valid realm structure
- [ ] `blue realm admin join` registers repo correctly
- [ ] `blue realm admin domain create` creates domain with contracts
- [ ] `blue realm admin domain create` rejects cycles
- [ ] `blue realm status` shows realm state, domains, notifications
- [ ] `blue realm sync` creates PR for contract changes
- [ ] `blue realm sync` rejects non-owner contract modifications
- [ ] `blue realm sync --dry-run` shows changes without committing

### Validation
- [ ] `blue realm check --mode=exporter` validates exports match code
- [ ] `blue realm check --mode=importer` validates bindings
- [ ] `blue realm check --mode=contract` validates schema
- [ ] `blue realm check --mode=compatibility` detects breaking changes
- [ ] Validation hooks run with correct exit codes

### Daemon
- [ ] Daemon starts on first CLI invocation if not running
- [ ] Daemon responds on `localhost:7865`
- [ ] Daemon creates `~/.blue/daemon.db` on first start
- [ ] Daemon clones realm repos to `~/.blue/realms/`
- [ ] Daemon PID file created in `/var/run/blue/` or `$XDG_RUNTIME_DIR/blue/`
- [ ] Daemon graceful shutdown cleans up resources
- [ ] CLI commands fail gracefully if daemon unreachable

### Session Coordination
- [ ] Session registered on CLI command start
- [ ] Session deregistered on CLI command exit
- [ ] Orphaned sessions cleaned on daemon restart
- [ ] Contract changes create notifications
- [ ] Notifications visible in `blue realm status`

### Caching
- [ ] Export cache invalidates on file mtime change
- [ ] Contract cache invalidates on realm commit
- [ ] `--no-cache` forces fresh detection
- [ ] Cache survives concurrent access

### Unified Worktrees
- [ ] `blue realm worktree` creates worktrees in all domain repos
- [ ] `blue realm pr` creates linked PRs with merge order
- [ ] Exporters ordered before importers

### Conflict Resolution
- [ ] Concurrent syncs handled via git rebase
- [ ] Ownership violations rejected
- [ ] Semver incompatibility flagged as error
- [ ] Stale bindings flagged as warning

---

## Future Work

1. **Desktop GUI** - Native app for realm management and notifications
2. **Sophisticated contract detection** - Parse Python/TypeScript/etc. to auto-detect exports (tree-sitter)
3. **Signature verification** - Repos sign their exports
4. **Multiple realms** - One repo participates in multiple realms
5. **Cross-realm imports** - Import from domain in different realm
6. **Public registry** - Discover realms and contracts
7. **Infrastructure verification** - Check actual AWS state matches contracts
8. **Domain-level governance** - Override realm governance per domain
9. **GitHub/GitLab support** - Alternative to Forgejo for external users
10. **Multi-user daemon** - Support multiple users on shared machines

---

## References

- [Spike: cross-repo-coordination](../spikes/cross-repo-coordination.md)
- [Dialogue: cross-repo-realms](../dialogues/cross-repo-realms.dialogue.md)
- [Refinement: cross-repo-realms-refinement](../dialogues/cross-repo-realms-refinement.dialogue.md)
