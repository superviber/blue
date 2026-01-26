# RFC 0002: Realm MCP Integration

| | |
|---|---|
| **Status** | Implemented |
| **Created** | 2026-01-24 |
| **Depends On** | [RFC 0001: Cross-Repo Coordination with Realms](0001-cross-repo-realms.md) |
| **Dialogue** | [realm-mcp-design.dialogue.md](../dialogues/realm-mcp-design.dialogue.md) |
| **Alignment** | 98% (12 experts, 6 rounds) |

---

## Problem

RFC 0001 implemented realm coordination via CLI commands. Claude sessions currently have to shell out to use them:

```bash
blue realm status
blue realm check
```

This works but has limitations:
1. No structured data - Claude parses text output
2. No push notifications - Claude must poll for changes
3. No context awareness - tools don't know current session state

## Goals

1. **Native MCP tools** - Claude calls realm functions directly with structured input/output
2. **Session integration** - Tools aware of current repo, realm, and active RFC
3. **Notifications** - Claude receives contract change notifications during sessions
4. **Guided workflow** - Tools suggest next steps based on realm state

## Non-Goals

- Replacing CLI commands (MCP complements, doesn't replace)
- Cross-machine coordination (daemon is local only for MVP)
- Automatic code generation from contracts (future scope)
- Multi-realm support (one repo belongs to one realm)

---

## Proposal

### MCP Tools (8 total)

#### Status & Read (3 tools)

| Tool | Description | Returns |
|------|-------------|---------|
| `realm_status` | Realm overview | Repos, domains, contracts, bindings, sessions |
| `realm_check` | Validation for CI | Errors, warnings (including schema-without-version) |
| `contract_get` | Contract details | Full contract with schema, value, version history |

#### Session (2 tools)

| Tool | Description | Returns |
|------|-------------|---------|
| `session_start` | Begin work session | Session ID, realm context |
| `session_stop` | End session | Summary of changes made |

#### Workflow (2 tools)

| Tool | Description | Returns |
|------|-------------|---------|
| `worktree_create` | Create RFC worktrees | Paths, branches created, repos selected |
| `pr_status` | PR readiness across repos | Uncommitted changes, commits ahead |

#### Notifications (1 tool)

| Tool | Description | Returns |
|------|-------------|---------|
| `notifications_list` | Get notification history | All notifications with state filter |

### Context Awareness

MCP tools automatically detect context from:

1. **Current directory** - Read `.blue/config.yaml` for realm/repo
2. **Active session** - Read `.blue/session` for session ID
3. **Daemon state** - Query for notifications, other sessions

Optional `realm` parameter allows explicit override for edge cases.

### Notification Model

**Delivery: Piggyback with explicit list**

Every tool response includes pending notifications. This provides natural discovery without separate polling.

```json
{
  "result": { ... },
  "notifications": [
    {
      "id": "notif-123",
      "type": "contract_updated",
      "realm": "aperture",
      "domain": "s3-access",
      "contract": "s3-permissions",
      "from_repo": "fungal",
      "old_version": "1.0.0",
      "new_version": "1.1.0",
      "state": "pending",
      "created_at": "2026-01-24T12:00:00Z"
    }
  ],
  "next_steps": ["Review contract changes from fungal"]
}
```

**Lifecycle: pending → seen → expired**

1. **Pending** - Created when trigger fires
2. **Seen** - Marked on first piggyback delivery
3. **Expired** - 7 days after creation (auto-cleanup)

Piggyback only delivers `pending` notifications. `notifications_list` shows all states with filters.

**Triggers (3 types)**

| Trigger | Severity | Destination |
|---------|----------|-------------|
| Contract version change | Notification | Piggyback + list |
| Contract schema change (same version) | Warning | `realm_check` only |
| Binding added/removed in shared domain | Notification | Piggyback + list |

**Scope**: Notifications only for domains the current repo participates in.

### Schema Change Detection

Detect schema changes via canonical JSON hash (RFC 8785 style):

1. Store schema hash in contract metadata on save
2. Compute hash on load, compare to stored
3. Mismatch with same version = warning in `realm_check`

This catches accidental/malicious schema changes without version bumps.

### Worktree Scope

`worktree_create` parameters:
- `rfc` (required) - Branch name for worktrees
- `repos` (optional) - Specific repos to create worktrees for

**Default behavior (no `repos` specified):**
- Select "domain peers" - repos sharing at least one domain with current repo
- Solo repo in realm defaults to just self
- Multiple domains: union of all peers

**Logging**: Tool response explains why repos were selected:
```json
{
  "created": ["aperture", "fungal"],
  "reason": "Domain peers via s3-access domain",
  "paths": {
    "aperture": "/Users/eric/.blue/worktrees/aperture/rfc-123/aperture",
    "fungal": "/Users/eric/.blue/worktrees/aperture/rfc-123/fungal"
  }
}
```

### Guided Workflow

All tools return `next_steps` suggestions based on state:

```json
{
  "result": { ... },
  "next_steps": [
    "Run realm_check to validate changes",
    "Contract s3-permissions was updated - review changes"
  ]
}
```

---

## Implementation Phases

### Phase 1: Core Tools ✓
- `realm_status`, `realm_check`, `contract_get`
- Context detection from cwd
- Middleware for notification injection (deferred to Phase 4)

### Phase 2: Session Tools ✓
- `session_start`, `session_stop`
- Session-scoped context via `.blue/session` file
- Tracks active RFC, domains, contracts modified/watched
- Daemon integration deferred to Phase 4

### Phase 3: Workflow Tools ✓
- `realm_worktree_create` with domain peer selection
- `realm_pr_status` across worktrees
- Creates worktrees under `~/.blue/worktrees/<realm>/<rfc>/`
- Auto-selects domain peers (repos sharing domains with current repo)

### Phase 4: Notifications ✓
- `notifications_list` with state filters (pending, seen, expired, all)
- Schema hash detection via canonical JSON (SHA-256)
- 7-day expiration cleanup on notification list
- Notification piggybacking on realm_status and realm_check
- DaemonDb extended with list_notifications_with_state and cleanup_expired_notifications

---

## Example Session

```
Human: what's the realm status?

Claude: [calls realm_status]

MCP returns:
{
  "realm": "aperture",
  "repos": ["blue", "fungal"],
  "domains": [{
    "name": "s3-access",
    "contracts": [{"name": "s3-permissions", "version": "1.0.0", "owner": "blue"}],
    "bindings": [
      {"repo": "blue", "role": "provider"},
      {"repo": "fungal", "role": "consumer"}
    ]
  }],
  "current_repo": "blue",
  "session": null,
  "notifications": [],
  "next_steps": ["Start a session with session_start to track your work"]
}

Claude: You're in the blue repo, part of the aperture realm.
There's one domain (s3-access) where blue provides the s3-permissions
contract and fungal consumes it. No active session. Want me to start one?
```

---

## Resolved Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Tool granularity | 8 separate tools | Focused tools work better with LLMs; clear contracts |
| Notification delivery | Piggyback + explicit list | Natural discovery; no separate polling |
| Multi-realm | Single realm per repo | Simplicity; no real user need for multi-realm |
| Notification persistence | 7 days, pending→seen→expired | Balance between history and cleanup |
| Schema detection | Canonical JSON hash | Catches bugs without complex diffing |
| Worktree scope | Domain peers by default | Smart default; explicit override available |

## Deferred (2%)

- **Notification aggregation** - If contract changes 5 times rapidly, batch into 1 or send 5? Decide during implementation based on UX testing.
