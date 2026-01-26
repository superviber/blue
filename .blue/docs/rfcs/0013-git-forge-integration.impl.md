# RFC 0013: Git Forge Integration

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-25 |
| **Source Spike** | Git Forge Integration for Blue MCP |

---

## Problem

Blue's PR tools (`blue_pr_create`, `blue_pr_verify`, `blue_pr_merge`) shell out to `gh` CLI, which only works with GitHub. Users with Forgejo/Gitea remotes can't create PRs via Blue MCP - the commands fail silently or with cryptic errors.

This blocks the workflow for anyone not using GitHub.

## Goals

1. Native REST API integration for GitHub and Forgejo/Gitea
2. Auto-detect forge type from git remotes
3. Unified interface - same Blue tools work regardless of forge
4. No external CLI dependencies (`gh`, `tea`, etc.)

## Non-Goals

- GitLab support (different API, future RFC)
- Issue management (PRs only for now)
- Full forge feature parity (minimal surface for Blue's workflow)

## Proposal

### 1. Forge Trait

```rust
pub trait Forge: Send + Sync {
    fn create_pr(&self, opts: CreatePrOpts) -> Result<PullRequest, ForgeError>;
    fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest, ForgeError>;
    fn merge_pr(&self, owner: &str, repo: &str, number: u64, strategy: MergeStrategy) -> Result<(), ForgeError>;
    fn pr_is_merged(&self, owner: &str, repo: &str, number: u64) -> Result<bool, ForgeError>;
}

pub struct CreatePrOpts {
    pub owner: String,
    pub repo: String,
    pub head: String,      // branch with changes
    pub base: String,      // target branch
    pub title: String,
    pub body: Option<String>,
    pub draft: bool,
}

pub enum MergeStrategy {
    Merge,
    Squash,
    Rebase,
}
```

### 2. Implementations

**GitHubForge**
- Endpoint: `https://api.github.com/repos/{owner}/{repo}/pulls`
- Auth: `Authorization: Bearer {GITHUB_TOKEN}`

**ForgejoForge** (works with Gitea too)
- Endpoint: `https://{host}/api/v1/repos/{owner}/{repo}/pulls`
- Auth: `Authorization: token {FORGEJO_TOKEN}`

### 3. Auto-Detection

Parse git remotes to detect forge type:

```rust
fn detect_forge(remote_url: &str) -> ForgeType {
    let url = parse_git_url(remote_url);

    match url.host {
        "github.com" => ForgeType::GitHub,
        "codeberg.org" => ForgeType::Forgejo,
        host if host.contains("gitea") => ForgeType::Forgejo,
        host if host.contains("forgejo") => ForgeType::Forgejo,
        _ => {
            // Probe /api/v1/version - Forgejo/Gitea respond, GitHub doesn't
            if probe_forgejo_api(&url.host) {
                ForgeType::Forgejo
            } else {
                ForgeType::GitHub // fallback
            }
        }
    }
}
```

Cache detected type in `.blue/config.yaml`:

```yaml
forge:
  type: forgejo
  host: git.beyondtheuniverse.superviber.com
  owner: superviber
  repo: blue
```

### 4. Token Resolution

Environment variables with fallback chain:

| Forge | Variables (in order) |
|-------|---------------------|
| GitHub | `GITHUB_TOKEN`, `GH_TOKEN` |
| Forgejo | `FORGEJO_TOKEN`, `GITEA_TOKEN` |

### 5. Updated MCP Tools

`blue_pr_create` changes:
- Remove `gh` CLI dependency
- Use detected forge's REST API
- Return PR URL directly

Response includes forge info:

```json
{
  "status": "success",
  "pr_url": "https://git.example.com/owner/repo/pulls/42",
  "pr_number": 42,
  "forge": "forgejo"
}
```

### 6. New Module Structure

```
crates/blue-core/src/
├── forge/
│   ├── mod.rs          # Forge trait, ForgeType, detection
│   ├── github.rs       # GitHub implementation
│   ├── forgejo.rs      # Forgejo/Gitea implementation
│   └── git_url.rs      # URL parsing utilities
```

## Alternatives Considered

### A. Keep shelling out to CLIs
Rejected: Requires users to install and configure `gh`/`tea`. Fragile, hard to get structured output.

### B. Use existing MCP servers (forgejo-mcp, github-mcp)
Rejected: Adds external dependencies. forgejo-mcp doesn't support PR creation. Better to own the integration.

### C. GitLab support in this RFC
Deferred: Different API patterns. Keep scope focused. Future RFC.

## Implementation Plan

1. Add `forge` module to blue-core with trait and types
2. Implement `ForgejoForge` with REST client
3. Implement `GitHubForge` with REST client
4. Add auto-detection logic with caching
5. Update `handle_pr_create` to use forge
6. Update `handle_pr_verify` and `handle_pr_merge`
7. Remove `gh` CLI dependency

## Test Plan

- [ ] ForgejoForge creates PR via REST API
- [ ] GitHubForge creates PR via REST API
- [ ] Auto-detection identifies github.com as GitHub
- [ ] Auto-detection identifies codeberg.org as Forgejo
- [ ] Auto-detection probes unknown hosts
- [ ] Token resolution finds FORGEJO_TOKEN
- [ ] Token resolution finds GITHUB_TOKEN
- [ ] blue_pr_create works with Forgejo remote
- [ ] blue_pr_create works with GitHub remote
- [ ] PR merge works with both forges

---

*"One interface, many forges. The abstraction serves the worker."*

— Blue
