# RFC 0001: Cross-Repo Coordination with Realms

| | |
|---|---|
| **Status** | Draft |
| **Created** | 2026-01-24 |
| **Source** | [Spike: cross-repo-coordination](../spikes/cross-repo-coordination.md) |
| **Dialogue** | [cross-repo-realms.dialogue.md](../dialogues/cross-repo-realms.dialogue.md) |

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

---

## Proposal

### Hierarchy

```
Index (~/.blue/index.yaml)
  └── Realm (git repo)
        └── Domain (coordination context)
              ├── Repo A (participant)
              └── Repo B (participant)
```

| Level | Purpose | Storage |
|-------|---------|---------|
| **Index** | List of realms user participates in | `~/.blue/index.yaml` |
| **Realm** | Groups related coordination domains | Git repository |
| **Domain** | Coordination context between repos | Directory in realm repo |
| **Repo** | Actual code repository (can participate in multiple domains) | `.blue/` directory |

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

### Realm Structure

A realm is a git repository:

```
realm-letemcook/
├── realm.yaml                        # Realm metadata and governance
├── repos/                            # Registry of all participating repos
│   ├── aperture.yaml                 # Repo metadata (path, maintainers)
│   └── fungal-image-analysis.yaml
├── domains/
│   ├── s3-access/                    # A coordination context
│   │   ├── domain.yaml               # What is being coordinated
│   │   ├── contracts/
│   │   │   └── s3-permissions.yaml   # The contract schema + value
│   │   └── bindings/
│   │       ├── aperture.yaml         # aperture exports this contract
│   │       └── fungal.yaml           # fungal imports this contract
│   │
│   └── training-pipeline/            # Another coordination context
│       ├── domain.yaml
│       ├── contracts/
│       │   └── job-schema.yaml
│       └── bindings/
│           ├── fungal.yaml           # fungal exports here
│           └── ml-infra.yaml         # ml-infra imports here
└── .github/
    └── CODEOWNERS
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
```

### repos/{repo}.yaml

Each participating repo is registered:

```yaml
# repos/aperture.yaml
name: aperture
org: cultivarium  # Optional org prefix
path: /Users/ericg/letemcook/aperture
# Or remote:
# url: git@github.com:cultivarium/aperture.git

maintainers:
  - eric@example.com

joined_at: 2026-01-24T10:00:00Z
```

```yaml
# repos/fungal-image-analysis.yaml
name: fungal-image-analysis
org: fungal-org
path: /Users/ericg/letemcook/fungal-image-analysis

maintainers:
  - fungal-maintainer@example.com

joined_at: 2026-01-24T10:00:00Z
```

### domains/{domain}/domain.yaml

Describes what is being coordinated:

```yaml
# domains/s3-access/domain.yaml
name: s3-access
description: Coordinates S3 bucket access between aperture and fungal

created_at: 2026-01-24T10:00:00Z

# Which repos participate in this coordination
members:
  - aperture
  - fungal-image-analysis
```

### domains/{domain}/contracts/{contract}.yaml

The actual contract being coordinated:

```yaml
# domains/s3-access/contracts/s3-permissions.yaml
name: s3-permissions
version: "1.3.0"
description: S3 paths that need cross-account access

schema:
  type: object
  properties:
    read:
      type: array
      items: { type: string }
    write:
      type: array
      items: { type: string }

value:
  read:
    - "jobs/*/masks/*"
    - "jobs/*/*/config.json"
    - "jobs/*/*/manifest.json"
    - "training-runs/*"
    - "training-metrics/*"
  write:
    - "jobs/*/*/manifest.json"
    - "training-metrics/*"

changelog:
  - version: "1.3.0"
    date: 2026-01-24
    changes:
      - "Added training-metrics/* for experiment tracking"
```

### domains/{domain}/bindings/{repo}.yaml

How each repo relates to the contracts in this domain:

```yaml
# domains/s3-access/bindings/aperture.yaml
repo: aperture
role: provider  # This repo defines/exports the contract

exports:
  - contract: s3-permissions
    source_files:
      - models/training/s3_paths.py
      - models/shared/data/parquet_export.py
```

```yaml
# domains/s3-access/bindings/fungal.yaml
repo: fungal-image-analysis
role: consumer  # This repo implements/imports the contract

imports:
  - contract: s3-permissions
    version: ">=1.0.0"
    binding: cdk/training_tools_access_stack.py
    status: current
    resolved_version: "1.3.0"
    resolved_at: 2026-01-24T12:00:00Z
```

### Local Configuration

Each repo stores its realm participation:

```yaml
# aperture/.blue/config.yaml
realm:
  name: letemcook
  path: ../realm-letemcook

repo: aperture

# Domains this repo participates in (populated by realm join/sync)
domains:
  - name: s3-access
    role: provider
    contracts:
      - s3-permissions
```

### Index File

```yaml
# ~/.blue/index.yaml
realms:
  - name: letemcook
    path: /Users/ericg/letemcook/realm-letemcook

  - name: ml-infra
    url: git@github.com:org/realm-ml-infra.git
    local_path: /Users/ericg/.blue/realms/ml-infra
```

### Session Coordination (IPC/Socket)

Blue MCP servers communicate directly for real-time session awareness.

**Architecture:**

```
┌─────────────────────┐     IPC Socket      ┌─────────────────────┐
│  Blue MCP Server    │◄──────────────────►│  Blue MCP Server    │
│  (aperture)         │                     │  (fungal)           │
│                     │                     │                     │
│  Socket: /tmp/blue/ │                     │  Socket: /tmp/blue/ │
│    aperture.sock    │                     │    fungal.sock      │
└─────────────────────┘                     └─────────────────────┘
           │                                           │
           └──────────────┬────────────────────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │  Realm Session Index  │
              │  /tmp/blue/letemcook/ │
              │    sessions.json      │
              └───────────────────────┘
```

**Session Registration:**

```json
// /tmp/blue/letemcook/sessions.json
{
  "sessions": [
    {
      "repo": "aperture",
      "socket": "/tmp/blue/aperture.sock",
      "pid": 12345,
      "started_at": "2026-01-24T10:00:00Z",
      "active_rfc": "training-metrics-v2",
      "active_domains": ["s3-access"],
      "exports_modified": ["s3-permissions"]
    },
    {
      "repo": "fungal-image-analysis",
      "socket": "/tmp/blue/fungal.sock",
      "pid": 12346,
      "started_at": "2026-01-24T10:05:00Z",
      "active_rfc": null,
      "active_domains": ["s3-access"],
      "imports_watching": ["s3-permissions"]
    }
  ]
}
```

**Real-time Notifications:**

When aperture modifies an export, it broadcasts to all sessions watching that export:

```rust
// Session broadcast
fn notify_export_change(export_name: &str, changes: &ExportChanges) {
    let sessions = load_realm_sessions();
    for session in sessions.watching(export_name) {
        send_ipc_message(&session.socket, Message::ExportChanged {
            from_repo: self.repo.clone(),
            contract: export_name.to_string(),
            changes: changes.clone(),
        });
    }
}
```

**Receiving Notifications:**

```bash
# In fungal-image-analysis terminal (real-time)
$ blue status
📊 fungal-image-analysis (in realm: letemcook)

🔔 Live notification from repo 'aperture':
   Contract 's3-permissions' in domain 's3-access' changed:
   + training-metrics/experiments/*

   Your import is now stale.
   Run 'blue realm worktree --domain s3-access' to start coordinated changes.
```

### Unified Worktrees (Cross-Repo Branches)

When changes span multiple repos, create worktrees in all affected repos simultaneously with a shared branch name.

**Workflow:**

```bash
# In aperture: start cross-repo change
$ blue realm worktree --rfc training-metrics-v2

Creating unified worktree 'feat/training-metrics-v2':
  ✓ aperture: .worktrees/feat-training-metrics-v2/
  ✓ fungal-image-analysis: .worktrees/feat-training-metrics-v2/

Branch 'feat/training-metrics-v2' created in both repos.
You are now in aperture worktree.

Related worktree:
  cd /Users/ericg/letemcook/fungal-image-analysis/.worktrees/feat-training-metrics-v2
```

**Realm Worktree Tracking:**

```yaml
# realm-letemcook/worktrees/feat-training-metrics-v2.yaml
name: feat/training-metrics-v2
created_at: 2026-01-24T10:00:00Z
source_rfc: aperture:training-metrics-v2
domain: s3-access  # The coordination context this change affects

repos:
  - name: aperture
    path: /Users/ericg/letemcook/aperture/.worktrees/feat-training-metrics-v2
    status: active
    commits: 3

  - name: fungal-image-analysis
    path: /Users/ericg/letemcook/fungal-image-analysis/.worktrees/feat-training-metrics-v2
    status: active
    commits: 1

coordination:
  # Commits are linked across repos
  commits:
    - aperture: abc1234
      fungal-image-analysis: null
      message: "feat: add experiment metrics export"

    - aperture: def5678
      fungal-image-analysis: ghi9012
      message: "feat: update IAM for experiment metrics"
      linked: true  # These commits are coordinated
```

**Coordinated Commits:**

```bash
# After making changes in both worktrees
$ blue realm commit -m "feat: add experiment metrics with IAM support"

Coordinated commit across repos in domain 's3-access':
  aperture:
    ✓ Committed: abc1234
    Files: models/training/metrics_exporter.py

  fungal-image-analysis:
    ✓ Committed: def5678
    Files: cdk/training_tools_access_stack.py

Commits linked in realm worktree tracking.

$ blue realm push
Pushing coordinated branches:
  ✓ aperture: feat/training-metrics-v2 → origin
  ✓ fungal-image-analysis: feat/training-metrics-v2 → origin

Ready for coordinated PRs. Run 'blue realm pr' to create.
```

**Coordinated PRs:**

```bash
$ blue realm pr

Creating coordinated pull requests:

aperture:
  ✓ PR #45: "feat: add experiment metrics export"
    Base: main
    Links to: fungal-image-analysis#23

fungal-image-analysis:
  ✓ PR #23: "feat: IAM policy for experiment metrics"
    Base: main
    Links to: aperture#45
    Blocked by: aperture#45 (merge aperture first)

PRs are linked. Merge order: aperture#45 → fungal-image-analysis#23
```

---

## Workflow

### Initial Setup

```bash
# 1. Create realm (one-time)
$ mkdir realm-letemcook && cd realm-letemcook
$ blue realm init --name letemcook
✓ Created realm.yaml
✓ Initialized git repository

# 2. Register aperture in realm
$ cd ../aperture
$ blue realm join ../realm-letemcook
✓ Created repos/aperture.yaml
✓ Auto-detected exports: s3-permissions
✓ Updated .blue/config.yaml

# 3. Create the s3-access domain (coordination context)
$ blue realm domain create s3-access --repos aperture,fungal-image-analysis
✓ Created domains/s3-access/domain.yaml
✓ Created domains/s3-access/bindings/aperture.yaml (provider)
✓ Created domains/s3-access/bindings/fungal-image-analysis.yaml (consumer)

# 4. Register fungal in realm
$ cd ../fungal-image-analysis
$ blue realm join ../realm-letemcook
✓ Created repos/fungal-image-analysis.yaml
✓ Detected import: s3-permissions in domain s3-access
✓ Updated .blue/config.yaml
```

### Daily Development

```bash
# Developer in aperture adds new S3 path
$ cd aperture
$ vim models/training/metrics_exporter.py
# Added: s3://bucket/training-metrics/experiments/*

# Blue status shows cross-realm impact
$ blue status
📊 aperture (in realm: letemcook)

RFCs:
  training-metrics-v2 [in-progress] 3/5 tasks

⚠️  Cross-repo change detected:
   Domain 's3-access' contract 's3-permissions' has local changes:
   + training-metrics/experiments/*

   Affected repos:
   - fungal-image-analysis (imports >=1.0.0 of s3-permissions)

   Run 'blue realm sync' to update realm

# Sync changes to realm
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Contracts updated in domain 's3-access':
  s3-permissions: 1.3.0 → 1.4.0
  + training-metrics/experiments/*

Notifying affected repos:
  fungal-image-analysis:
    ✓ Created GitHub issue #42
      "Update IAM policy: new S3 path training-metrics/experiments/*"

✓ Realm synced (commit abc1234)
```

### Consumer Response

```bash
# Maintainer in fungal sees notification
$ cd fungal-image-analysis
$ blue status
📊 fungal-image-analysis (in realm: letemcook)

⚠️  Stale imports in domain 's3-access':
   s3-permissions: 1.3.0 → 1.4.0 available
   Binding: cdk/training_tools_access_stack.py

   Changes:
   + training-metrics/experiments/* (read/write)

# Update the IAM policy
$ vim cdk/training_tools_access_stack.py
# Add new path to policy

# Mark import as resolved
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Imports resolved in domain 's3-access':
  s3-permissions: now at 1.4.0

✓ Realm synced (commit def5678)
```

---

## New Tools

### blue_realm_init

Create a new realm.

```
blue_realm_init
  --name: Realm name (kebab-case)
  --path: Where to create realm repo (default: current directory)
```

### blue_realm_join

Register a repository in a realm.

```
blue_realm_join
  realm_path: Path to realm repo
  --name: Repo name in realm (default: repo directory name)
  --detect-exports: Auto-detect exports (default: true)
```

### blue_realm_domain

Create or manage a coordination domain.

```
blue_realm_domain
  create: Create a new domain
  --name: Domain name (kebab-case)
  --repos: Comma-separated list of participating repos
  --contract: Initial contract name
```

### blue_realm_leave

Remove a repository from a realm.

```
blue_realm_leave
  --force: Leave even if other repos depend on our exports
```

### blue_realm_export

Declare or update an export.

```
blue_realm_export
  --name: Export name
  --version: Semantic version
  --value: JSON value (or --file for YAML file)
  --detect: Auto-detect from code patterns
```

### blue_realm_import

Declare an import dependency on a contract.

```
blue_realm_import
  --contract: Contract name
  --domain: Domain containing the contract
  --version: Version requirement (semver)
  --binding: Local file that uses this import
```

### blue_realm_sync

Synchronize local state with realm.

```
blue_realm_sync
  --dry-run: Show what would change without committing
  --notify: Create GitHub issues for affected consumers (default: true)
```

### blue_realm_check

Check realm status without syncing.

```
blue_realm_check
  --exports: Check if local exports have changed
  --imports: Check if any imports are stale
```

### blue_realm_graph

Display the dependency graph.

```
blue_realm_graph
  --format: text | mermaid | dot
```

### blue_realm_sessions

List active Blue sessions in the realm.

```
blue_realm_sessions
  --watch: Continuously update (real-time)
```

### blue_realm_worktree

Create unified worktrees across affected repos.

```
blue_realm_worktree
  --rfc: RFC title that drives the change
  --branch: Branch name (default: feat/{rfc-title})
  --domain: Domain to coordinate (default: auto-detect from exports)
  --repos: Specific repos to include (default: all in domain)
```

### blue_realm_commit

Create coordinated commits across realm worktrees.

```
blue_realm_commit
  -m: Commit message (applied to all repos)
  --repos: Which repos to commit (default: all with changes)
  --link: Link commits in realm tracking (default: true)
```

### blue_realm_push

Push coordinated branches to remotes.

```
blue_realm_push
  --repos: Which repos to push (default: all in worktree)
```

### blue_realm_pr

Create linked pull requests across repos.

```
blue_realm_pr
  --title: PR title (applied to all)
  --body: PR body template
  --draft: Create as draft PRs
```

---

## Implementation

### Phase Overview

| Phase | Scope | Duration |
|-------|-------|----------|
| 0 | Data model in blue-core | 1 week |
| 1 | Realm init | 1 week |
| 2 | Domain join | 1 week |
| 3 | Status integration | 1 week |
| 4 | Sync & notifications | 2 weeks |
| 5 | **Session coordination (IPC)** | 2 weeks |
| 6 | **Unified worktrees** | 2 weeks |
| 7 | **Coordinated commits & PRs** | 2 weeks |
| 8 | Polish & docs | 1 week |

**Total:** 13 weeks (Phases 0-4 for basic functionality, 5-7 for full coordination)

### Phase 0: Data Model (Week 1)

Add to `blue-core/src/`:

```rust
// realm.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealmConfig {
    pub name: String,
    pub version: String,
    pub governance: Governance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Governance {
    pub admission: AdmissionPolicy,
    pub approvers: Vec<String>,
    pub breaking_changes: BreakingChangePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdmissionPolicy {
    Open,
    Approval,
    InviteOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingChangePolicy {
    pub require_approval: bool,
    pub grace_period_days: u32,
}

/// A registered repository in the realm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealmRepo {
    pub name: String,
    pub org: Option<String>,
    pub path: Option<PathBuf>,
    pub url: Option<String>,
    pub maintainers: Vec<String>,
    pub joined_at: String,
}

/// A coordination context (edge between repos)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<String>,  // repo names
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Export {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub schema: Option<serde_json::Value>,
    pub value: serde_json::Value,
    pub changelog: Vec<ChangelogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub version: String,
    pub date: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub contract: String,
    pub domain: String,    // which domain contains this contract
    pub version: String,   // semver requirement
    pub binding: String,   // local file that uses this
    pub status: ImportStatus,
    pub resolved_version: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportStatus {
    Current,
    Stale,
    Broken,
}
```

### Phase 1: Realm Init (Week 2)

```rust
// handlers/realm.rs

pub fn handle_init(args: &Value) -> Result<Value, ServerError> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let path = args.get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Create realm directory
    let realm_path = path.join(format!("realm-{}", name));
    fs::create_dir_all(&realm_path)?;

    // Create realm.yaml
    let config = RealmConfig {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        governance: Governance {
            admission: AdmissionPolicy::Approval,
            approvers: vec![],
            breaking_changes: BreakingChangePolicy {
                require_approval: true,
                grace_period_days: 14,
            },
        },
    };

    let yaml = serde_yaml::to_string(&config)?;
    fs::write(realm_path.join("realm.yaml"), yaml)?;

    // Create directories
    fs::create_dir_all(realm_path.join("domains"))?;
    fs::create_dir_all(realm_path.join("contracts"))?;

    // Initialize git
    Command::new("git")
        .args(["init"])
        .current_dir(&realm_path)
        .output()?;

    Ok(json!({
        "status": "success",
        "message": format!("Created realm '{}' at {}", name, realm_path.display()),
        "path": realm_path.display().to_string()
    }))
}
```

### Phase 2: Repo Registration (Week 3)

```rust
pub fn handle_join(args: &Value, repo_path: &Path) -> Result<Value, ServerError> {
    let realm_path = args.get("realm_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or(ServerError::InvalidParams)?;

    let repo_name = args.get("name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            repo_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    // Validate realm exists
    let realm_yaml = realm_path.join("realm.yaml");
    if !realm_yaml.exists() {
        return Err(ServerError::NotFound("Realm not found".into()));
    }

    // Create repos directory if needed
    let repos_dir = realm_path.join("repos");
    fs::create_dir_all(&repos_dir)?;

    // Register repo in realm
    let repo = RealmRepo {
        name: repo_name.clone(),
        org: None,
        path: Some(repo_path.to_path_buf()),
        url: None,
        maintainers: vec![],
        joined_at: chrono::Utc::now().to_rfc3339(),
    };
    fs::write(
        repos_dir.join(format!("{}.yaml", repo_name)),
        serde_yaml::to_string(&repo)?
    )?;

    // Auto-detect exports (stored temporarily until domain is created)
    let exports = detect_exports(repo_path)?;

    // Update repo's .blue/config.yaml
    let blue_dir = repo_path.join(".blue");
    fs::create_dir_all(&blue_dir)?;

    let config = json!({
        "realm": {
            "name": realm_name,
            "path": realm_path.display().to_string(),
        },
        "repo": repo_name,
        "detected_exports": exports,
    });
    fs::write(blue_dir.join("config.yaml"), serde_yaml::to_string(&config)?)?;

    // Commit to realm
    Command::new("git")
        .args(["add", "."])
        .current_dir(&realm_path)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", &format!("Register repo: {}", repo_name)])
        .current_dir(&realm_path)
        .output()?;

    Ok(json!({
        "status": "success",
        "message": format!("Registered '{}' in realm", repo_name),
        "exports_detected": exports.len(),
        "next_step": "Run 'blue realm domain create' to create a coordination domain"
    }))
}
```

### Phase 3: Status Integration (Week 4)

Modify `handle_status` to include realm information:

```rust
fn get_realm_status(state: &ProjectState) -> Option<Value> {
    let config_path = state.repo_path.join(".blue/config.yaml");
    let config: Value = serde_yaml::from_str(
        &fs::read_to_string(&config_path).ok()?
    ).ok()?;

    let realm_path = config["realm"]["path"].as_str()?;
    let repo_name = config["repo"].as_str()?;

    // Find domains this repo participates in
    let domains = find_repo_domains(realm_path, repo_name).ok()?;

    // Check for local export changes in each domain
    let local_exports = detect_exports(&state.repo_path).ok()?;
    let mut export_changes = vec![];
    let mut stale_imports = vec![];

    for domain in &domains {
        let declared = load_domain_exports(realm_path, domain, repo_name).ok()?;
        export_changes.extend(diff_exports(&local_exports, &declared, domain));

        let imports = load_domain_imports(realm_path, domain, repo_name).ok()?;
        stale_imports.extend(check_import_staleness(realm_path, domain, &imports));
    }

    Some(json!({
        "realm": config["realm"]["name"],
        "repo": repo_name,
        "domains": domains,
        "export_changes": export_changes,
        "stale_imports": stale_imports
    }))
}
```

### Phase 4: Sync (Weeks 5-6)

```rust
pub fn handle_sync(args: &Value, state: &ProjectState) -> Result<Value, ServerError> {
    let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
    let notify = args.get("notify").and_then(|v| v.as_bool()).unwrap_or(true);

    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.realm_path);
    let repo_name = &config.repo;

    // Pull latest realm state
    git_pull(&realm_path)?;

    // Find domains this repo participates in
    let domains = find_repo_domains(&realm_path, repo_name)?;

    // Detect export changes across all domains
    let local_exports = detect_exports(&state.repo_path)?;
    let mut all_changes = vec![];

    for domain in &domains {
        let declared = load_domain_exports(&realm_path, domain, repo_name)?;
        let changes = diff_exports(&local_exports, &declared);
        if !changes.is_empty() {
            all_changes.push((domain.clone(), changes));
        }
    }

    if all_changes.is_empty() {
        return Ok(json!({
            "status": "success",
            "message": "No changes to sync"
        }));
    }

    if dry_run {
        return Ok(json!({
            "status": "dry_run",
            "changes": all_changes
        }));
    }

    // Update exports in each affected domain
    let mut notifications = vec![];
    for (domain, changes) in &all_changes {
        let contract = &changes[0].contract;
        let new_version = bump_version(&changes[0].old_version, changes);
        save_domain_export(&realm_path, domain, repo_name, contract, &new_version)?;

        // Find affected repos in this domain
        let affected_repos = find_domain_consumers(&realm_path, domain, contract)?;

        if notify {
            for affected_repo in &affected_repos {
                let issue = create_github_issue(affected_repo, domain, changes)?;
                notifications.push(issue);
            }
        }
    }

    // Commit and push
    git_commit(&realm_path, &format!(
        "{}: sync exports from {}",
        all_changes.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>().join(", "),
        repo_name
    ))?;
    git_push(&realm_path)?;

    Ok(json!({
        "status": "success",
        "message": format!("Synced changes in {} domain(s)", all_changes.len()),
        "notifications": notifications
    }))
}
```

### Phase 5: Session Coordination (Weeks 8-9)

**IPC Socket Infrastructure:**

```rust
// blue-core/src/ipc.rs

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;

pub struct BlueIpcServer {
    socket_path: PathBuf,
    listener: UnixListener,
    realm: String,
    repo: String,
}

impl BlueIpcServer {
    pub fn start(realm: &str, repo: &str) -> Result<Self, IpcError> {
        let socket_path = PathBuf::from(format!("/tmp/blue/{}.sock", repo));
        std::fs::create_dir_all("/tmp/blue")?;

        let listener = UnixListener::bind(&socket_path)?;

        // Register in realm session index
        register_session(realm, repo, &socket_path)?;

        Ok(Self { socket_path, listener, realm: realm.to_string(), repo: repo.to_string() })
    }

    pub fn broadcast_contract_change(&self, domain: &str, contract: &str, changes: &ContractChanges) {
        let sessions = load_realm_sessions(&self.realm);
        for session in sessions.watching_contract(domain, contract) {
            if let Ok(mut stream) = UnixStream::connect(&session.socket) {
                let msg = IpcMessage::ContractChanged {
                    from_repo: self.repo.clone(),
                    domain: domain.to_string(),
                    contract: contract.to_string(),
                    changes: changes.clone(),
                };
                serde_json::to_writer(&mut stream, &msg).ok();
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum IpcMessage {
    ContractChanged { from_repo: String, domain: String, contract: String, changes: ContractChanges },
    SessionStarted { repo: String, rfc: Option<String> },
    SessionEnded { repo: String },
    Ping,
    Pong,
}
```

**Session Index:**

```rust
// blue-core/src/realm_sessions.rs

pub fn register_session(realm: &str, repo: &str, socket: &Path) -> Result<(), Error> {
    let index_path = PathBuf::from(format!("/tmp/blue/{}/sessions.json", realm));
    std::fs::create_dir_all(index_path.parent().unwrap())?;

    let mut sessions = load_sessions(&index_path)?;
    sessions.push(Session {
        repo: repo.to_string(),
        socket: socket.to_path_buf(),
        pid: std::process::id(),
        started_at: chrono::Utc::now(),
        active_rfc: None,
        active_domains: vec![],
        exports_modified: vec![],
        imports_watching: vec![],
    });

    save_sessions(&index_path, &sessions)
}

pub fn unregister_session(realm: &str, repo: &str) -> Result<(), Error> {
    let index_path = PathBuf::from(format!("/tmp/blue/{}/sessions.json", realm));
    let mut sessions = load_sessions(&index_path)?;
    sessions.retain(|s| s.repo != repo);
    save_sessions(&index_path, &sessions)
}
```

### Phase 6: Unified Worktrees (Weeks 10-11)

```rust
// handlers/realm_worktree.rs

pub fn handle_realm_worktree(args: &Value, state: &ProjectState) -> Result<Value, ServerError> {
    let rfc_title = args.get("rfc").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let branch = args.get("branch").and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("feat/{}", rfc_title));

    let domain_filter = args.get("domain").and_then(|v| v.as_str());

    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.realm_path);
    let repo_name = &config.repo;

    // Find domains this repo participates in (or use filter)
    let domains = match domain_filter {
        Some(d) => vec![d.to_string()],
        None => find_repo_domains(&realm_path, repo_name)?,
    };

    // Find all repos affected by changes in these domains
    let mut affected_repos = HashSet::new();
    affected_repos.insert(repo_name.clone());

    for domain in &domains {
        let domain_repos = get_domain_members(&realm_path, domain)?;
        affected_repos.extend(domain_repos);
    }

    // Create worktree in our repo
    let our_worktree = create_worktree(&state.repo_path, &branch)?;

    // Create worktrees in affected repos
    let mut worktrees = vec![WorktreeInfo {
        repo: repo_name.clone(),
        path: our_worktree.clone(),
        status: "active".to_string(),
    }];

    for repo in &affected_repos {
        if repo == repo_name { continue; }  // Skip ourselves
        let repo_config = load_realm_repo(&realm_path, repo)?;
        if let Some(repo_path) = &repo_config.path {
            let wt = create_worktree(repo_path, &branch)?;
            worktrees.push(WorktreeInfo {
                repo: repo.clone(),
                path: wt,
                status: "active".to_string(),
            });
        }
    }

    // Track in realm
    let tracking = RealmWorktree {
        name: branch.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        source_rfc: format!("{}:{}", repo_name, rfc_title),
        domains: domains.clone(),
        repos: worktrees.clone(),
        commits: vec![],
    };

    save_realm_worktree(&realm_path, &tracking)?;

    Ok(json!({
        "status": "success",
        "message": format!("Created unified worktree '{}'", branch),
        "branch": branch,
        "domains": domains,
        "worktrees": worktrees
    }))
}
```

### Phase 7: Coordinated Commits & PRs (Weeks 12-13)

```rust
// handlers/realm_commit.rs

pub fn handle_realm_commit(args: &Value, state: &ProjectState) -> Result<Value, ServerError> {
    let message = args.get("m").and_then(|v| v.as_str())
        .ok_or(ServerError::InvalidParams)?;

    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.realm_path);

    // Find current worktree tracking
    let branch = get_current_branch(&state.repo_path)?;
    let tracking = load_realm_worktree(&realm_path, &branch)?;

    let mut commits = vec![];

    // Commit in each repo that has changes
    for wt in &tracking.repos {
        if has_uncommitted_changes(&wt.path)? {
            let commit = git_commit(&wt.path, message)?;
            commits.push(LinkedCommit {
                repo: wt.repo.clone(),
                sha: commit,
                message: message.to_string(),
            });
        }
    }

    // Update realm tracking
    let mut tracking = tracking;
    tracking.commits.push(CoordinatedCommit {
        commits: commits.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    save_realm_worktree(&realm_path, &tracking)?;

    Ok(json!({
        "status": "success",
        "message": format!("Committed in {} repo(s)", commits.len()),
        "commits": commits
    }))
}

// handlers/realm_pr.rs

pub fn handle_realm_pr(args: &Value, state: &ProjectState) -> Result<Value, ServerError> {
    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.realm_path);

    let branch = get_current_branch(&state.repo_path)?;
    let tracking = load_realm_worktree(&realm_path, &branch)?;

    let title = args.get("title").and_then(|v| v.as_str())
        .unwrap_or(&branch);

    let mut prs = vec![];

    // Determine merge order (repos that export should merge before repos that import)
    let ordered_repos = topological_sort_repos(&realm_path, &tracking.domains, &tracking.repos)?;

    for (i, wt) in ordered_repos.iter().enumerate() {
        let repo_config = load_realm_repo(&realm_path, &wt.repo)?;
        if let Some(repo_path) = &repo_config.path {
            // Create PR with links to other PRs
            let body = generate_pr_body(&tracking, &prs, i)?;
            let pr = create_github_pr(repo_path, &branch, title, &body)?;

            prs.push(LinkedPR {
                repo: wt.repo.clone(),
                number: pr.number,
                url: pr.url,
                blocked_by: if i > 0 { Some(prs[i-1].clone()) } else { None },
            });
        }
    }

    Ok(json!({
        "status": "success",
        "message": format!("Created {} linked PR(s)", prs.len()),
        "prs": prs,
        "merge_order": ordered_repos.iter().map(|r| &r.repo).collect::<Vec<_>>()
    }))
}
```

### Phase 8: Polish (Week 14)

- Error handling for all edge cases
- User-friendly messages in Blue's voice
- Documentation
- Tests
- Socket cleanup on process exit

---

## Test Plan

### Basic Realm Operations
- [ ] `blue realm init` creates valid realm structure
- [ ] `blue realm join` registers repo correctly in repos/ directory
- [ ] `blue realm join` auto-detects potential exports
- [ ] `blue realm domain create` creates domain with contracts and bindings
- [ ] `blue status` shows realm state and participating domains
- [ ] `blue status` detects local contract changes
- [ ] `blue status` shows stale imports across domains
- [ ] `blue realm sync` updates contracts in affected domains
- [ ] `blue realm sync` creates GitHub issues for affected repos
- [ ] `blue realm sync --dry-run` shows changes without committing
- [ ] Multiple repos can coordinate through a domain
- [ ] Breaking changes are flagged appropriately
- [ ] CODEOWNERS prevents cross-repo writes to domain bindings

### Session Coordination
- [ ] IPC socket created on Blue start
- [ ] Session registered in realm index with repo name
- [ ] Contract changes broadcast to watching sessions
- [ ] Sessions cleaned up on process exit
- [ ] `blue realm sessions` lists active sessions by repo

### Unified Worktrees
- [ ] `blue realm worktree` creates worktrees in all affected repos in domain
- [ ] Worktree tracked in realm repo with domain reference
- [ ] `blue realm commit` commits in all repos with changes
- [ ] Commits linked in realm tracking
- [ ] `blue realm push` pushes all branches
- [ ] `blue realm pr` creates linked PRs with correct merge order (exporters first)

---

## Future Work

1. **Signature verification** - Domains sign their exports
2. **Multiple realms** - One repo participates in multiple realms
3. **Cross-realm imports** - Import from domain in different realm
4. **Public registry** - Discover realms and contracts
5. **Infrastructure verification** - Check actual AWS state matches exports

---

## References

- [Spike: cross-repo-coordination](../spikes/cross-repo-coordination.md)
- [Dialogue: cross-repo-realms](../dialogues/cross-repo-realms.dialogue.md)
