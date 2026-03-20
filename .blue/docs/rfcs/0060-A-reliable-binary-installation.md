# RFC 0060: Reliable Binary Installation

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-02-06 |
| **Relates To** | RFC 0052 (CLI Hook Management), RFC 0049 (Synchronous Guard) |

---

## Summary

Fix the binary installation flow so it doesn't produce binaries that hang on macOS. Add `cargo install --path` support as the primary install method, and post-copy re-signing as a fallback safety net.

## Problem Statement

### The Bug

When the Blue binary is copied to `~/.cargo/bin/` (or `/usr/local/bin/`), macOS preserves stale extended attributes (`com.apple.provenance`) and the adhoc code signature from the original build. If Homebrew updates a dynamically linked library (e.g., `openssl@3`) between when the binary was built and when it's run, `dyld` hangs indefinitely at `_dyld_start` during signature verification. The process never reaches `main()`.

### Observed Behavior

```
$ blue init
[hangs forever — no output, 0 bytes written to stdout/stderr]

$ sample $PID
890 _dyld_start  (in dyld) + 0    # 100% of samples stuck here
```

- `kill -9` cannot terminate the process (state: `UE` — uninterruptible + exiting)
- The same binary works fine when run from `target/release/blue` directly
- The same binary works fine when copied to `/tmp/` without extended attributes

### Root Cause

`cp` on macOS preserves extended attributes by default. The combination of:
1. `com.apple.provenance` xattr (marks binary as "downloaded/copied")
2. Stale adhoc linker signature from original `cargo build`
3. Updated dylib versions on disk (Homebrew `openssl@3`)

...causes `dyld` to enter a signature verification path that deadlocks.

### Evidence

| Test | Result |
|------|--------|
| `target/release/blue init` | Works (0.1s) |
| `cp` to `~/.cargo/bin/blue` then run | Hangs at `_dyld_start` |
| `cp` to `/tmp/blue-copy` then run | Works (no provenance xattr) |
| Symlink to `target/release/blue` | Works |
| `xattr -cr` + `codesign --force --sign -` | Works |

## Proposal

### 1. Support `cargo install` as primary method

Add workspace metadata so `cargo install --path apps/blue-cli` works correctly. This lets Cargo handle the build-and-install atomically, producing a freshly signed binary.

**INSTALL.md** becomes:
```bash
# Build and install (recommended)
cargo install --path apps/blue-cli

# Then configure for Claude Code
blue install
```

### 2. Post-copy re-signing in `install.sh`

For users who prefer `install.sh` or `cp`:

```bash
cp "$BINARY" "$INSTALL_DIR/blue"
# Fix macOS code signature after copy
if [[ "$OSTYPE" == "darwin"* ]]; then
    xattr -cr "$INSTALL_DIR/blue"
    codesign --force --sign - "$INSTALL_DIR/blue"
fi
```

### 3. Post-copy re-signing in `blue install` (Rust)

The `handle_install_command` currently doesn't copy the binary anywhere — it only sets up hooks, skills, and MCP config. But the `SessionStart` hook adds `target/release/` to PATH, which is fragile. Instead:

- Add an optional `--binary` flag to `blue install` that copies and re-signs the binary to `~/.cargo/bin/`
- Or detect if running from `target/release/` and warn the user

### 4. `blue doctor` validation

Add a check to `blue doctor` that detects:
- Stale code signatures on the installed binary
- Mismatched binary vs source versions
- Presence of `com.apple.provenance` xattr on the binary

## Implementation Plan

### Phase 1: Fix `install.sh` (immediate)

1. Add `xattr -cr` + `codesign --force --sign -` after every `cp` of the binary
2. Add a verification step that actually runs `blue --version` with a timeout

### Phase 2: Support `cargo install --path`

1. Verify `cargo install --path apps/blue-cli` works with the current workspace layout
2. Update `INSTALL.md` to recommend `cargo install` as the primary method
3. Update `install.sh` to use `cargo install` instead of `cp` when possible

### Phase 3: Harden `blue doctor`

1. Add macOS signature check: `codesign --verify` on the installed binary
2. Add xattr check: warn if `com.apple.provenance` is present
3. Add timeout-based liveness check: run `blue --version` with a 3s timeout

## Files Changed

| File | Change |
|------|--------|
| `install.sh` | Add post-copy re-signing for macOS |
| `apps/blue-cli/src/main.rs` | Add doctor checks for signature issues |
| `INSTALL.md` | Recommend `cargo install --path` as primary method |

## Test Plan

- [x] `install.sh` produces a working binary on macOS after Homebrew openssl update
- [x] `cargo install --path apps/blue-cli` produces a working binary
- [x] `blue doctor` detects provenance xattr on binary (warns with fix hint)
- [x] `blue doctor` passes on a freshly installed binary
- [x] Install flow works on Linux (no-op for codesign steps via `#[cfg(target_os = "macos")]`)

---

*"Right then. Let's get to it."*

-- Blue
