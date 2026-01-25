# Spike: Context Injection Mechanisms from coherence-mcp

| | |
|---|---|
| **Date** | 2026-01-25 |
| **Time-box** | 2 hours |
| **Status** | Complete |
| **Outcome** | 7 mechanisms designed; 4 implemented, 3 ready for RFC |

---

## Question

How does coherence-mcp inject functionality into Claude Code sessions without relying on files in `~/.claude/`? How can we bring these capabilities into Blue?

## Investigation

Explored coherence-mcp codebase focusing on:
- Installer module and hook setup
- MCP server resource/prompt capabilities
- Bootstrap and worktree context patterns
- Session lifecycle management

## Findings

### 1. MCP Server Registration (Already in Blue ✅)

Installation modifies `~/.claude.json` to register the MCP server.

**Status**: Blue already does this via `install.sh`.

### 2. Session Hooks (Now Implemented ✅)

coherence-mcp installs hooks to `~/.claude/hooks.json`.

**Implementation**: `install.sh` now configures hooks automatically:

```bash
jq --arg blue_root "$BLUE_ROOT" \
  '.hooks.SessionStart.command = ($blue_root + "/hooks/session-start") |
   .hooks.SessionEnd.command = ($blue_root + "/target/release/blue session-end") |
   .hooks.PreToolUse.command = ($blue_root + "/target/release/blue session-heartbeat") |
   .hooks.PreToolUse.match = "blue_*"' \
  "$HOOKS_FILE"
```

| Hook | Command | Purpose |
|------|---------|---------|
| `SessionStart` | `blue/hooks/session-start` | Inject knowledge + register session |
| `SessionEnd` | `blue session-end` | Clean up session record |
| `PreToolUse` | `blue session-heartbeat` | Keep session alive (match: `blue_*`) |

### 3. Knowledge Injection via Hook (Now Implemented ✅)

**New mechanism** not in original coherence-mcp: Private knowledge documents injected via SessionStart hook.

**Architecture**:
```
Any Repo (cwd)                     Blue Repo (fixed location)
┌─────────────────┐               ┌─────────────────────────────┐
│ fungal-image-   │               │ /path/to/blue               │
│ analysis/       │               │                             │
│                 │  SessionStart │ hooks/session-start ────────┤
│                 │ ────────────→ │ knowledge/alignment-measure │
│                 │               │ knowledge/... (future)      │
│                 │  ←─────────── │                             │
│                 │   stdout      │                             │
└─────────────────┘   (injected)  └─────────────────────────────┘
```

**How it works**:
1. Hook script reads from `blue/knowledge/*.md`
2. Outputs content wrapped in `<blue-knowledge>` tags
3. Claude Code captures stdout as `<system-reminder>`
4. Content injected into Claude's context for that session

**Files created**:
- `hooks/session-start` - Shell script that injects knowledge
- `knowledge/alignment-measure.md` - ALIGNMENT scoring framework

**Phase progression**:

| Phase | Source | Command |
|-------|--------|---------|
| **Now** | `blue/knowledge/*.md` | `cat` in hook script |
| **Future** | `~/.blue/knowledge.db` | `blue knowledge get --decrypt` |

### 4. MCP Resources for Autocomplete (Not Yet Implemented ❌)

coherence-mcp exposes documents as MCP resources for `@` autocomplete.

**Blue should expose**:
```
blue://rfc/{title}           # RFC documents
blue://rfc/{title}/plan      # RFC plan documents
blue://spike/{title}         # Spike investigations
blue://adr/{number}          # Architecture Decision Records
blue://prd/{title}           # Product Requirements Documents
blue://pattern/{name}        # Pattern specifications
blue://dialogue/{title}      # Alignment dialogues
blue://contract/{name}       # Component contracts (.blue/contracts/)
blue://knowledge/{name}      # Private knowledge docs
```

**Gap**: Blue has no MCP resource implementation yet.

**Priority**: Medium - would improve discoverability and allow `@blue://rfc/...` references.

### 5. Bootstrap Pattern (No Longer Needed ✅)

Each coherence-mcp project includes a bootstrap context file.

**Original gap**: Blue had CLAUDE.md but only in the Blue repo itself.

**Solution**: Replaced by injection mechanism:
- **Global knowledge**: Injected from `blue/knowledge/*.md`
- **Project workflow**: Injected from `.blue/workflow.md` (if exists in project)
- **Team visibility**: `.blue/workflow.md` is committed to git

Projects can create `.blue/workflow.md` with project-specific guidance:
```markdown
# Project Workflow

This project uses feature branches off `main`.
RFCs should reference the product roadmap in `/docs/roadmap.md`.
Run `npm test` before committing.
```

This file gets injected via SessionStart hook automatically.

**Workflow Creation Assistance**:

Two mechanisms help users create `.blue/workflow.md`:

1. **Hint in `blue_status`**: When workflow.md is missing, status returns:
   ```json
   {
     "hint": "No .blue/workflow.md found. Ask me to help set up project workflow."
   }
   ```

2. **Knowledge injection**: `knowledge/workflow-creation.md` teaches Claude how to:
   - Analyze project structure (Cargo.toml, package.json, etc.)
   - Ask clarifying questions (branching, CI, test requirements)
   - Generate customized workflow.md via Write tool

No dedicated MCP tool needed - Claude handles creation conversationally.

### 6. Worktree Context Injection (Design Updated ✅)

coherence-mcp injects CLAUDE.md files into worktrees.

**Blue approach**: Use knowledge injection instead of CLAUDE.md files.

When SessionStart detects we're in a worktree (`.blue/worktree.json` exists), inject:
- RFC title and summary
- Current phase
- Success criteria
- Linked documents

```bash
# In hooks/session-start
if [ -f ".blue/worktree.json" ]; then
    # Extract RFC info and inject as context
    "$BLUE_ROOT/target/release/blue" worktree-context
fi
```

**Advantages over CLAUDE.md**:
- No file clutter in worktrees
- Context stays fresh (read from RFC, not static file)
- Consistent with other injection patterns
- RFC changes automatically reflected

### 7. Activity Detection (Design Updated ✅)

coherence-mcp tracks activity levels via heartbeat.

**Status**: Blue has session tracking and heartbeat via `PreToolUse` hook.

**Hybrid Approach** (recommended):

```
┌─────────────────────────────────────────────────────────────┐
│  Activity Detection                                         │
│                                                             │
│  Primary: Heartbeat                                         │
│  ├── PreToolUse hook → blue session-heartbeat               │
│  ├── Detects worktree → links to RFC                        │
│  └── Updates session.last_heartbeat + rfc.last_activity     │
│                                                             │
│  Fallback: Git (when no recent heartbeat)                   │
│  ├── Check worktree for uncommitted changes                 │
│  └── Check branch for recent commits                        │
└─────────────────────────────────────────────────────────────┘
```

**Activity Levels**:

| Level | Condition | Icon |
|-------|-----------|------|
| ACTIVE | Heartbeat <5 min | 🟢 |
| RECENT | Activity <30 min | 🟡 |
| STALE | No activity >24h | 🟠 |
| CHANGES | Uncommitted changes (git fallback) | 🔵 |

**Tool Integration**:

| Tool | Behavior |
|------|----------|
| `blue_status` | Shows activity level per RFC |
| `blue_next` | Skips ACTIVE RFCs, prioritizes STALE |
| `blue_worktree_create` | Warns if RFC already active elsewhere |

**Implementation**:

1. **Schema**: Add `last_activity TEXT` to RFCs table
2. **Heartbeat**: Detect worktree via `.blue/worktree.json`, update linked RFC
3. **Activity function**: Calculate level from timestamps, git fallback
4. **Integration**: Update `blue_status`/`blue_next` to show/use activity levels

---

## Implementation Summary

### Completed

| Item | Files | Description |
|------|-------|-------------|
| Session Hooks | `install.sh` | Auto-configures `~/.claude/hooks.json` |
| Global Knowledge Injection | `hooks/session-start` | Injects `knowledge/*.md` on SessionStart |
| Project Workflow Injection | `hooks/session-start` | Injects `.blue/workflow.md` from current project |
| ALIGNMENT Framework | `knowledge/alignment-measure.md` | Scoring guidance for Claude |
| Workflow Creation Guide | `knowledge/workflow-creation.md` | Teaches Claude to help create workflow.md |
| Bootstrap Pattern | (superseded) | Replaced by injection - no separate template needed |
| Worktree Context Design | (in spike) | Use injection instead of CLAUDE.md files |
| Activity Detection Design | (in spike) | Hybrid: heartbeat + git fallback |
| Branch/Worktree Naming | (in spike) | Configurable prefix, default `feature/` |

### Remaining Implementation

| Item | Priority | Notes |
|------|----------|-------|
| MCP Resources | Medium | `blue://` autocomplete for RFCs, PRDs, patterns, dialogues, contracts, plans |
| Worktree Context Injection | Medium | `blue worktree-context` command |
| Activity Detection | Medium | Hybrid heartbeat + git fallback, update status/next |
| Branch/Worktree Naming | Medium | Configurable prefix (default `feature/`), context enforcement |
| `blue_status` workflow hint | Low | Hint when `.blue/workflow.md` missing |
| Encrypted Storage | Future | SQLite with AES-256-GCM |

---

## Updated Gap Analysis

| Mechanism | coherence-mcp | Blue | Status |
|-----------|--------------|------|--------|
| MCP Server Registration | ✅ | ✅ | Done |
| Session Hooks | ✅ | ✅ | **Implemented** |
| Global Knowledge Injection | ❌ | ✅ | **New in Blue** |
| Project Workflow Injection | ❌ | ✅ | **New in Blue** (`.blue/workflow.md`) |
| MCP Resources | ✅ | ❌ | Not yet |
| Bootstrap Pattern | ✅ | ✅ | **Superseded by injection** |
| Worktree Context | ✅ (CLAUDE.md) | ✅ (injection) | **Designed** |
| Activity Detection | ✅ | ✅ | **Designed** (hybrid: heartbeat + git fallback) |

---

## Remaining RFCs

### RFC 0016: MCP Resources

Implement in `crates/blue-mcp/src/server.rs`:

```rust
// In initialize response
"capabilities": { "resources": {} }

// New handlers
"resources/list" => handle_resources_list()
"resources/read" => handle_resources_read(uri)
```

Resources to expose:

| URI Pattern | Description |
|-------------|-------------|
| `blue://rfc/{title}` | RFC documents |
| `blue://rfc/{title}/plan` | RFC plan documents |
| `blue://spike/{title}` | Spike investigations |
| `blue://adr/{number}` | Architecture Decision Records |
| `blue://prd/{title}` | Product Requirements Documents |
| `blue://pattern/{name}` | Pattern specifications |
| `blue://dialogue/{title}` | Alignment dialogues |
| `blue://contract/{name}` | Component contracts (`.blue/contracts/`) |
| `blue://knowledge/{name}` | Private knowledge docs |

**Autocomplete example**:
```
@blue://rfc/alignment-dialogue-architecture
@blue://pattern/alignment-dialogue
@blue://prd/semantic-index
```

**Implementation**:
```rust
fn handle_resources_list(&self) -> Result<Vec<Resource>> {
    let mut resources = vec![];

    // RFCs
    for rfc in self.state.rfcs()? {
        resources.push(Resource {
            uri: format!("blue://rfc/{}", rfc.slug()),
            name: rfc.title.clone(),
            mime_type: Some("text/markdown".into()),
        });
        if rfc.has_plan() {
            resources.push(Resource {
                uri: format!("blue://rfc/{}/plan", rfc.slug()),
                name: format!("{} (Plan)", rfc.title),
                mime_type: Some("text/markdown".into()),
            });
        }
    }

    // PRDs, patterns, dialogues, etc.
    // ...

    Ok(resources)
}
```

### RFC 0017: Encrypted Knowledge Storage (Future)

Migrate from plaintext `knowledge/*.md` to encrypted SQLite:

```sql
CREATE TABLE knowledge (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    content_encrypted BLOB,  -- AES-256-GCM
    content_hash TEXT,       -- SHA-256 integrity
    created_at TEXT,
    updated_at TEXT
);
```

Access via:
```bash
blue knowledge get alignment-measure --decrypt
```

Hook would call this instead of `cat`:
```bash
# Phase 2: Encrypted storage
"$BLUE_ROOT/target/release/blue" knowledge get alignment-measure --decrypt
```

---

## Conclusion

Blue now **exceeds coherence-mcp** for context injection:

| Feature | coherence-mcp | Blue |
|---------|--------------|------|
| Session hooks | ✅ | ✅ |
| Activity tracking | ✅ | ✅ (hybrid design) |
| Global knowledge injection | ❌ | ✅ |
| Project workflow injection | ❌ | ✅ |
| Worktree context | ✅ (CLAUDE.md) | ✅ (injection design) |
| Bootstrap pattern | ✅ (manual) | ✅ (superseded by injection) |

**What Blue adds**:
- Private knowledge docs injected from `blue/knowledge/` (not in `~/.claude/`)
- Project-specific workflow from `.blue/workflow.md` (committed to git, auto-injected)
- Worktree context via injection (no CLAUDE.md clutter)
- Workflow creation assistance via injected knowledge
- Hybrid activity detection (heartbeat + git fallback)

**Path to encryption**:
1. Currently: `knowledge/*.md` in blue repo (plaintext)
2. Future: `~/.blue/knowledge.db` (encrypted SQLite)
3. Hook command changes from `cat` to `blue knowledge get --decrypt`

---

## 8. Branch/Worktree Naming Convention (Design Added ✅)

Coherence-MCP enforced `feature/{title}` branches and `.worktrees/feature/{title}` paths.

Blue currently uses `{stripped-name}` with no prefix.

**Design**: Configurable prefix with `feature/` default.

```yaml
# .blue/config.yaml (or .blue/blue.toml)
[worktree]
branch_prefix = "feature/"  # default
```

**Behavior**:
- Branch: `{prefix}{stripped-name}` → `feature/my-rfc-title`
- Worktree: `~/.blue/worktrees/{prefix}{stripped-name}`
- Each repo defines its own prefix (no realm-level override)
- Default: `feature/` if not specified

**Context Enforcement** (from coherence-mcp):
```rust
// Prevent modifying RFC from wrong branch
if let Some(prefix) = &config.branch_prefix {
    if current_branch.starts_with(prefix) {
        let current_rfc = current_branch.strip_prefix(prefix).unwrap();
        if current_rfc != rfc_title {
            return Err("Cannot modify RFC from different feature branch");
        }
    }
}
```

**Migration**: Existing worktrees without prefix continue to work; new ones use configured prefix.

---

## Next Steps

Create RFCs for remaining implementation:

| RFC | Scope |
|-----|-------|
| RFC 0016 | MCP Resources (`blue://rfc/*`, `blue://prd/*`, etc.) |
| RFC 0017 | Worktree Context Injection (`blue worktree-context`) |
| RFC 0018 | Activity Detection (hybrid heartbeat + git) |
| RFC 0019 | Branch/Worktree Naming Convention (configurable prefix) |
| RFC 0020 | Encrypted Knowledge Storage (future) |

---

*Spike complete. All 7 injection mechanisms from coherence-mcp have been analyzed and designed for Blue.*

*"The best documentation is the documentation that appears when you need it."*

— Blue

---

*"The best documentation is the documentation that appears when you need it."*

— Blue
