# RFC 0052: Blue Install Command

**Status**: Implemented
**Created**: 2026-02-01
**Updated**: 2026-02-01
**Author**: Claude Opus 4.5
**Related**: RFC 0038, RFC 0049, RFC 0051

## Problem Statement

Blue's Claude Code integration requires multiple manual setup steps:

1. **Hooks**: Create `.claude/hooks/`, write scripts, edit settings.json
2. **Skills**: Symlink `skills/*` to `~/.claude/skills/`
3. **MCP Server**: Add blue to `~/.claude.json` mcpServers

This is error-prone, not portable, and undiscoverable. New team members must manually configure everything.

## Proposed Solution

A unified `blue install` command that sets up all Claude Code integration:

```bash
blue install      # Install everything
blue uninstall    # Remove everything
blue doctor       # Check installation health
```

## What Gets Installed

| Component | Location | Purpose |
|-----------|----------|---------|
| **Hooks** | `.claude/hooks/` + `.claude/settings.json` | SessionStart (PATH), PreToolUse (guard) |
| **Skills** | `~/.claude/skills/` (symlinks) | `/alignment-play` and future skills |
| **MCP Server** | `~/.claude.json` | Blue MCP tools |

## Commands

### `blue install`

```bash
$ blue install
Installing Blue for Claude Code...

Hooks:
  ✓ .claude/hooks/session-start.sh
  ✓ .claude/hooks/guard-write.sh
  ✓ .claude/settings.json (merged)

Skills:
  ✓ ~/.claude/skills/alignment-play -> /Users/ericg/letemcook/blue/skills/alignment-play

MCP Server:
  ✓ ~/.claude.json (blue server configured)

Blue installed. Restart Claude Code to activate.
```

**Flags:**
- `--hooks-only` - Only install hooks
- `--skills-only` - Only install skills
- `--mcp-only` - Only configure MCP server
- `--force` - Overwrite existing files

### `blue uninstall`

```bash
$ blue uninstall
Removing Blue from Claude Code...

Hooks:
  ✓ Removed .claude/hooks/session-start.sh
  ✓ Removed .claude/hooks/guard-write.sh
  ✓ Cleaned .claude/settings.json

Skills:
  ✓ Removed ~/.claude/skills/alignment-play

MCP Server:
  ✓ Removed blue from ~/.claude.json

Blue uninstalled.
```

### `blue doctor`

Diagnoses installation issues:

```bash
$ blue doctor
Blue Installation Health Check

Binary:
  ✓ blue found at /Users/ericg/.cargo/bin/blue
  ✓ Version: 0.1.0

Hooks:
  ✓ session-start.sh (installed, executable)
  ✓ guard-write.sh (installed, executable)
  ✓ settings.json configured

Skills:
  ✓ alignment-play (symlink valid)

MCP Server:
  ✓ blue configured in ~/.claude.json
  ✓ Binary path correct
  ✓ Server responds to ping

All checks passed.
```

With issues:

```bash
$ blue doctor
Blue Installation Health Check

Binary:
  ✓ blue found at /Users/ericg/.cargo/bin/blue

Hooks:
  ✗ guard-write.sh missing
  ✗ settings.json not configured

  Run `blue install` to fix.

Skills:
  ✗ alignment-play symlink broken (target moved?)

  Run `blue install --force` to recreate.

MCP Server:
  ✓ blue configured in ~/.claude.json
  ✗ Binary path outdated (points to old location)

  Run `blue install --mcp-only` to fix.

3 issues found. Run suggested commands to fix.
```

## Implementation Details

### Hooks Installation

```rust
fn install_hooks(project_dir: &Path, force: bool) -> Result<()> {
    let hooks_dir = project_dir.join(".claude/hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Write hook scripts
    write_hook(&hooks_dir.join("session-start.sh"), SESSION_START_TEMPLATE)?;
    write_hook(&hooks_dir.join("guard-write.sh"), GUARD_WRITE_TEMPLATE)?;

    // Merge into settings.json
    let settings_path = project_dir.join(".claude/settings.json");
    let settings = merge_hook_settings(&settings_path)?;
    fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    Ok(())
}
```

### Skills Installation

```rust
fn install_skills(project_dir: &Path) -> Result<()> {
    let skills_dir = project_dir.join("skills");
    let target_dir = dirs::home_dir()?.join(".claude/skills");

    fs::create_dir_all(&target_dir)?;

    for entry in fs::read_dir(&skills_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let skill_name = entry.file_name();
            let link_path = target_dir.join(&skill_name);

            // Remove existing symlink if present
            if link_path.exists() {
                fs::remove_file(&link_path)?;
            }

            // Create symlink
            std::os::unix::fs::symlink(entry.path(), &link_path)?;
        }
    }

    Ok(())
}
```

### MCP Server Configuration

```rust
fn install_mcp_server(project_dir: &Path) -> Result<()> {
    let config_path = dirs::home_dir()?.join(".claude.json");
    let mut config: Value = if config_path.exists() {
        serde_json::from_str(&fs::read_to_string(&config_path)?)?
    } else {
        json!({})
    };

    // Add/update blue MCP server
    config["mcpServers"]["blue"] = json!({
        "command": project_dir.join("target/release/blue").to_string_lossy(),
        "args": ["mcp"]
    });

    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    Ok(())
}
```

## Identification of Managed Files

All Blue-managed files include a header comment:

```bash
#!/bin/bash
# Managed by: blue install
# Do not edit manually - changes will be overwritten
```

Or for JSON, a metadata key:

```json
{
  "_blue_managed": true,
  "_blue_version": "0.1.0",
  ...
}
```

## CLI Structure

```rust
#[derive(Subcommand)]
enum Commands {
    /// Install Blue for Claude Code
    Install {
        /// Only install hooks
        #[arg(long)]
        hooks_only: bool,

        /// Only install skills
        #[arg(long)]
        skills_only: bool,

        /// Only configure MCP server
        #[arg(long)]
        mcp_only: bool,

        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// Remove Blue from Claude Code
    Uninstall,

    /// Check installation health
    Doctor,

    // ... existing commands
}
```

## Future Extensions

- `blue install --global` - Install for all projects (user-level config)
- `blue upgrade` - Update hooks/skills to latest templates
- `blue install --dry-run` - Show what would be installed
- Plugin system for custom hooks/skills

## Implementation Plan

- [ ] Add `Commands::Install` with flags
- [ ] Add `Commands::Uninstall`
- [ ] Add `Commands::Doctor`
- [ ] Implement hook installation (from RFC 0051)
- [ ] Implement skill symlink management
- [ ] Implement MCP server configuration
- [ ] Embed hook templates as constants
- [ ] Add settings.json merge logic
- [ ] Add ~/.claude.json merge logic
- [ ] Add installation verification
- [ ] Add tests

## Benefits

1. **One command setup**: `blue install`
2. **Discoverable**: `blue doctor` shows what's wrong
3. **Portable**: Works on any machine
4. **Idempotent**: Safe to run repeatedly
5. **Reversible**: `blue uninstall` cleanly removes everything
6. **Maintainable**: Templates embedded in binary, easy to update

## References

- RFC 0038: SDLC Workflow Discipline
- RFC 0049: Synchronous Guard Command
- RFC 0051: Portable Hook Binary Resolution
- Claude Code Hooks: https://code.claude.com/docs/en/hooks.md
