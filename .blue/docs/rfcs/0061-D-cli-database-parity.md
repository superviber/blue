# RFC 0061: CLI Database Parity

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-11 |
| **Relates To** | RFC 0003 (Per-Repo Structure), RFC 0057 (CLI Parity), RFC 0060 (Reliable Binary Installation) |

---

## Summary

Complete the CLI-MCP parity work by fixing stub commands to use the shared handler infrastructure already established in RFC 0057.

## Problem Statement

### The Pattern Already Exists

RFC 0057 established the correct pattern for CLI commands:

```rust
fn get_project_state() -> Result<ProjectState> {
    let cwd = std::env::current_dir()?;
    let home = blue_core::detect_blue(&cwd)?;
    let project = home.project_name.clone().unwrap_or_default();
    ProjectState::load(home, &project)
}

async fn handle_adr_command(command: AdrCommands) -> Result<()> {
    let mut state = get_project_state()?;
    match command {
        AdrCommands::Create { title } => {
            let args = json!({ "title": title });
            match blue_mcp::handlers::adr::handle_create(&mut state, &args) {
                // ...
            }
        }
    }
}
```

This pattern:
1. Uses `get_project_state()` to load the database
2. Calls shared `blue_mcp::handlers::*` functions
3. Formats output for CLI consumption

### Stub Commands That Don't Use This Pattern

The following commands are stubs that just print messages:

| Command | Current Behavior | Should Call |
|---------|-----------------|-------------|
| `blue init` | Prints welcome | `detect_blue()` + `ProjectState::load()` |
| `blue status` | Prints welcome | `blue_mcp::handlers::status` |
| `blue next` | Prints message | `blue_mcp::handlers::next` |
| `blue rfc create` | Prints message | `blue_mcp::handlers::rfc::handle_create` |
| `blue rfc get` | Prints message | `blue_mcp::handlers::rfc::handle_get` |
| `blue rfc plan` | Prints message | `blue_mcp::handlers::rfc::handle_plan` |
| `blue worktree create` | Prints message | `blue_mcp::handlers::worktree::handle_create` |
| `blue worktree list` | Prints message | `blue_mcp::handlers::worktree::handle_list` |
| `blue worktree remove` | Prints message | `blue_mcp::handlers::worktree::handle_remove` |
| `blue pr create` | Prints message | `blue_mcp::handlers::pr::handle_create` |
| `blue lint` | Prints message | `blue_mcp::handlers::lint::handle_lint` |

### Commands Already Using Shared Handlers (RFC 0057)

These are correctly implemented:

| Command Group | Calls Into |
|---------------|------------|
| `blue dialogue *` | `blue_mcp::handlers::dialogue::*` |
| `blue adr *` | `blue_mcp::handlers::adr::*` |
| `blue spike *` | `blue_mcp::handlers::spike::*` |
| `blue audit *` | `blue_mcp::handlers::audit_doc::*` |
| `blue prd *` | `blue_mcp::handlers::prd::*` |
| `blue reminder *` | `blue_mcp::handlers::reminder::*` |

## Proposal

### 1. Implement `blue init`

```rust
Some(Commands::Init) => {
    let cwd = std::env::current_dir()?;

    if cwd.join(".blue").exists() {
        println!("Blue already initialized.");
        return Ok(());
    }

    // detect_blue auto-creates .blue/ per RFC 0003
    let home = blue_core::detect_blue(&cwd)?;

    // Load state to ensure database is created with schema
    let project = home.project_name.clone().unwrap_or_default();
    let _state = ProjectState::load(home.clone(), &project)?;

    println!("{}", blue_core::voice::welcome());
    println!();
    println!("Initialized Blue:");
    println!("  Database: {}", home.db_path.display());
    println!("  Docs:     {}", home.docs_path.display());
}
```

### 2. Wire Up Core Commands

Replace the stub implementations with calls to shared handlers:

```rust
Some(Commands::Status) => {
    let state = get_project_state()?;
    let args = json!({});
    let result = blue_mcp::handlers::status::handle_status(&state, &args)?;
    // Format for CLI output
    println!("{}", format_status(&result));
}

Some(Commands::Rfc { command }) => {
    handle_rfc_command(command).await?;
}
```

Add handler function following RFC 0057 pattern:

```rust
async fn handle_rfc_command(command: RfcCommands) -> Result<()> {
    let mut state = get_project_state()?;

    match command {
        RfcCommands::Create { title } => {
            let args = json!({ "title": title });
            match blue_mcp::handlers::rfc::handle_create(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                    if let Some(file) = result.get("file").and_then(|v| v.as_str()) {
                        println!("File: {}", file);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        // ... other subcommands
    }
    Ok(())
}
```

### 3. Missing MCP Handler Functions

Some handlers exist but lack the functions needed. Check and add:

| Handler File | Needed Functions |
|--------------|------------------|
| `rfc.rs` | `handle_create`, `handle_get`, `handle_plan` |
| `worktree.rs` | `handle_create`, `handle_list`, `handle_remove` |
| `pr.rs` | `handle_create` |
| `lint.rs` | `handle_lint` |

The server.rs has these as methods on `BlueServer`. They need to be refactored into standalone functions in the handler modules, similar to how RFC 0057 handlers work.

### 4. Handler Refactoring Strategy

Currently, MCP server methods look like:

```rust
// In server.rs
fn handle_rfc_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
    self.ensure_state(args)?;  // Loads state
    let state = self.state.as_ref().unwrap();
    // ... implementation
}
```

Refactor to:

```rust
// In handlers/rfc.rs
pub fn handle_create(state: &mut ProjectState, args: &Value) -> Result<Value, ServerError> {
    // ... implementation (no self, takes state directly)
}

// In server.rs
fn handle_rfc_create(&mut self, args: &Option<Value>) -> Result<Value, ServerError> {
    self.ensure_state(args)?;
    let state = self.state.as_mut().unwrap();
    let args = args.as_ref().cloned().unwrap_or(json!({}));
    handlers::rfc::handle_create(state, &args)
}
```

This lets CLI call handlers directly without going through the MCP server.

## Implementation Plan

### Phase 1: `blue init` (Immediate)

1. Implement `blue init` using `detect_blue()` + `ProjectState::load()`
2. Add `--force` flag to reinitialize
3. Print helpful output showing what was created

### Phase 2: Extract Handler Functions

For each handler file, extract the core logic into standalone functions:

1. `handlers/rfc.rs`: `handle_create`, `handle_get`, `handle_plan`, `handle_update_status`
2. `handlers/worktree.rs`: `handle_create`, `handle_list`, `handle_remove`
3. `handlers/pr.rs`: `handle_create`
4. `handlers/lint.rs`: `handle_lint`
5. `handlers/status.rs` (new): `handle_status`, `handle_next`

### Phase 3: Wire Up CLI Commands

1. Add `handle_rfc_command()` following RFC 0057 pattern
2. Add `handle_worktree_command()` following RFC 0057 pattern
3. Add `handle_pr_command()` following RFC 0057 pattern
4. Implement `blue lint` calling shared handler
5. Implement `blue status` and `blue next`

### Phase 4: Add Missing CLI Commands

MCP has tools not yet exposed via CLI:

| MCP Tool | Proposed CLI |
|----------|--------------|
| `blue_rfc_complete` | `blue rfc complete <title>` |
| `blue_rfc_validate` | `blue rfc validate <title>` |
| `blue_worktree_cleanup` | `blue worktree cleanup` |
| `blue_search` | Already exists: `blue search` |
| `blue_health_check` | `blue health` |
| `blue_sync` | `blue sync` |

## Files Changed

| File | Change |
|------|--------|
| `apps/blue-cli/src/main.rs` | Implement init, wire up commands |
| `crates/blue-mcp/src/handlers/rfc.rs` | Extract standalone functions |
| `crates/blue-mcp/src/handlers/worktree.rs` | Extract standalone functions |
| `crates/blue-mcp/src/handlers/pr.rs` | Extract standalone functions |
| `crates/blue-mcp/src/handlers/lint.rs` | Extract standalone functions |
| `crates/blue-mcp/src/handlers/status.rs` | New file for status/next |
| `crates/blue-mcp/src/server.rs` | Call extracted functions |

## Architecture: No Code Duplication

```
┌─────────────────┐     ┌─────────────────┐
│   CLI (clap)    │     │  MCP Server     │
│                 │     │                 │
│ get_project_    │     │ ensure_state()  │
│ state()         │     │                 │
└────────┬────────┘     └────────┬────────┘
         │                       │
         │  ProjectState         │  ProjectState
         ▼                       ▼
┌─────────────────────────────────────────────┐
│        blue_mcp::handlers::*                │
│                                             │
│  rfc::handle_create(state, args)            │
│  worktree::handle_create(state, args)       │
│  dialogue::handle_create(state, args)       │
│  ...                                        │
└─────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────┐
│        blue_core::*                         │
│                                             │
│  DocumentStore, ProjectState, Rfc, etc.     │
└─────────────────────────────────────────────┘
```

## Test Plan

- [ ] `blue init` creates `.blue/` and `blue.db`
- [ ] `blue init` is idempotent
- [ ] `blue rfc create "Test"` creates RFC in database
- [ ] `blue rfc list` shows RFCs (add this command)
- [ ] `blue worktree create "Test"` creates git worktree
- [ ] `blue worktree list` shows worktrees
- [ ] `blue status` shows accurate project state
- [ ] `blue lint` runs validation checks
- [ ] All commands work in new project after `blue init`

## Migration

No migration needed. The change is purely internal refactoring to share code between CLI and MCP.

---

*"One implementation, two interfaces."*

-- Blue
