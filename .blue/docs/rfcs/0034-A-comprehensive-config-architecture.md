# RFC 0034: Comprehensive Config Architecture

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-01-26 |
| **Supersedes** | RFC 0032 |
| **Dialogue** | [blue-config-architecture.dialogue.recorded.md](../dialogues/2026-01-26T1829Z-blue-config-architecture.dialogue.recorded.md) |

---

## Summary

This RFC defines a comprehensive `.blue/config.yaml` architecture that serves as the single source of truth for repo-level Blue configuration. It addresses four requirements: (1) worktree initialization, (2) release constraints, (3) AWS profile per-repo, and (4) forge configuration—all unified under a versioned schema with clear separation between configuration and runtime state.

## Problem

Blue currently lacks a cohesive configuration architecture:

1. **No schema versioning** — Config changes can break existing tooling silently
2. **Worktree initialization undefined** — No spec for what happens when creating isolated environments
3. **Release constraints scattered** — Branch policies hardcoded in Rust, not configurable per-repo
4. **State/config conflation** — Unclear what belongs in config vs runtime state

RFC 0032 proposed adding AWS profile configuration but didn't address these broader architectural concerns.

## Design

### Core Principles

1. **Config declares intent** — Branch roles, dependencies, requirements
2. **Blue validates** — Pre-flight checks, warnings, guidance
3. **Forge enforces** — Branch protection, required reviews, CI gates
4. **State separation** — config.yaml (pure declarations), blue.db (runtime state)
5. **Single source** — One versioned file with semantic sections

### Schema (Version 1)

```yaml
version: 1

forge:
  type: github             # github | forgejo | gitlab | bitbucket
  host: github.com         # or self-hosted (e.g., git.example.com)
  owner: superviber
  repo: blue

aws:
  profile: cultivarium  # AWS profile from ~/.aws/config

release:
  develop_branch: develop   # where active development happens
  main_branch: main         # protected release branch

worktree:
  env:
    # Additional environment variables injected into .env.isolated
    # LOG_LEVEL: debug
    # RUST_BACKTRACE: 1
```

### Section Definitions

#### `version` (required)

Schema version for migration support. Blue validates this field and rejects unknown versions with actionable error messages.

```yaml
version: 1
```

#### `forge` (required)

Forge connection details. Blue uses these for PR creation, issue tracking, and API operations.

```yaml
forge:
  type: github              # github | forgejo | gitlab | bitbucket
  host: github.com          # API host (self-hosted URLs supported)
  owner: superviber         # Repository owner/organization
  repo: blue                # Repository name
```

**Forgejo/Gitea Note**: Forgejo uses a GitHub-compatible API for most operations. Set `type: forgejo` and `host` to your instance URL (e.g., `git.example.com`).

#### `aws` (optional)

AWS profile configuration. Inherits RFC 0032's precedence rules.

```yaml
aws:
  profile: cultivarium      # Profile name from ~/.aws/config
```

**Precedence**: Shell `AWS_PROFILE` > config > Blue defaults

#### `release` (optional)

Branch topology for release workflows. Blue **observes** these for intelligent operations; **Forge enforces** protection rules.

```yaml
release:
  develop_branch: develop   # Default PR target, RFC work branch
  main_branch: main         # Release target, protected branch
```

**Blue's role**: Validate PR targets, warn on policy violations, generate correct release notes.
**Forge's role**: Enforce branch protection, required reviews, CI gates.

#### `worktree` (optional)

Worktree initialization configuration. Defines environment variables injected into `.env.isolated` during `blue_env_mock`.

```yaml
worktree:
  env:
    AWS_PROFILE: "${aws.profile}"   # Reference other config values
    LOG_LEVEL: debug
    RUST_BACKTRACE: 1
```

### Worktree Initialization

"Worktree initialization" means: **the operations Blue performs when creating an isolated work environment**.

1. **Read config** — Load `.blue/config.yaml`
2. **Generate `.env.isolated`** — Write environment variables from `aws.profile` and `worktree.env`
3. **Validate state** — Check branches exist, forge is reachable (warn, don't block)

This happens in `blue_env_mock` and `ensure_state()`.

### Observe vs Enforce

Blue operates in the **knowledge layer**—it knows branch policies exist, validates conformance, warns on deviations. The forge operates in the **enforcement layer**—it blocks merges, requires reviews, enforces protection rules.

| Aspect | Blue's Role | Forge's Role |
|--------|-------------|--------------|
| Branch topology | Observes, validates | Enforces protection |
| PR targets | Suggests correct branch | Rejects invalid targets |
| Direct commits to main | Warns user | Blocks if protected |
| Release workflow | Generates docs from correct branch | Enforces CI gates |

**Why not enforce in Blue?**
- Blue cannot prevent `git push --force main`—only Forge can
- Enforcement creates brittle duplication of forge logic
- Sync problems between local config and remote reality
- False sense of security

### State Separation

| Category | Location | Example |
|----------|----------|---------|
| **Declarations** | `.blue/config.yaml` | AWS profile, branch names |
| **Runtime state** | `blue.db` | Active RFC, session data |
| **Generated artifacts** | `.env.isolated` | Environment for worktree |
| **Ephemeral state** | `.blue/state/` | Lock files, cache |

Config is **read-only after load**. Rust's ownership model enforces this—pass `&BlueConfig` references, never `&mut`.

### Rust Implementation

#### 1. Config Struct

File: `crates/blue-core/src/config.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueConfig {
    pub version: u32,
    pub forge: ForgeConfig,
    #[serde(default)]
    pub aws: Option<AwsConfig>,
    #[serde(default)]
    pub release: Option<ReleaseConfig>,
    #[serde(default)]
    pub worktree: Option<WorktreeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeConfig {
    #[serde(rename = "type")]
    pub forge_type: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseConfig {
    #[serde(default = "default_develop")]
    pub develop_branch: String,
    #[serde(default = "default_main")]
    pub main_branch: String,
}

fn default_develop() -> String { "develop".to_string() }
fn default_main() -> String { "main".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeConfig {
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}
```

#### 2. Schema Validation

File: `crates/blue-core/src/config.rs`

```rust
impl BlueConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Version check
        if self.version != 1 {
            return Err(ConfigError::UnsupportedVersion {
                found: self.version,
                supported: vec![1],
            });
        }

        // Forge validation
        if !["github", "forgejo", "gitlab", "bitbucket"].contains(&self.forge.forge_type.as_str()) {
            return Err(ConfigError::InvalidForgeType(self.forge.forge_type.clone()));
        }

        Ok(())
    }
}
```

#### 3. Environment Injection

File: `crates/blue-mcp/src/server.rs` in `ensure_state()`

```rust
// Inject AWS_PROFILE from config (RFC 0032 precedence: shell wins)
if let Some(aws) = &state.config.aws {
    if std::env::var("AWS_PROFILE").is_err() {
        std::env::set_var("AWS_PROFILE", &aws.profile);
        tracing::info!(profile = %aws.profile, "AWS profile set from config");
    }
}
```

#### 4. Worktree Environment Generation

File: `crates/blue-mcp/src/handlers/env.rs`

```rust
pub fn generate_env_isolated(config: &BlueConfig) -> Vec<String> {
    let mut lines = vec![
        "# Generated by Blue - do not edit manually".to_string(),
        format!("# Source: .blue/config.yaml (version {})", config.version),
    ];

    // AWS profile
    if let Some(aws) = &config.aws {
        lines.push(format!("AWS_PROFILE={}", aws.profile));
    }

    // Custom worktree env vars
    if let Some(worktree) = &config.worktree {
        for (key, value) in &worktree.env {
            lines.push(format!("{}={}", key, value));
        }
    }

    lines
}
```

### Migration Path

#### From RFC 0032 (proposed but not implemented)

No migration needed—RFC 0032 was never implemented.

#### From existing config.yaml

Existing configs without `version` field:

```yaml
# Before (version-less)
forge:
  type: github
  host: github.com
  owner: superviber
  repo: blue
```

Blue CLI provides migration command:

```bash
blue config migrate   # Adds version: 1, preserves existing fields
blue config validate  # Checks config-to-reality alignment
```

### Validation Timing

Blue validates config at multiple points:

| Timing | Validation | Action |
|--------|------------|--------|
| `ensure_state()` | Schema version, required fields | Error if invalid |
| `blue_env_mock` | AWS profile exists | Warn if missing |
| `blue_worktree_create` | Branches exist | Warn if missing |
| `blue_pr_create` | PR target matches policy | Warn if mismatch |

Warnings are surfaced; operations proceed. Enforcement is Forge's responsibility.

## Test Plan

- [ ] Schema v1 loads and validates correctly
- [ ] Unknown version rejected with actionable error
- [ ] `forge` section required, others optional
- [ ] AWS profile injected into environment (shell precedence honored)
- [ ] Release branch defaults applied when section missing
- [ ] `.env.isolated` generated with all worktree.env vars
- [ ] Config validation warns on missing AWS profile
- [ ] PR creation warns when target mismatches release.develop_branch
- [ ] `blue config migrate` adds version to legacy configs
- [ ] `blue config validate` checks forge reachability

## Alternatives Considered

### 1. Layered config files (`.blue/config.yaml` + `.blue/policies.yaml`)

**Rejected**: Adds operational complexity. Where do I look when things break? Single file with semantic sections provides the same separation without filesystem fragmentation.

### 2. Blue enforces branch policies

**Rejected**: Blue cannot prevent `git push --force main`. Attempting enforcement creates false security and brittle duplication of forge logic. Blue's role is validation and guidance; Forge owns enforcement.

### 3. Config contains runtime state

**Rejected**: Violates config-as-contract semantics. Git doesn't put "is working tree clean?" in `.git/config`. Config declares identity; state tracks operations.

### 4. Per-worktree config overrides

**Deferred**: Edge case raised in dialogue. Could add `.blue/worktrees/{name}/config.yaml` layering later if real need emerges. YAGNI for now.

## References

- [Alignment Dialogue: blue-config-architecture](../dialogues/2026-01-26T1829Z-blue-config-architecture.dialogue.recorded.md) — 12-expert deliberation, 100% convergence
- RFC 0032 (superseded) — Original AWS profile proposal
- `.blue/context.manifest.yaml` — Precedent for versioned YAML schemas in Blue

---

*"Configuration declares reality; validation observes consistency; tooling enforces policy at appropriate boundaries."*

— Blue
