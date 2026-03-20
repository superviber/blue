# RFC 0049: Synchronous Guard Command

**Status**: Implemented
**Created**: 2026-02-01
**Author**: 💙 Judge (via alignment dialogue)
**Related**: RFC 0038 (SDLC Workflow Discipline)

## Problem Statement

The `blue guard` command validates file writes against worktree rules (RFC 0038). Currently it runs as an async function within `#[tokio::main]`, which causes hanging issues when invoked from Claude Code PreToolUse hooks.

### Root Cause

The guard command performs only synchronous operations:
- Environment variable checks (`BLUE_BYPASS_WORKTREE`)
- Path pattern matching (allowlist)
- Filesystem reads (`.git` file/directory)
- Subprocess execution (`git branch --show-current`)

None of these require async, but the tokio runtime initialization adds:
1. Thread pool creation overhead
2. Potential resource contention in hook contexts
3. Failure modes when spawned from non-tokio parent processes

### Remaining Issue: PATH Lookup

Even with synchronous guard, PATH-based command lookup hangs in Claude Code's hook environment. The hook must use a full binary path:
```bash
/Users/ericg/letemcook/blue/target/release/blue guard --path="$FILE_PATH"
```

This is a Claude Code subprocess environment issue, not a Blue issue.

## Proposed Solution

Run the guard command synchronously **before** tokio runtime initialization.

### Implementation

```rust
fn main() {
    // Fast-path: handle guard before tokio
    if let Some(exit_code) = maybe_handle_guard() {
        std::process::exit(exit_code);
    }

    // Normal path: tokio runtime for everything else
    tokio_main();
}

fn maybe_handle_guard() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 && args[1] == "guard" {
        let path = args.iter()
            .find(|a| a.starts_with("--path="))
            .map(|a| &a[7..]);

        if let Some(path) = path {
            return Some(run_guard_sync(path));
        }
    }
    None
}

fn run_guard_sync(path: &str) -> i32 {
    // Synchronous implementation of guard logic
    // No tokio, no tracing, just the check
    // ...
}

#[tokio::main]
async fn tokio_main() -> Result<()> {
    // Existing async main logic
}
```

### Benefits

1. **Eliminates hanging**: No tokio runtime to initialize
2. **Faster execution**: Microseconds instead of milliseconds
3. **Simpler hook integration**: No stdin/stdout complexity
4. **Correct architecture**: Pre-init gates don't depend on post-init infrastructure

### Changes Required

1. Add `maybe_handle_guard()` function in `apps/blue-cli/src/main.rs`
2. Implement `run_guard_sync()` with same logic as current async version
3. Update hook script to use simple `blue guard --path="$FILE_PATH"`
4. Remove hardcoded binary path from hook

## Alignment Dialogue

This RFC emerged from an alignment dialogue with 5 experts. Key insights:

| Expert | Perspective |
|--------|-------------|
| 🧁 Muffin (Systems Architect) | Pre-init gates must not depend on post-init infrastructure |
| 🧁 Cupcake (CLI UX Designer) | Fast-path validation gains nothing from async |
| 🧁 Scone (DevOps Engineer) | Hook context resource starvation avoided with sync |
| 🧁 Eclair (Security Analyst) | Sync prevents runtime initialization deadlock |
| 🧁 Donut (Minimalist) | Guard has no actual async work - signature is misleading |

**Convergence**: Unanimous in Round 0. All experts independently concluded guard should be synchronous.

## Open Questions

1. **Future extensibility**: What if guard needs to call daemon APIs later?
   - Answer: Create a separate async guard command (e.g., `blue guard-async`) if needed

2. **Pattern consistency**: This makes guard an exception to async-first design
   - Answer: Pre-flight validation is a legitimate exception case

## Implementation Plan

- [x] Add `maybe_handle_guard_sync()` pre-tokio check
- [x] Implement `run_guard_sync()` with current logic
- [x] Add `is_in_allowlist_sync()` helper
- [x] Add `is_source_code_path_sync()` helper
- [x] Add `main()` entry point that checks guard before tokio
- [ ] ~~Update hook script to remove full path~~ (blocked by Claude Code PATH issue)
- [ ] ~~Remove workaround code~~ (blocked by Claude Code PATH issue)

## References

- RFC 0038: SDLC Workflow Discipline (introduced guard command)
- ADR 0014: Alignment Dialogue Agents (used to deliberate this RFC)
- Dialogue: `2026-02-01T2214Z-guard-command-architecture.dialogue.recorded.md`
