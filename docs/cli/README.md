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
| `blue mcp` | Run as MCP server |

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

## MCP Server

Run Blue as an MCP server for Claude integration:

```bash
blue mcp
```

This exposes 8 realm coordination tools to Claude:
- `realm_status`, `realm_check`, `contract_get`
- `session_start`, `session_stop`
- `realm_worktree_create`, `realm_pr_status`
- `notifications_list`

See [../mcp/README.md](../mcp/README.md) for tool reference and [../mcp/integration.md](../mcp/integration.md) for setup guide.
