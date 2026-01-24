# Spike: Cross-Repo Coordination

| | |
|---|---|
| **Status** | Complete |
| **Outcome** | Recommends Implementation |
| **RFC** | [0001-cross-repo-realms](../rfcs/0001-cross-repo-realms.md) |
| **Question** | How can Blue sessions in different repos be aware of each other and coordinate changes when repos have dependencies? |
| **Time Box** | 2 hours |
| **Started** | 2026-01-24 |

---

## Context

We have repos with cross-repo dependencies:
- `aperture` (training-tools webapp) - runs in Account A
- `fungal-image-analysis` - runs in Account B, grants IAM access to Account A

When changes are made in one repo (e.g., adding a new S3 path pattern), the corresponding changes must be made in the other (e.g., updating IAM policies).

### Current Pain Points

1. **No awareness** - Blue session in repo A doesn't know repo B exists
2. **No dependency graph** - Changes to IAM policies don't trigger awareness of dependent services
3. **Manual coordination** - Developer must remember to update both repos
4. **Planning blindness** - RFCs in one repo can't reference or depend on RFCs in another

---

## Research Areas

### 1. Dependency Declaration

How do we declare cross-repo dependencies?

**Option A: Blue manifest file**
```yaml
# .blue/manifest.yaml
dependencies:
  - repo: ../fungal-image-analysis
    type: infrastructure
    resources:
      - cdk/training_tools_access_stack.py
```

**Option B: In-document links**
```markdown
<!-- In RFC -->
| **Cross-Repo** | [fungal-image-analysis](../fungal-image-analysis) |
```

**Option C: Centralized registry**
```
# ~/.blue/repos.yaml (or domain-level DB)
repos:
  aperture:
    path: /Users/ericg/letemcook/aperture
    depends_on: [fungal-image-analysis]
  fungal-image-analysis:
    path: /Users/ericg/letemcook/fungal-image-analysis
    depended_by: [aperture]
```

### 2. Session Coordination

How do Blue sessions communicate?

**Option A: Shared SQLite (domain store)**
- All repos in a domain share a single `.data/domain.db`
- Sessions register themselves and their active RFCs
- Can query "who else is working on related changes?"

**Option B: File-based signals**
- Write `.blue/active-session.json` with current work
- Other sessions poll or watch for changes

**Option C: IPC/Socket**
- Blue MCP server listens on a socket
- Sessions can query each other directly
- More complex but real-time

### 3. Change Propagation

When a change is made in repo A that affects repo B, what happens?

**Option A: Manual notification**
```
⚠️ This change affects dependent repo: fungal-image-analysis
   - cdk/training_tools_access_stack.py may need updates
   Run: blue_cross_repo_check
```

**Option B: Automatic RFC creation**
- Detect affected files via dependency graph
- Create draft RFC in dependent repo
- Link the RFCs together

**Option C: Unified worktree**
- Create worktrees in both repos simultaneously
- Single branch name spans repos
- Coordinate commits

### 4. Planning Integration

How do cross-repo RFCs work together?

**Requirements:**
- RFC in repo A can declare dependency on RFC in repo B
- Status changes propagate (can't implement A until B is accepted)
- Plan tasks can span repos

**Proposal:**
```markdown
| **Depends On** | fungal-image-analysis:rfc-0060-cross-account-access |
| **Blocks** | aperture:rfc-0045-training-metrics |
```

---

## Findings

### Key Insight: Domain-Level Store

The cleanest solution is a **domain-level store** that sits above individual repos:

```
~/.blue/domains/
  letemcook/
    domain.db          # Cross-repo coordination
    repos.yaml         # Repo registry
    sessions/          # Active sessions
```

This enables:
1. Single source of truth for repo relationships
2. Cross-repo RFC dependencies
3. Session awareness without IPC complexity
4. Centralized audit of cross-repo changes

### Proposed Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Domain Store                          │
│  ~/.blue/domains/letemcook/domain.db                    │
│  - repos table (path, name, dependencies)               │
│  - cross_repo_links table (source_rfc, target_rfc)     │
│  - active_sessions table (repo, rfc, agent_id)         │
└─────────────────────────────────────────────────────────┘
           │                              │
           ▼                              ▼
┌─────────────────────┐      ┌─────────────────────┐
│     aperture        │      │ fungal-image-analysis│
│  .blue/blue.db      │      │  .blue/blue.db      │
│  docs/rfcs/         │      │  docs/rfcs/         │
└─────────────────────┘      └─────────────────────┘
```

### New Tools Needed

1. `blue_domain_init` - Initialize domain, register repos
2. `blue_domain_link` - Link two repos as dependencies
3. `blue_cross_repo_check` - Check if changes affect other repos
4. `blue_cross_repo_rfc` - Create linked RFCs across repos

---

## Outcome

**Recommendation:** Implement domain-level store with cross-repo RFC linking.

**Next Steps:**
1. Design domain store schema (new RFC)
2. Add domain detection to Blue startup
3. Implement cross-repo RFC dependencies
4. Add change impact detection

---

## Notes

- Start simple: just repo registry + session awareness
- Don't over-engineer IPC - polling shared DB is sufficient
- Consider git worktree naming conventions that span repos
