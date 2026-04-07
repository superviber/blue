# Blue

A CLI tool for structured software development workflows with [Claude Code](https://claude.ai/claude-code).

Blue manages RFCs, worktrees, alignment dialogues, and guard hooks to keep your development process disciplined and traceable.

## Install

```bash
cargo install --path apps/blue-cli
blue install
```

Restart Claude Code after installation. See [INSTALL.md](INSTALL.md) for details.

## What It Does

**Document lifecycle** -- Create, track, and transition RFCs, ADRs, spikes, and other documents with enforced status workflows and filename conventions.

```bash
blue rfc create "Feature Name"       # Create a new RFC (Draft)
blue rfc status "Feature Name" --set approved   # Approve it
blue rfc list                        # List all RFCs
```

**Worktree management** -- Branch-per-RFC workflow with guard hooks that block writes to unapproved work.

```bash
blue worktree create "Feature Name"  # Create a git worktree for an RFC
blue worktree list                   # Show active worktrees
```

**Guard hooks** -- Global PreToolUse hook (`blue guard --stdin`) that enforces:
- Worktree scope protection (RFC 0038)
- RFC approval checks before implementation (RFC 0073)
- RFC status field edit interception (RFC 0076) -- prevents direct edits, routes through `blue rfc status`

**Alignment dialogues** -- Multi-expert deliberation for design decisions via `/blue-alignment-play`.

**Org-level operations** -- Coordinate across multiple repos with `org.yaml` and `blue org status`.

## What Gets Installed

| Component | Location | Purpose |
|-----------|----------|---------|
| **Guard hook** | `~/.claude/settings.json` | Global PreToolUse write guard |
| **Session hooks** | `~/.claude/settings.json` | Session start, compaction recovery |
| **Skills** | `~/.claude/skills/` | Alignment dialogues, worktree management |

All hooks are global (RFC 0076) -- no per-project `.claude/` directories needed.

## Verify

```bash
blue doctor    # Check installation health
blue status    # Show project state
```

## Project Structure

```
apps/blue-cli/       # CLI binary
crates/blue-core/    # Core library (store, handlers, install)
skills/              # Claude Code skills (embedded at compile time)
.blue/docs/rfcs/     # Blue's own RFCs
```

## License

[MIT](LICENSE) -- Eric Minton Garcia, 2026.
