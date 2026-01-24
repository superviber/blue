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
- Monorepo support (one repo = one domain for MVP)

---

## Proposal

### Hierarchy

```
Index (~/.blue/index.yaml)
  └── Realm (git repo)
        └── Domain (directory in realm)
              └── Repo (.blue/ in code repo)
```

| Level | Purpose | Storage |
|-------|---------|---------|
| **Index** | List of realms user participates in | `~/.blue/index.yaml` |
| **Realm** | Federation of related domains | Git repository |
| **Domain** | Single org's presence in a realm | Directory in realm repo |
| **Repo** | Actual code repository | `.blue/` directory |

### Realm Structure

A realm is a git repository:

```
realm-letemcook/
├── realm.yaml                     # Realm metadata and governance
├── domains/
│   ├── aperture/
│   │   ├── domain.yaml           # Domain metadata
│   │   ├── exports.yaml          # What this domain provides
│   │   └── imports.yaml          # What this domain consumes
│   └── fungal-image-analysis/
│       ├── domain.yaml
│       ├── exports.yaml
│       └── imports.yaml
├── contracts/
│   └── s3-paths.schema.yaml      # Shared schema definitions
└── .github/
    └── CODEOWNERS                # Domain isolation
```

### realm.yaml

```yaml
name: letemcook
version: "1.0.0"
created_at: 2026-01-24T10:00:00Z

governance:
  # Who can add new domains?
  admission: approval  # open | approval | invite-only
  approvers:
    - eric@example.com

  # Breaking change policy
  breaking_changes:
    require_approval: true
    grace_period_days: 14
```

### domain.yaml

```yaml
name: aperture
repo_path: /Users/ericg/letemcook/aperture
# Or for remote:
# repo_url: git@github.com:cultivarium/aperture.git

maintainers:
  - eric@example.com

joined_at: 2026-01-24T10:00:00Z
```

### exports.yaml

```yaml
exports:
  - name: required-s3-permissions
    version: "1.3.0"
    description: S3 paths that aperture training code needs to access

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
      - version: "1.2.0"
        date: 2026-01-20
        changes:
          - "Added write permissions for manifest updates"
```

### imports.yaml

```yaml
imports:
  - contract: required-s3-permissions
    from: aperture
    version: ">=1.0.0"

    binding: cdk/training_tools_access_stack.py

    status: current  # current | stale | broken
    resolved_version: "1.3.0"
    resolved_at: 2026-01-24T12:00:00Z
```

### Local Configuration

Each repo stores its realm membership:

```yaml
# aperture/.blue/config.yaml
realm:
  name: letemcook
  path: ../realm-letemcook  # Relative path to realm repo
  domain: aperture
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
      "domain": "aperture",
      "socket": "/tmp/blue/aperture.sock",
      "pid": 12345,
      "started_at": "2026-01-24T10:00:00Z",
      "active_rfc": "training-metrics-v2",
      "exports_modified": ["required-s3-permissions"]
    },
    {
      "domain": "fungal-image-analysis",
      "socket": "/tmp/blue/fungal.sock",
      "pid": 12346,
      "started_at": "2026-01-24T10:05:00Z",
      "active_rfc": null,
      "imports_watching": ["required-s3-permissions"]
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
            from: self.domain,
            export: export_name,
            changes: changes.clone(),
        });
    }
}
```

**Receiving Notifications:**

```bash
# In fungal-image-analysis terminal (real-time)
$ blue status
📊 fungal-image-analysis

🔔 Live notification from aperture:
   Export 'required-s3-permissions' changed:
   + training-metrics/experiments/*

   Your import is now stale.
   Run 'blue realm worktree' to start coordinated changes.
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

domains:
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

Coordinated commit across realm:
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

# 2. Add aperture to realm
$ cd ../aperture
$ blue realm join ../realm-letemcook --as aperture
✓ Created domains/aperture/domain.yaml
✓ Auto-detected exports: required-s3-permissions
✓ Created domains/aperture/exports.yaml
✓ Updated .blue/config.yaml

# 3. Add fungal to realm
$ cd ../fungal-image-analysis
$ blue realm join ../realm-letemcook --as fungal-image-analysis
✓ Created domains/fungal-image-analysis/domain.yaml
✓ Detected import: required-s3-permissions from aperture
✓ Created domains/fungal-image-analysis/imports.yaml
```

### Daily Development

```bash
# Developer in aperture adds new S3 path
$ cd aperture
$ vim models/training/metrics_exporter.py
# Added: s3://bucket/training-metrics/experiments/*

# Blue status shows cross-realm impact
$ blue status
📊 aperture (domain in letemcook realm)

RFCs:
  training-metrics-v2 [in-progress] 3/5 tasks

⚠️  Cross-realm change detected:
   Export 'required-s3-permissions' has local changes:
   + training-metrics/experiments/*

   Consumers:
   - fungal-image-analysis (imports >=1.0.0)

   Run 'blue realm sync' to update realm

# Sync changes to realm
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Exports updated:
  required-s3-permissions: 1.3.0 → 1.4.0
  + training-metrics/experiments/*

Notifying consumers:
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
📊 fungal-image-analysis (domain in letemcook realm)

⚠️  Stale imports:
   required-s3-permissions: 1.3.0 → 1.4.0 available
   Binding: cdk/training_tools_access_stack.py

   Changes:
   + training-metrics/experiments/* (read/write)

# Update the IAM policy
$ vim cdk/training_tools_access_stack.py
# Add new path to policy

# Mark import as resolved
$ blue realm sync
📤 Syncing with realm 'letemcook'...

Imports resolved:
  required-s3-permissions: now at 1.4.0

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

Join a repository to a realm as a domain.

```
blue_realm_join
  realm_path: Path to realm repo
  --as: Domain name (default: repo directory name)
  --detect-exports: Auto-detect exports (default: true)
```

### blue_realm_leave

Remove domain from realm.

```
blue_realm_leave
  --force: Leave even if other domains import from us
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

Declare an import dependency.

```
blue_realm_import
  --contract: Contract name
  --from: Source domain
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
  --domains: Specific domains to include (default: all affected)
```

### blue_realm_commit

Create coordinated commits across realm worktrees.

```
blue_realm_commit
  -m: Commit message (applied to all repos)
  --domains: Which domains to commit (default: all with changes)
  --link: Link commits in realm tracking (default: true)
```

### blue_realm_push

Push coordinated branches to remotes.

```
blue_realm_push
  --domains: Which domains to push (default: all)
```

### blue_realm_pr

Create linked pull requests across repos.

```
blue_realm_pr
  --title: PR title (applied to all)
  --body: PR body template
  --draft: Create as draft PRs
```

```
blue_realm_graph
  --format: text | mermaid | dot
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub repo_path: Option<PathBuf>,
    pub repo_url: Option<String>,
    pub maintainers: Vec<String>,
    pub joined_at: String,
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
    pub from: String,
    pub version: String,  // semver requirement
    pub binding: String,
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

### Phase 2: Domain Join (Week 3)

```rust
pub fn handle_join(args: &Value, repo_path: &Path) -> Result<Value, ServerError> {
    let realm_path = args.get("realm_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or(ServerError::InvalidParams)?;

    let domain_name = args.get("as")
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

    // Create domain directory
    let domain_dir = realm_path.join("domains").join(&domain_name);
    fs::create_dir_all(&domain_dir)?;

    // Create domain.yaml
    let domain = Domain {
        name: domain_name.clone(),
        repo_path: Some(repo_path.to_path_buf()),
        repo_url: None,
        maintainers: vec![],
        joined_at: chrono::Utc::now().to_rfc3339(),
    };
    fs::write(
        domain_dir.join("domain.yaml"),
        serde_yaml::to_string(&domain)?
    )?;

    // Auto-detect exports
    let exports = detect_exports(repo_path)?;
    if !exports.is_empty() {
        fs::write(
            domain_dir.join("exports.yaml"),
            serde_yaml::to_string(&json!({ "exports": exports }))?
        )?;
    }

    // Update repo's .blue/config.yaml
    let blue_dir = repo_path.join(".blue");
    fs::create_dir_all(&blue_dir)?;

    let config = json!({
        "realm": {
            "name": realm_name,
            "path": realm_path.display().to_string(),
            "domain": domain_name
        }
    });
    fs::write(blue_dir.join("config.yaml"), serde_yaml::to_string(&config)?)?;

    // Commit to realm
    Command::new("git")
        .args(["add", "."])
        .current_dir(&realm_path)
        .output()?;

    Command::new("git")
        .args(["commit", "-m", &format!("Add domain: {}", domain_name)])
        .current_dir(&realm_path)
        .output()?;

    Ok(json!({
        "status": "success",
        "message": format!("Joined realm as '{}'", domain_name),
        "exports_detected": exports.len()
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
    let domain_name = config["realm"]["domain"].as_str()?;

    // Check for local export changes
    let local_exports = detect_exports(&state.repo_path).ok()?;
    let declared_exports = load_declared_exports(realm_path, domain_name).ok()?;

    let export_changes = diff_exports(&local_exports, &declared_exports);

    // Check for stale imports
    let imports = load_imports(realm_path, domain_name).ok()?;
    let stale_imports = check_import_staleness(realm_path, &imports);

    Some(json!({
        "realm": config["realm"]["name"],
        "domain": domain_name,
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
    let realm_path = PathBuf::from(&config.path);

    // Pull latest realm state
    git_pull(&realm_path)?;

    // Detect export changes
    let local_exports = detect_exports(&state.repo_path)?;
    let declared_exports = load_declared_exports(&realm_path, &config.domain)?;
    let changes = diff_exports(&local_exports, &declared_exports);

    if changes.is_empty() {
        return Ok(json!({
            "status": "success",
            "message": "No changes to sync"
        }));
    }

    if dry_run {
        return Ok(json!({
            "status": "dry_run",
            "changes": changes
        }));
    }

    // Update exports in realm
    let new_version = bump_version(&declared_exports[0].version, &changes);
    save_exports(&realm_path, &config.domain, &local_exports, &new_version)?;

    // Commit and push
    git_commit(&realm_path, &format!(
        "{}: export {}@{}",
        config.domain, local_exports[0].name, new_version
    ))?;
    git_push(&realm_path)?;

    // Find affected consumers
    let consumers = find_consumers(&realm_path, &local_exports[0].name)?;

    // Create notifications
    let mut notifications = vec![];
    if notify {
        for consumer in &consumers {
            let issue = create_github_issue(consumer, &changes)?;
            notifications.push(issue);
        }
    }

    Ok(json!({
        "status": "success",
        "message": format!("Synced {} export(s)", changes.len()),
        "new_version": new_version,
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
    domain: String,
}

impl BlueIpcServer {
    pub fn start(realm: &str, domain: &str) -> Result<Self, IpcError> {
        let socket_path = PathBuf::from(format!("/tmp/blue/{}.sock", domain));
        std::fs::create_dir_all("/tmp/blue")?;

        let listener = UnixListener::bind(&socket_path)?;

        // Register in realm session index
        register_session(realm, domain, &socket_path)?;

        Ok(Self { socket_path, listener, realm: realm.to_string(), domain: domain.to_string() })
    }

    pub fn broadcast_export_change(&self, export: &str, changes: &ExportChanges) {
        let sessions = load_realm_sessions(&self.realm);
        for session in sessions.watching(export) {
            if let Ok(mut stream) = UnixStream::connect(&session.socket) {
                let msg = IpcMessage::ExportChanged {
                    from: self.domain.clone(),
                    export: export.to_string(),
                    changes: changes.clone(),
                };
                serde_json::to_writer(&mut stream, &msg).ok();
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum IpcMessage {
    ExportChanged { from: String, export: String, changes: ExportChanges },
    SessionStarted { domain: String, rfc: Option<String> },
    SessionEnded { domain: String },
    Ping,
    Pong,
}
```

**Session Index:**

```rust
// blue-core/src/realm_sessions.rs

pub fn register_session(realm: &str, domain: &str, socket: &Path) -> Result<(), Error> {
    let index_path = PathBuf::from(format!("/tmp/blue/{}/sessions.json", realm));
    std::fs::create_dir_all(index_path.parent().unwrap())?;

    let mut sessions = load_sessions(&index_path)?;
    sessions.push(Session {
        domain: domain.to_string(),
        socket: socket.to_path_buf(),
        pid: std::process::id(),
        started_at: chrono::Utc::now(),
        active_rfc: None,
        exports_modified: vec![],
        imports_watching: vec![],
    });

    save_sessions(&index_path, &sessions)
}

pub fn unregister_session(realm: &str, domain: &str) -> Result<(), Error> {
    let index_path = PathBuf::from(format!("/tmp/blue/{}/sessions.json", realm));
    let mut sessions = load_sessions(&index_path)?;
    sessions.retain(|s| s.domain != domain);
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

    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.path);

    // Find affected domains (those that import from us)
    let our_exports = load_declared_exports(&realm_path, &config.domain)?;
    let affected = find_consumers(&realm_path, &our_exports)?;

    // Create worktree in our repo
    let our_worktree = create_worktree(&state.repo_path, &branch)?;

    // Create worktrees in affected repos
    let mut worktrees = vec![WorktreeInfo {
        domain: config.domain.clone(),
        path: our_worktree.clone(),
        status: "active".to_string(),
    }];

    for domain in &affected {
        let domain_config = load_domain_config(&realm_path, domain)?;
        if let Some(repo_path) = &domain_config.repo_path {
            let wt = create_worktree(repo_path, &branch)?;
            worktrees.push(WorktreeInfo {
                domain: domain.clone(),
                path: wt,
                status: "active".to_string(),
            });
        }
    }

    // Track in realm
    let tracking = RealmWorktree {
        name: branch.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        source_rfc: format!("{}:{}", config.domain, rfc_title),
        domains: worktrees.clone(),
        commits: vec![],
    };

    save_realm_worktree(&realm_path, &tracking)?;

    Ok(json!({
        "status": "success",
        "message": format!("Created unified worktree '{}'", branch),
        "branch": branch,
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
    let realm_path = PathBuf::from(&config.path);

    // Find current worktree
    let branch = get_current_branch(&state.repo_path)?;
    let tracking = load_realm_worktree(&realm_path, &branch)?;

    let mut commits = vec![];

    // Commit in each domain that has changes
    for wt in &tracking.domains {
        if has_uncommitted_changes(&wt.path)? {
            let commit = git_commit(&wt.path, message)?;
            commits.push(LinkedCommit {
                domain: wt.domain.clone(),
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
        "message": format!("Committed in {} domain(s)", commits.len()),
        "commits": commits
    }))
}

// handlers/realm_pr.rs

pub fn handle_realm_pr(args: &Value, state: &ProjectState) -> Result<Value, ServerError> {
    let config = load_realm_config(&state.repo_path)?;
    let realm_path = PathBuf::from(&config.path);

    let branch = get_current_branch(&state.repo_path)?;
    let tracking = load_realm_worktree(&realm_path, &branch)?;

    let title = args.get("title").and_then(|v| v.as_str())
        .unwrap_or(&branch);

    let mut prs = vec![];

    // Determine merge order (domains that are imported should merge first)
    let ordered_domains = topological_sort(&realm_path, &tracking.domains)?;

    for (i, wt) in ordered_domains.iter().enumerate() {
        let domain_config = load_domain_config(&realm_path, &wt.domain)?;
        if let Some(repo_path) = &domain_config.repo_path {
            // Create PR with links to other PRs
            let body = generate_pr_body(&tracking, &prs, i)?;
            let pr = create_github_pr(repo_path, &branch, title, &body)?;

            prs.push(LinkedPR {
                domain: wt.domain.clone(),
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
        "merge_order": ordered_domains.iter().map(|d| &d.domain).collect::<Vec<_>>()
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
- [ ] `blue realm join` registers domain correctly
- [ ] `blue realm join` auto-detects S3 path exports
- [ ] `blue status` shows realm state
- [ ] `blue status` detects local export changes
- [ ] `blue status` shows stale imports
- [ ] `blue realm sync` updates exports in realm
- [ ] `blue realm sync` creates GitHub issues
- [ ] `blue realm sync --dry-run` shows changes without committing
- [ ] Multiple domains can coordinate through realm
- [ ] Breaking changes are flagged appropriately
- [ ] CODEOWNERS prevents cross-domain writes

### Session Coordination
- [ ] IPC socket created on Blue start
- [ ] Session registered in realm index
- [ ] Export changes broadcast to watching sessions
- [ ] Sessions cleaned up on process exit
- [ ] `blue realm sessions` lists active sessions

### Unified Worktrees
- [ ] `blue realm worktree` creates worktrees in all affected repos
- [ ] Worktree tracked in realm repo
- [ ] `blue realm commit` commits in all repos with changes
- [ ] Commits linked in realm tracking
- [ ] `blue realm push` pushes all branches
- [ ] `blue realm pr` creates linked PRs with correct merge order

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
