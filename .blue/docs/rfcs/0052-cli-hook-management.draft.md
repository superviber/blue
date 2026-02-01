# RFC 0052: CLI Hook Management

**Status**: Draft
**Created**: 2026-02-01
**Author**: Claude Opus 4.5
**Related**: RFC 0038, RFC 0049, RFC 0051

## Problem Statement

Blue's Claude Code integration requires hooks to be manually configured:
1. Create `.claude/hooks/` directory
2. Write hook scripts
3. Edit `.claude/settings.json`
4. Make scripts executable

This is error-prone and not portable. New team members must manually set up hooks or copy configurations.

## Proposed Solution

Add `blue hooks` subcommand to manage Claude Code hook installation:

```bash
blue hooks install    # Install all Blue hooks
blue hooks uninstall  # Remove Blue hooks
blue hooks status     # Show current hook status
blue hooks check      # Verify hooks are working
```

## Commands

### `blue hooks install`

Installs Blue's Claude Code hooks:

```bash
$ blue hooks install
Installing Blue hooks...
  ✓ Created .claude/hooks/session-start.sh
  ✓ Created .claude/hooks/guard-write.sh
  ✓ Updated .claude/settings.json
  ✓ Made scripts executable

Blue hooks installed. Restart Claude Code to activate.
```

**Behavior:**
- Creates `.claude/hooks/` if missing
- Writes hook scripts from embedded templates
- Merges into existing `.claude/settings.json` (preserves other hooks)
- Sets executable permissions
- Idempotent - safe to run multiple times

### `blue hooks uninstall`

Removes Blue's hooks:

```bash
$ blue hooks uninstall
Removing Blue hooks...
  ✓ Removed .claude/hooks/session-start.sh
  ✓ Removed .claude/hooks/guard-write.sh
  ✓ Updated .claude/settings.json

Blue hooks removed.
```

**Behavior:**
- Removes only Blue-managed hook scripts
- Removes Blue entries from settings.json (preserves other hooks)
- Leaves `.claude/` directory if other files exist

### `blue hooks status`

Shows current hook state:

```bash
$ blue hooks status
Blue Claude Code Hooks:

  SessionStart:
    ✓ session-start.sh (installed, executable)

  PreToolUse (Write|Edit|MultiEdit):
    ✓ guard-write.sh (installed, executable)

Settings: .claude/settings.json (configured)

All hooks installed and ready.
```

### `blue hooks check`

Verifies hooks work correctly:

```bash
$ blue hooks check
Checking Blue hooks...
  ✓ session-start.sh exits 0
  ✓ guard-write.sh allows /tmp/test.md
  ✓ guard-write.sh blocks src/main.rs (no worktree)

All hooks working correctly.
```

## Hook Templates

Hooks are embedded in the blue binary as string constants:

```rust
const SESSION_START_HOOK: &str = r#"#!/bin/bash
# Blue SessionStart hook - sets up PATH
# Managed by: blue hooks install

if [ -n "$CLAUDE_ENV_FILE" ] && [ -n "$CLAUDE_PROJECT_DIR" ]; then
  echo "export PATH=\"\$CLAUDE_PROJECT_DIR/target/release:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi

exit 0
"#;

const GUARD_WRITE_HOOK: &str = r#"#!/bin/bash
# Blue PreToolUse hook - enforces RFC 0038 worktree protection
# Managed by: blue hooks install

FILE_PATH=$(jq -r '.tool_input.file_path // empty')

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

blue guard --path="$FILE_PATH"
"#;
```

## Settings.json Management

The install command merges Blue hooks into existing settings:

```rust
fn merge_hooks(existing: Value, blue_hooks: Value) -> Value {
    // Preserve existing hooks, add/update Blue hooks
    // Use "# Managed by: blue" comments to identify Blue hooks
}
```

**Identification:** Hook scripts include a `# Managed by: blue hooks install` comment to identify Blue-managed hooks.

## Implementation Plan

- [ ] Add `Commands::Hooks` enum variant with subcommands
- [ ] Implement `handle_hooks_install()`
- [ ] Implement `handle_hooks_uninstall()`
- [ ] Implement `handle_hooks_status()`
- [ ] Implement `handle_hooks_check()`
- [ ] Embed hook templates as constants
- [ ] Add settings.json merge logic
- [ ] Add tests for hook management

## CLI Structure

```rust
#[derive(Subcommand)]
enum HooksCommands {
    /// Install Blue's Claude Code hooks
    Install,

    /// Remove Blue's Claude Code hooks
    Uninstall,

    /// Show hook installation status
    Status,

    /// Verify hooks are working correctly
    Check,
}
```

## Future Extensions

- `blue hooks update` - Update hooks to latest version
- `blue hooks diff` - Show differences from installed hooks
- `blue hooks export` - Export hooks for manual installation
- Support for custom hooks via `.blue/hooks/` templates

## Benefits

1. **One command setup**: `blue hooks install`
2. **Portable**: Works on any machine with blue installed
3. **Idempotent**: Safe to run repeatedly
4. **Discoverable**: `blue hooks status` shows what's installed
5. **Reversible**: `blue hooks uninstall` cleanly removes

## Migration

Existing manual installations can be migrated:

```bash
$ blue hooks install
Note: Found existing hooks. Replacing with managed versions.
  ✓ Backed up .claude/hooks/guard-write.sh to .claude/hooks/guard-write.sh.bak
  ✓ Installed managed guard-write.sh
```

## References

- RFC 0038: SDLC Workflow Discipline
- RFC 0049: Synchronous Guard Command
- RFC 0051: Portable Hook Binary Resolution
- Claude Code Hooks: https://code.claude.com/docs/en/hooks.md
