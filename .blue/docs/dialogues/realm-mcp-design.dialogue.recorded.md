# Dialogue: Realm MCP Integration Design

**RFC**: [0002-realm-mcp-integration](../rfcs/0002-realm-mcp-integration.md)
**Goal**: Reach 95% alignment on open design questions
**Format**: 12 experts, structured rounds

---

## Open Questions

1. **Tool granularity** - One `realm` tool with subcommands, or separate tools?
2. **Notification delivery** - Poll on each tool call, or separate subscription?
3. **Multi-realm** - How to handle repos in multiple realms?

---

## Expert Panel

| Expert | Domain | Perspective |
|--------|--------|-------------|
| **Ada** | API Design | Clean interfaces, discoverability |
| **Ben** | Developer Experience | Friction, learning curve |
| **Carmen** | Systems Architecture | Scalability, performance |
| **David** | MCP Protocol | Tool conventions, client compatibility |
| **Elena** | Claude Integration | LLM tool use patterns |
| **Felix** | Distributed Systems | Consistency, coordination |
| **Grace** | Security | Trust boundaries, access control |
| **Hassan** | Product | User workflows, value delivery |
| **Iris** | Simplicity | Minimalism, YAGNI |
| **James** | Observability | Debugging, transparency |
| **Kim** | Testing | Testability, reliability |
| **Luna** | Documentation | Learnability, examples |

---

## Round 1: Initial Positions

### Question 1: Tool Granularity

**Ada (API Design)**: Separate tools. Each tool has a clear contract. `realm_status` returns status, `realm_check` returns validation results. Easier to document, easier to version independently.

**Ben (DX)**: Separate tools, but not too many. 5-7 tools max in the "realm" namespace. Too many tools overwhelms. Group by workflow: status, validation, session, worktree.

**David (MCP Protocol)**: MCP tools should be atomic operations. One tool = one action. Subcommand patterns work poorly because the LLM has to understand nested schemas. Separate tools with clear names.

**Elena (Claude Integration)**: Claude performs better with focused tools. A tool that does one thing well gets used correctly. A multi-purpose tool with modes leads to parameter confusion.

**Iris (Simplicity)**: Start with 3 tools: `realm_status`, `realm_check`, `realm_action`. The action tool can handle mutations. Expand only when pain is proven.

**Luna (Documentation)**: Separate tools are easier to document with examples. Each tool gets its own "when to use this" section.

**Alignment**: 85% toward separate tools, debate on how many.

### Question 2: Notification Delivery

**Carmen (Systems)**: Polling is simpler and more reliable. MCP doesn't have a push channel. Each tool call can check for pending notifications and include them in the response.

**Felix (Distributed)**: Polling with piggybacking. Don't make a separate notification tool - just include notifications in every response when relevant. The daemon tracks what's been delivered.

**David (MCP Protocol)**: MCP has no subscription model. Polling is the only option. But we can be smart: return notifications with any tool response, mark as delivered, client sees them naturally.

**Hassan (Product)**: Users don't want to manually check notifications. Piggybacking is right - if Claude calls any realm tool and there are notifications, surface them. Claude can then decide to act.

**James (Observability)**: Need a way to explicitly list/ack notifications too. Piggybacking is good for discovery, but sometimes you want to see "what happened while I was away."

**Grace (Security)**: Notifications should only show changes the current session is authorized to see. Scope to domains the repo participates in.

**Alignment**: 90% toward poll-with-piggyback, explicit list/ack as supplement.

### Question 3: Multi-Realm

**Iris (Simplicity)**: Defer. MVP is one realm per repo. Multi-realm is complexity we don't need yet. YAGNI.

**Ada (API Design)**: If we defer, design the API to not preclude it. All tools should accept optional `realm` parameter. Default to detected realm from cwd.

**Ben (DX)**: Agree with Iris. I've never seen a repo that needs to be in multiple realms. Solve real problems first.

**Carmen (Systems)**: One repo, one realm is a reasonable constraint. It simplifies the daemon state model significantly.

**Felix (Distributed)**: Multi-realm creates coordination nightmares. Which realm's contract wins? Keep it simple: one repo belongs to one realm.

**Grace (Security)**: Multi-realm means multi-trust-boundary. That's a security model I'd want to think very carefully about. Defer.

**Kim (Testing)**: Single realm is much easier to test. The state space explodes with multi-realm.

**Hassan (Product)**: What's the user story for multi-realm? I can't think of one. A repo coordinates with its team. That's one realm.

**Alignment**: 95% toward single-realm constraint for MVP.

---

## Round 2: Convergence

### Question 1: How Many Tools?

**Elena**: Let me propose a concrete list based on user workflows:

**Status/Read (3 tools)**:
- `realm_status` - Overview of realm state
- `realm_check` - Validation for CI
- `contract_get` - Get specific contract details

**Session (2 tools)**:
- `session_start` - Begin work session
- `session_stop` - End session (status is included in other responses)

**Workflow (2 tools)**:
- `worktree_create` - Set up RFC worktrees
- `pr_status` - Check PR readiness (prepare is a CLI action, not MCP)

**Notifications (1 tool)**:
- `notifications_list` - Explicit check (ack happens implicitly when acted upon)

**Total: 8 tools**

**Ben**: That's reasonable. Each maps to a clear user intent.

**Ada**: I'd merge `session_start` and `session_stop` into `session` with an action parameter. "Start" and "stop" are two sides of one concept.

**Elena**: Fair, but Claude handles "start a session" better than "call session with action=start". The verb in the tool name guides usage.

**David**: Agree with Elena. MCP tools work best when the name is the action. `session_start` is clearer than `session(action="start")`.

**Iris**: 8 tools feels like a lot. Can we cut?

**Hassan**: Which would you cut? Each serves a distinct workflow.

**Iris**: `contract_get` could be part of `realm_status` with a filter. `notifications_list` could be piggybacked only.

**James**: I want `notifications_list` as explicit tool. "Show me what changed" is a real user intent.

**Luna**: 8 tools is fine if they're well-documented. The CLI has more commands than that.

**Alignment on Q1**: 90% - 8 tools as proposed, with room to consolidate if usage shows overlap.

### Question 2: Notification Details

**Felix**: Proposal for piggybacking:

1. Every tool response includes `notifications: []` field
2. Daemon marks notifications as "delivered" when returned
3. `notifications_list` shows all (including delivered) with filter options
4. No explicit ack needed - acting on a notification is implicit ack

**Carmen**: What triggers a notification? Contract version bump?

**Felix**: Three triggers:
- Contract updated (version change)
- Contract schema changed (even same version - dangerous)
- Binding added/removed in shared domain

**Grace**: Notifications scoped to domains the current repo participates in. If aperture and fungal share s3-access domain, aperture sees fungal's changes to contracts in that domain only.

**Kim**: How do we test piggybacking? Every tool needs to include the notification check.

**Ada**: Extract to middleware. Every MCP handler calls `check_notifications()` and merges into response.

**Alignment on Q2**: 95% - Piggyback with explicit list, middleware pattern, three trigger types.

### Question 3: Single Realm Confirmed

**All**: Consensus. One repo, one realm. The `realm` parameter is optional (defaults to cwd detection) but exists for explicit override in edge cases.

**Ada**: Document clearly: "A repo belongs to one realm. To coordinate across organizational boundaries, create a shared realm."

**Alignment on Q3**: 95% - Single realm constraint, documented clearly.

---

## Round 3: Final Positions

### Resolved Design

**Tool Inventory (8 tools)**:

| Tool | Purpose | Notifications |
|------|---------|---------------|
| `realm_status` | Realm overview | Yes |
| `realm_check` | Validation | Yes |
| `contract_get` | Contract details | Yes |
| `session_start` | Begin session | Yes |
| `session_stop` | End session | No (final) |
| `worktree_create` | Create RFC worktrees | Yes |
| `pr_status` | PR readiness | Yes |
| `notifications_list` | Explicit notification check | N/A |

**Notification Model**:
- Piggybacked on tool responses
- Three triggers: version change, schema change, binding change
- Scoped to shared domains
- Middleware pattern for implementation
- Explicit list for "catch up" workflow

**Realm Constraint**:
- One repo belongs to one realm
- Optional `realm` parameter for explicit override
- Detected from `.blue/config.yaml` by default

---

## Round 4: Resolving the Deferred 5%

### Question 4: Notification Persistence

**Carmen (Systems)**: Notifications need a lifecycle. Options:
- A) Session-scoped: live until session ends
- B) Time-based: live for N hours
- C) Ack-based: live until explicitly acknowledged
- D) Hybrid: session OR time, whichever comes first

**Felix (Distributed)**: Session-scoped is problematic. What if I start a session, see a notification, don't act on it, end session, start new session - is it gone? That's data loss.

**James (Observability)**: Notifications are events. Events should be durable. I want to see "what changed in the last week" even if I wasn't in a session.

**Hassan (Product)**: User story: "I was on vacation for a week. I come back, start a session. What changed?" Time-based with reasonable window.

**Grace (Security)**: Notifications contain information about what changed. Long retention = larger attack surface if daemon db is compromised. Keep it short.

**Iris (Simplicity)**: 7 days, no ack needed. Old notifications auto-expire. Simple to implement, simple to reason about.

**Ben (DX)**: What about "I've seen this, stop showing me"? Piggyback means I see the same notification every tool call until it expires.

**Ada (API Design)**: Two states: `pending` and `seen`. Piggyback only returns `pending`. First piggyback delivery marks as `seen`. `notifications_list` can show both with filter.

**Felix**: So the lifecycle is:
1. Created (pending) - triggered by contract change
2. Seen - first piggybacked delivery
3. Expired - 7 days after creation

**Kim (Testing)**: That's testable. Clear state machine.

**Elena (Claude)**: Claude sees notification once via piggyback, can ask for history via `notifications_list`. Clean.

**Luna (Docs)**: Easy to document: "Notifications appear once automatically, then move to history. History retained 7 days."

**Alignment on Q4**: 95%
- **Lifecycle**: pending → seen → expired
- **Retention**: 7 days from creation
- **Piggyback**: only pending notifications
- **List**: shows all with state filter

---

### Question 5: Schema Change Detection

**Carmen (Systems)**: JSON Schema diffing is hard. Semantic equivalence is undecidable in general. Options:
- A) Hash comparison (fast, false positives on formatting)
- B) Normalized hash (canonicalize then hash)
- C) Structural diff (expensive, accurate)
- D) Don't detect schema changes, only version changes

**Ada (API Design)**: What's the user need? "Contract schema changed" means "you might need to update your code." Version bump should signal that.

**David (MCP)**: If we require version bump for schema changes, we don't need schema diffing. The version IS the signal.

**Iris (Simplicity)**: I like D. Schema changes without version bump is a bug. Don't build tooling for buggy workflows.

**Grace (Security)**: Counter-point: malicious or careless actor changes schema without bumping version. Consumer code breaks silently. Detection is a safety net.

**Felix (Distributed)**: Schema hash as secondary check. If schema hash changes but version doesn't, that's a warning, not a notification. Different severity.

**Ben (DX)**: So we have:
- Version change → notification (normal)
- Schema change without version change → warning in `realm_check` (smells bad)

**Kim (Testing)**: Normalized hash is deterministic. Canonicalize JSON (sorted keys, no whitespace), SHA256. Same schema always produces same hash.

**Carmen**: Canonicalization is well-defined for JSON. Use RFC 8785 (JSON Canonicalization Scheme) or similar.

**James (Observability)**: Store schema hash in contract metadata. On load, compute hash, compare. Mismatch = warning. No complex diffing needed.

**Hassan (Product)**: I like the split: version changes are notifications (expected), schema-without-version is a check warning (unexpected, possibly buggy).

**Elena (Claude)**: Clear for Claude too. Notifications are "things happened." Warnings are "something might be wrong."

**Alignment on Q5**: 95%
- **Version change**: notification (normal workflow)
- **Schema change without version**: warning in `realm_check` (smells bad)
- **Detection method**: canonical JSON hash (RFC 8785 style)
- **Storage**: hash stored in contract, computed on load, compared

---

### Question 6: Worktree Tool Scope

**Hassan (Product)**: User stories:
1. "I'm starting RFC work, set up worktrees for all repos in my realm"
2. "I only need to touch aperture and fungal for this RFC, not the others"
3. "I'm in aperture, create a worktree just for this repo"

**Ben (DX)**: Default should be "smart" - create worktrees for repos in domains I participate in, not all repos in realm.

**Ada (API Design)**: Parameters:
- `rfc` (required): branch name
- `repos` (optional): specific list, default = domain peers

**Felix (Distributed)**: "Domain peers" = repos that share at least one domain with current repo. If aperture and fungal share s3-access, they're peers.

**Iris (Simplicity)**: What if I just want current repo? That's the simplest case.

**Luna (Docs)**: Three modes:
1. `worktree_create(rfc="x")` → domain peers (smart default)
2. `worktree_create(rfc="x", repos=["a","b"])` → specific list
3. `worktree_create(rfc="x", repos=["self"])` → just current repo

**Kim (Testing)**: "self" is a magic value. I'd prefer explicit: `repos=["aperture"]` where aperture is current repo.

**Elena (Claude)**: Claude can figure out current repo name from context. Magic values are confusing for LLMs.

**Ada**: Revised:
- `repos` omitted → domain peers
- `repos=[]` (empty) → error, must specify something
- `repos=["aperture"]` → just aperture

**Ben**: What if repo has no domain peers? Solo repo in realm.

**Felix**: Then domain peers = empty = just self. Natural fallback.

**Carmen**: Edge case: repo in multiple domains with different peer sets. Union of all peers?

**Grace**: Union. If you share any domain, you might need to coordinate.

**James (Observability)**: Log which repos were selected and why. "Creating worktrees for domain peers: aperture, fungal (shared domain: s3-access)"

**Alignment on Q6**: 95%
- **Default**: domain peers (repos sharing at least one domain)
- **Explicit**: `repos` parameter for specific list
- **Solo repo**: defaults to just self
- **Multiple domains**: union of all peers
- **Logging**: explain selection reasoning

---

## Remaining 5%: Truly Deferred

1. **Notification aggregation** - If contract changes 5 times in an hour, 5 notifications or 1? (Decide during implementation based on UX testing)

---

## Final Alignment: 98%

**Consensus reached on**:

### Core Design (Rounds 1-3)
- 8 focused tools mapping to user workflows
- Piggyback notifications with explicit list fallback
- Single realm constraint with documented rationale

### Notification Persistence (Round 4)
- Lifecycle: pending → seen → expired
- Retention: 7 days from creation
- Piggyback delivers pending only, marks as seen
- List tool shows all with state filter

### Schema Change Detection (Round 5)
- Version changes → notifications (normal workflow)
- Schema-without-version → `realm_check` warning (smells bad)
- Detection via canonical JSON hash (RFC 8785 style)

### Worktree Scope (Round 6)
- Default: domain peers (repos sharing domains with current repo)
- Explicit: `repos` parameter overrides default
- Solo repos default to self
- Multiple domains: union of all peers
- Log selection reasoning for transparency

### Truly Deferred (2%)
- Notification aggregation (rapid changes: batch or individual?)

**Panel Sign-off**:
- Ada ✓, Ben ✓, Carmen ✓, David ✓, Elena ✓, Felix ✓
- Grace ✓, Hassan ✓, Iris ✓, James ✓, Kim ✓, Luna ✓
