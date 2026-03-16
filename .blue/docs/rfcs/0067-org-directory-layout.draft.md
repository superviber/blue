# RFC 0067: Org-Mapped Directory Layout

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |
| **Depends on** | RFC 0001 (Cross-Repo Realms) |

---

## Problem

Every repo lives as a flat peer under `~/letemcook/`:

```
~/letemcook/
  aperture/          → github.com/muffin-labs/training-tools
  blue/              → github.com/superviber/blue
  fungal-image-analysis/ → github.com/cultivarium/fungal-image-analysis
  coherence/         → github.com/superviber/coherence
  superviber-web/    → github.com/superviber/superviber-web
  ...20+ more repos
```

Problems:

1. **No org boundary in the filesystem.** Repos from `superviber`, `muffin-labs`, and `cultivarium` are all siblings. There's no visual or structural indication of ownership.
2. **Blue must live as a peer.** You have to be in the blue repo (or a sibling) for blue to work. There's no concept of "blue manages repos elsewhere."
3. **Realm repo paths are brittle.** `RepoConfig.path` is an absolute path string. Move a folder and the realm breaks.
4. **GitHub org → local directory is implicit.** When cloning a new repo, you have to manually decide where to put it. Nothing enforces or suggests the org-based structure.
5. **Scaling.** 25 repos flat is already noisy. At 50+ it's unmanageable.

## Goals

1. **Org-mapped directory layout** — repos from `github.com/superviber/*` live under `~/code/superviber/`, repos from `github.com/muffin-labs/*` under `~/code/muffin-labs/`, etc.
2. **Blue works from anywhere** — `blue` should function in any repo it manages, not just from its own directory.
3. **Automatic path resolution** — given an org and repo name, Blue knows where to find it on disk.
4. **Clone-to-org** — `blue clone <url>` puts the repo in the right org subfolder automatically.
5. **Backward compatible** — existing flat layouts continue to work; migration is opt-in.

## Non-Goals

- Monorepo support (each repo is still a distinct Git repo)
- Enforcing a single root directory across machines
- Managing repos Blue doesn't know about

---

## Proposal

### 1. Blue Home Directory

Introduce a **blue home** — the root under which org directories live:

```
~/.config/blue/config.toml
```

```toml
[home]
path = "/Users/ericg/code"  # default: ~/code
```

When `home.path` is not set, Blue falls back to discovering repos via `.blue/config.yaml` in the current directory (existing behavior).

### 2. Org Directories

Under blue home, each GitHub/Forgejo org gets a subdirectory:

```
~/code/
  superviber/
    blue/
    coherence/
    superviber-web/
  muffin-labs/
    training-tools/
  cultivarium/
    fungal-image-analysis/
```

The org name matches the Git remote org exactly (case-preserved from GitHub).

### 3. Org Registry

```toml
# ~/.config/blue/config.toml

[home]
path = "/Users/ericg/code"

[[orgs]]
name = "superviber"
provider = "github"
# Optional: override clone URL pattern
# url_pattern = "git@github.com:{org}/{repo}.git"

[[orgs]]
name = "muffin-labs"
provider = "github"

[[orgs]]
name = "cultivarium"
provider = "github"

[[orgs]]
name = "letemcook"
provider = "forgejo"
host = "git.beyondtheuniverse.superviber.com"
```

### 4. Repo Resolution

Blue resolves a repo path by:

1. Check `{home}/{org}/{repo}/` (org layout)
2. Check `{home}/{repo}/` (flat fallback)
3. Check `RepoConfig.path` from realm config (absolute path fallback)
4. Not found → offer to clone

This means realm `RepoConfig` can store `org: "superviber"` and `name: "blue"` instead of an absolute path. Blue computes the path from the org registry + home.

### 5. `blue clone`

New command that clones into the right place:

```bash
# Infers org from URL
blue clone git@github.com:muffin-labs/training-tools.git
# → ~/code/muffin-labs/training-tools/

# Explicit org
blue clone --org superviber coherence
# → git@github.com:superviber/coherence.git → ~/code/superviber/coherence/

# Also initializes .blue/ in the cloned repo and registers in realm
blue clone --realm letemcook git@github.com:cultivarium/fungal-image-analysis.git
```

### 6. `blue init` in Any Repo

Currently `blue init` bootstraps the `.blue/` directory. Extend it to:

1. Detect the repo's remote org from `git remote -v`
2. Register the org in `~/.config/blue/config.toml` if not already present
3. Optionally register in a realm

```bash
cd ~/code/superviber/coherence
blue init
# Detects: org=superviber, provider=github
# Creates .blue/ config
# Asks: register in a realm? [letemcook]
```

### 7. Realm Integration

Update `RepoConfig` to prefer org-relative paths:

```yaml
# Before (absolute, brittle)
name: blue
path: /Users/ericg/letemcook/blue

# After (org-relative, portable)
name: blue
org: superviber
# path resolved via: {home}/{org}/{name}
```

Absolute `path` remains supported for repos outside the org layout.

### 8. `blue org` Commands

```bash
blue org list                    # List registered orgs and their repos
blue org add superviber          # Register a GitHub org
blue org add letemcook --provider forgejo --host git.beyondtheuniverse.superviber.com
blue org scan superviber         # Discover repos in ~/code/superviber/
blue org status                  # Show org → repo mapping, missing clones, stale paths
```

### 9. Migration Path

For existing flat layouts:

```bash
blue org migrate
# Scans ~/letemcook/ for repos with git remotes
# Groups by org
# Proposes moves:
#   ~/letemcook/blue → ~/code/superviber/blue
#   ~/letemcook/aperture → ~/code/muffin-labs/training-tools
#   ~/letemcook/fungal-image-analysis → ~/code/cultivarium/fungal-image-analysis
# User confirms, Blue moves dirs and updates realm configs
```

This is opt-in. Existing setups keep working.

## Implementation Plan

### Task 1: Blue global config

- [x] Define `~/.config/blue/config.toml` schema
- [x] Load/save with `dirs::config_dir()`
- [x] `home.path` resolution with fallback
- [x] Unit tests

### Task 2: Org registry

- [x] `Org` struct: name, provider, host, url_pattern
- [x] Parse orgs from config.toml
- [x] Resolve repo path: `{home}/{org}/{repo}`
- [x] `blue org list`, `blue org add`, `blue org scan`

### Task 3: `blue clone`

- [x] Parse GitHub/Forgejo URL → org + repo name
- [x] Clone to `{home}/{org}/{repo}/`
- [x] Auto `blue init` after clone
- [x] `--realm` flag for auto-registration

### Task 4: Repo resolution update

- [x] Fallback chain: org layout → flat → absolute path
- [x] Update realm service to use new resolution (`resolve_repo_path` method)
- [x] Update worktree creation to use new paths
- [x] `join_realm` now auto-detects org from git remote

### Task 5: Migration

- [x] `blue org migrate` — scan, propose, move
- [x] Collision detection in dry-run output
- [x] Auto-register orgs on execute
- [x] ~Symlink support~ — not needed; `blue init` re-detects after move

### Task 6: `blue init` enhancements

- [x] Detect org from git remote
- [x] Auto-register org in global config
- [x] Realm registration prompt

## Test Plan

- [x] Config load/save round-trip
- [x] Org resolution: `{home}/{org}/{repo}` → correct path
- [x] Fallback chain: org → flat → absolute
- [x] URL parsing: GitHub SSH, HTTPS, Forgejo
- [x] `blue clone` creates correct directory structure
- [x] Migration proposes correct moves (with collision detection)
- [x] Realm `RepoConfig` works with org-relative and absolute paths
- [x] Existing flat layout continues to work (no config set)

---

*"A place for everything, and everything in its place."*

--- Blue
