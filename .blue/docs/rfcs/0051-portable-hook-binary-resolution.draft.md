# RFC 0051: Portable Hook Binary Resolution

**Status**: Draft
**Created**: 2026-02-01
**Author**: Claude Opus 4.5
**Related**: RFC 0038, RFC 0049

## Problem Statement

Claude Code PreToolUse hooks run in a minimal environment without full PATH initialization. When hooks invoke commands by name (e.g., `blue guard`), the shell cannot resolve the binary and hangs indefinitely.

### Current Workaround

The guard hook uses a hardcoded absolute path:
```bash
/Users/ericg/letemcook/blue/target/release/blue guard --path="$FILE_PATH"
```

This works but is:
- **Not portable**: Different paths on different machines
- **Fragile**: Breaks if binary moves
- **Not team-friendly**: Each developer needs different paths

### Root Cause

Hook processes don't inherit the user's shell initialization (`.bashrc`, `.zshrc`). This means:
- Custom PATH entries (like `~/.cargo/bin`) are not available
- Homebrew paths may be missing
- Language version managers (nvm, rbenv) don't work

This is likely intentional for security (preventing secrets/aliases in hooks).

## Proposed Solution

Use `$CLAUDE_PROJECT_DIR` environment variable for portable binary resolution.

### Option A: Project-Relative Binary (Recommended)

Update hook to use `$CLAUDE_PROJECT_DIR`:

```bash
#!/bin/bash
# .claude/hooks/guard-write.sh

FILE_PATH=$(jq -r '.tool_input.file_path // empty')

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

# Use CLAUDE_PROJECT_DIR for portable path resolution
"$CLAUDE_PROJECT_DIR/target/release/blue" guard --path="$FILE_PATH"
```

**Pros:**
- Portable across team members
- Works with any checkout location
- Documented Claude Code pattern

**Cons:**
- Requires binary in project directory
- Must rebuild after checkout

### Option B: SessionStart PATH Injection

Add a SessionStart hook that adds blue to PATH:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [{
          "type": "command",
          "command": ".claude/hooks/setup-path.sh"
        }]
      }
    ]
  }
}
```

```bash
#!/bin/bash
# .claude/hooks/setup-path.sh
if [ -n "$CLAUDE_ENV_FILE" ]; then
  echo "export PATH=\"$CLAUDE_PROJECT_DIR/target/release:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi
exit 0
```

**Pros:**
- `blue` works by name in all subsequent hooks
- Cleaner hook scripts

**Cons:**
- Requires SessionStart hook
- More complex setup
- Session-specific (resets on restart)

### Option C: Installed Binary with Explicit PATH

For teams that install blue globally:

```bash
#!/bin/bash
# Explicitly set PATH to include common install locations
export PATH="$HOME/.cargo/bin:/usr/local/bin:$PATH"

FILE_PATH=$(jq -r '.tool_input.file_path // empty')
if [ -z "$FILE_PATH" ]; then
    exit 0
fi

blue guard --path="$FILE_PATH"
```

**Pros:**
- Works with installed binaries
- Standard Unix pattern

**Cons:**
- Must enumerate all possible install locations
- Still not fully portable

## Recommendation

**Option B** (SessionStart PATH injection) is recommended because:
1. Cleaner hook scripts - just use `blue` by name
2. Works for any hook that needs blue
3. Consistent with existing SessionStart hooks pattern
4. PATH set once, used everywhere

## Implementation Plan

- [ ] Update `.claude/hooks/guard-write.sh` to use `$CLAUDE_PROJECT_DIR`
- [ ] Update `.claude/settings.json` to use quoted project dir path
- [ ] Test on fresh checkout
- [ ] Document in README or CONTRIBUTING.md

## Migration

Before:
```bash
/Users/ericg/letemcook/blue/target/release/blue guard --path="$FILE_PATH"
```

After:
```bash
"$CLAUDE_PROJECT_DIR/target/release/blue" guard --path="$FILE_PATH"
```

## Testing

1. Clone repo to new location
2. Build: `cargo build --release -p blue`
3. Run Claude Code
4. Attempt a write operation
5. Verify guard runs without hanging

## References

- Claude Code Hooks Documentation: https://code.claude.com/docs/en/hooks.md
- RFC 0038: SDLC Workflow Discipline
- RFC 0049: Synchronous Guard Command
