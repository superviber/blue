# Installing Blue

## Quick Install

```bash
cargo install --path apps/blue-cli
blue install
```

Restart Claude Code after installation.

## What Gets Installed

`blue install` configures everything for Claude Code:

| Component | Location | Purpose |
|-----------|----------|---------|
| **Guard hook** | `~/.claude/settings.json` | PreToolUse guard for write protection |
| **Session hooks** | `~/.claude/settings.json` | Session start, context recovery |
| **Skills** | `~/.claude/skills/` | Alignment dialogues, worktree management |

All hooks are registered globally (RFC 0076). No per-project `.claude/` directories are created.

### Skills

Installed globally to `~/.claude/skills/` (embedded in the binary):

- `blue-alignment-play` -- Multi-expert alignment dialogues
- `blue-alignment-expert` -- Marker syntax for expert agents
- `blue-domain-setup` -- Interactive domain.yaml creation
- `blue-org-context` -- Show org context
- `blue-wt` -- Worktree lifecycle management

## Uninstall

```bash
blue uninstall
```

Or manually:

```bash
rm -rf ~/.claude/skills/blue-*
cargo uninstall blue
```

## Requirements

- Rust toolchain (cargo)
- macOS or Linux
- Claude Code

## Verify Installation

```bash
blue doctor
```

## Troubleshooting

**"command not found: blue"**
- Ensure `~/.cargo/bin` is in your PATH
- Or run: `cargo install --path apps/blue-cli`

**Binary hangs on macOS**
- Code signature issue (RFC 0060)
- Fix: `xattr -cr $(which blue) && codesign --force --sign - $(which blue)`
- Or: `cargo install --path apps/blue-cli --force`

**Hooks not firing**
- Run `blue doctor` to check global hook registration
- Run `blue install` to re-register hooks
- If you have stale per-project hooks, `blue install` will clean them up
