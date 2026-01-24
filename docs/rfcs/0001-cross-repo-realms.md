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
- Real-time synchronization (pull-based is sufficient)
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

---

## Implementation

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

### Phase 5: Polish (Week 7)

- Error handling for all edge cases
- User-friendly messages in Blue's voice
- Documentation
- Tests

---

## Test Plan

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

---

## Future Work

1. **Signature verification** - Domains sign their exports
2. **Multiple realms** - One repo participates in multiple realms
3. **Cross-realm imports** - Import from domain in different realm
4. **Public registry** - Discover realms and contracts
5. **Infrastructure verification** - Check actual AWS state matches exports
6. **Automatic PR creation** - Generate code changes, not just issues

---

## References

- [Spike: cross-repo-coordination](../spikes/cross-repo-coordination.md)
- [Dialogue: cross-repo-realms](../dialogues/cross-repo-realms.dialogue.md)
