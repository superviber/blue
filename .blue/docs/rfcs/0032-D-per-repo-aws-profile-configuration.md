# RFC 0032: Per-Repo AWS Profile Configuration

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Dialogue** | [aws-profile-config.dialogue.md](../dialogues/2026-01-26T0700Z-aws-profile-config.dialogue.md) |

---

## Summary

Blue needs a way to configure per-repo AWS profiles in `.blue/config.yaml` so that all AWS operations during a Claude Code session use the correct credentials. Different repos require different AWS profiles (e.g., `f-i-a` uses `cultivarium`, `hearth`/`aperture` use `muffinlabs`).

## Problem

When working across multiple repositories that deploy to different AWS accounts, developers and Claude Code sessions need to use the correct AWS profile for each repo. Currently:

1. No mechanism exists to declare which AWS profile a repo expects
2. Claude's bash commands inherit whatever `AWS_PROFILE` is set in the user's shell
3. Worktree isolation (`blue_env_mock`) doesn't inject AWS profile settings
4. First-run discovery is poor—new contributors don't know which profile to use

## Design

### Two-Layer Architecture

| Layer | Scope | Implementation |
|-------|-------|----------------|
| **Layer 1: Blue MCP** | Bash commands, Blue tools | `std::env::set_var("AWS_PROFILE", ...)` in `ensure_state()` |
| **Layer 2: Worktrees** | Isolated environments | `blue_env_mock` writes `AWS_PROFILE` to `.env.isolated` |
| **Layer 3: AWS MCP** | External server | User responsibility (documented limitation) |

### Config Schema

Add `aws` section to `.blue/config.yaml`, parallel to existing `forge`:

```yaml
forge:
  type: github
  host: ...
  owner: superviber
  repo: blue

aws:
  profile: cultivarium  # AWS profile name from ~/.aws/config
```

### Precedence Rules

1. **Shell `AWS_PROFILE` always wins** (standard AWS CLI behavior)
2. Config provides repo default for Blue's process and `.env.isolated`
3. CI/CD sets its own `AWS_PROFILE` externally

### Rust Implementation

#### 1. Add `AwsConfig` to `BlueConfig`

File: `crates/blue-core/src/forge/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueConfig {
    pub forge: Option<ForgeConfig>,
    pub aws: Option<AwsConfig>,  // NEW
}
```

#### 2. Inject at MCP Server Startup

File: `crates/blue-mcp/src/server.rs` in `ensure_state()`

```rust
// After loading config, inject AWS_PROFILE into process environment
if let Some(aws) = &state.config.aws {
    // Only set if not already present (respect shell override)
    if std::env::var("AWS_PROFILE").is_err() {
        std::env::set_var("AWS_PROFILE", &aws.profile);
        tracing::info!(profile = %aws.profile, "AWS profile set for session");
    } else {
        let shell_profile = std::env::var("AWS_PROFILE").unwrap();
        if shell_profile != aws.profile {
            tracing::info!(
                config_profile = %aws.profile,
                shell_profile = %shell_profile,
                "Shell AWS_PROFILE differs from config—using shell value"
            );
        }
    }
}
```

#### 3. Inject into `.env.isolated`

File: `crates/blue-mcp/src/handlers/env.rs` in `generate_env_isolated()`

```rust
// Add AWS profile from config if present
if let Some(aws) = &config.aws {
    lines.push(format!("AWS_PROFILE={}", aws.profile));
}
```

### AWS MCP Server Limitation

The AWS MCP server (`mcp__aws-api__call_aws`) runs as a **separate process** that Blue cannot control. Users must either:

1. Set `AWS_PROFILE` in their shell **before** launching Claude Code desktop app
2. Use `--profile` flags explicitly in AWS CLI calls

This limitation should be documented in Blue's setup guide.

## Per-Repo Configuration

| Repo | Profile | Config |
|------|---------|--------|
| `f-i-a` | `cultivarium` | `aws: { profile: cultivarium }` |
| `hearth` | `muffinlabs` | `aws: { profile: muffinlabs }` |
| `aperture` | `muffinlabs` | `aws: { profile: muffinlabs }` |

## What Blue Controls vs. Documents

| Aspect | Blue Controls | Blue Documents |
|--------|---------------|----------------|
| Bash `aws` CLI commands | Yes | — |
| Blue MCP tools touching AWS | Yes | — |
| `.env.isolated` for worktrees | Yes | — |
| AWS MCP server (`call_aws`) | No | User sets shell env or `--profile` |

## Test Plan

- [ ] Add `aws` section to test config, verify `BlueConfig` deserializes it
- [ ] Verify `ensure_state()` sets `AWS_PROFILE` when config present
- [ ] Verify `ensure_state()` respects existing shell `AWS_PROFILE`
- [ ] Verify `blue_env_mock` includes `AWS_PROFILE` in `.env.isolated`
- [ ] Verify startup diagnostic logs active profile
- [ ] Verify warning when shell profile differs from config

## Alternatives Considered

1. **Environment-specific profiles** (`aws.profiles.{dev,ci,prod}`) — Rejected as over-engineering; CI should set its own profile externally.

2. **Store in `.env.example`** — Rejected; config.yaml is the single source of truth for repo settings.

3. **Enforce profile (override shell)** — Rejected; violates standard AWS CLI precedence and surprises operators.

---

*"Right then. Let's get to it."*

— Blue
