# RFC 0003: Per Repo Blue Folders

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Source Spike** | per-repo-blue-folder |

---

## Summary

Currently all docs flow to one central .blue folder. Each repo should have its own .blue folder so docs live with code and git tracking works naturally.

## Current Behavior

```
blue/                          # Central repo
├── .blue/
│   ├── repos/
│   │   ├── blue/docs/...      # Blue's docs
│   │   └── other-repo/docs/   # Other repo's docs (wrong!)
│   └── data/
│       └── blue/blue.db
```

All repos' docs end up in the blue repo's `.blue/repos/`.

## Proposed Behavior

```
repo-a/
├── .blue/
│   ├── docs/
│   │   ├── rfcs/
│   │   ├── spikes/
│   │   └── runbooks/
│   └── blue.db
└── src/...

repo-b/
├── .blue/
│   ├── docs/...
│   └── blue.db
└── src/...
```

Each repo has its own `.blue/` with its own docs and database.

## Changes Required

### 1. Simplify BlueHome structure

```rust
pub struct BlueHome {
    pub root: PathBuf,      // Repo root
    pub blue_dir: PathBuf,  // .blue/
    pub docs_path: PathBuf, // .blue/docs/
    pub db_path: PathBuf,   // .blue/blue.db
}
```

### 2. Change detect_blue behavior

- Find git repo root for current directory
- Look for `.blue/` there (don't search upward beyond repo)
- Auto-create on first blue command (no `blue init` required)

**Edge cases:**
- No git repo: Create `.blue/` in current directory with warning
- Monorepo: One `.blue/` at git root (packages share it)
- Subdirectory: Always resolve to git root

### 3. Flatten docs structure

Before: `.blue/repos/<project>/docs/rfcs/`
After: `.blue/docs/rfcs/`

No need for project subdirectory when per-repo.

### 4. Migration

Automatic on first run:

1. Detect old structure (`.blue/repos/` exists)
2. Find docs for current project in `.blue/repos/<project>/docs/`
3. Move to `.blue/docs/`
4. Migrate database entries
5. Clean up empty directories
6. Log what was migrated

**Conflict resolution:** If docs exist in both locations, prefer newer by mtime.

## Git Tracking

Repos should commit their `.blue/` folder:

**Track:**
- `.blue/docs/**` - RFCs, spikes, runbooks, etc.
- `.blue/blue.db` - SQLite database (source of truth)
- `.blue/config.yaml` - Configuration

**Gitignore:**
- `.blue/*.db-shm` - SQLite shared memory (transient)
- `.blue/*.db-wal` - SQLite write-ahead log (transient)

Recommended `.gitignore` addition:
```
# Blue transient files
.blue/*.db-shm
.blue/*.db-wal
```

## Cross-Repo Coordination

The daemon/realm system (RFC 0001) handles cross-repo concerns:
- Central daemon tracks active sessions
- Realms coordinate contracts between repos
- Each repo remains self-contained

## FAQ

**Q: Do I need to run `blue init`?**
A: No. Blue auto-creates `.blue/` on first command.

**Q: What about my existing docs in the central location?**
A: Auto-migrated on first run. Check git status to verify.

**Q: Should I commit `.blue/blue.db`?**
A: Yes. It's the source of truth for your project's Blue state.

**Q: What if I'm in a monorepo?**
A: One `.blue/` at the git root. All packages share it.

**Q: Can I use Blue without git?**
A: Yes, but with a warning. `.blue/` created in current directory.

**Q: How do I see cross-repo status?**
A: Use `blue realm_status` (requires daemon running).

## Test Plan

- [ ] New repo gets `.blue/` on first blue command
- [ ] Docs created in repo's own `.blue/docs/`
- [ ] Database at `.blue/blue.db`
- [ ] Old structure migrated automatically
- [ ] Realm/daemon still works across repos
- [ ] No git repo falls back gracefully with warning
- [ ] Monorepo uses single `.blue/` at root

---

*"Right then. Let's get to it."*

— Blue
