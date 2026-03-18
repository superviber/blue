# Blue CLI

Command-line interface for Blue.

## Installation

```bash
cargo install --path apps/blue-cli
```

Or run directly:

```bash
cargo run --bin blue
```

## Commands

| Command | Description |
|---------|-------------|
| `blue` | Show welcome message |
| `blue status` | Project status |
| `blue realm` | [Cross-repo coordination](realm.md) |
| `blue session` | Work session management |
| `blue daemon` | Background service |
| `blue worktree` | Worktree management for RFC implementation |
| `blue release` | Release develop to main with semver tag |

## Realm Commands

See [realm.md](realm.md) for full documentation.

```bash
blue realm status           # Show realm info
blue realm check            # Validate contracts
blue realm sync             # Commit changes
blue realm worktree create  # Create RFC worktrees
blue realm pr status        # Check PR readiness
blue realm admin init       # Create realm
blue realm admin join       # Join repo to realm
```

## Session Commands

```bash
blue session start --rfc <name>   # Start work session
blue session list                 # List active sessions
blue session status               # Current session info
blue session stop                 # End session
```

## Daemon Commands

```bash
blue daemon start    # Start daemon (foreground)
blue daemon status   # Check if running
blue daemon stop     # Stop daemon
```

## Worktree Commands

Manage worktrees for RFC implementation. See [workflow documentation](../workflow/README.md) for the full lifecycle.

```bash
blue worktree create --rfc <slug>   # Create worktree for approved RFC
blue worktree list                  # List active worktrees
blue worktree remove --rfc <slug>   # Remove worktree after completion
```

## Release Commands

Release develop to main with semver tagging.

```bash
blue release                # Release develop to main
blue release --bump minor   # Specify version bump (patch|minor|major)
```
