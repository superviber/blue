# Spike: Native Kanban Apps for Blue (macOS and iOS)

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

How would macOS and iPhone apps interact with Blue via a kanban board, starting with local connectivity and evolving toward a hosted Blue instance in EC2?

---

## Related Spikes

- [Thin Plugin / Fat Binary](2026-01-26-thin-plugin-fat-binary.md) — the app is another thin surface; intelligence stays in the compiled daemon
- [Blue Plugin Architecture](2026-01-26-blue-plugin-architecture.md) — app is a visual client alongside the Claude Code plugin client
- [ClaudeBot Functionality Transfer](2026-01-26-claudebot-functionality-transfer-opportunities.md) — smart output management via temp files; the app is the richer output surface

## Investigation

### What Blue Exposes Today

The daemon (`127.0.0.1:7865`) serves 7 HTTP endpoints:

| Endpoint | Method | Returns |
|----------|--------|---------|
| `/health` | GET | Status, version |
| `/realms` | GET | All tracked realms |
| `/realms/{name}` | GET | Specific realm |
| `/realms/{name}/sync` | POST | Trigger sync |
| `/sessions` | GET | Active work sessions |
| `/sessions` | POST | Register session |
| `/sessions/{id}` | DELETE | End session |
| `/notifications` | GET | Pending notifications |
| `/notifications/{id}/ack` | POST | Acknowledge notification |

**Not exposed over HTTP**: Documents (RFCs, spikes, ADRs, audits), tasks, project state, semantic index, dialogues. These are MCP-only today.

### Kanban Model: Blue Documents as Cards

Blue documents have natural kanban columns:

**RFCs**:
```
Draft → Dialogue → Final → Implemented → Superseded
```

**Spikes**:
```
In Progress → Complete (no-action | decision-made | recommends-implementation)
```

**PRDs**:
```
Draft → Approved → Complete
```

**Audits**:
```
Open → Complete
```

**Tasks (from RFC plan files)**:
```
Pending → In Progress → Completed
```

A kanban board could show one of:
- **Document-centric**: All Blue documents as cards, columns = status. Filter by type (RFC, spike, ADR).
- **Task-centric**: RFC plan tasks as cards. Each RFC is a swimlane. Columns = pending/in-progress/completed.
- **Mixed**: Top row = active documents. Bottom row = tasks within the current RFC.

The task-centric view is probably the most useful for day-to-day work. The document-centric view is better for project overview.

### Phase 1: Local Connection (macOS App)

```
┌─────────────┐      HTTP       ┌──────────────┐
│  macOS App   │ ◄────────────► │ Blue Daemon   │
│  (SwiftUI)   │   localhost    │ (127.0.0.1:   │
│              │    :7865       │   7865)        │
└─────────────┘                 └──────────────┘
                                       │
                                       ▼
                                ┌──────────────┐
                                │ ~/.blue/      │
                                │ project/.blue/│
                                │ daemon.db     │
                                └──────────────┘
```

**What needs to change in the daemon**:

1. **Document CRUD endpoints** — the daemon needs to expose documents, not just coordination metadata:
   - `GET /projects/{path}/rfcs` → list RFCs with status
   - `GET /projects/{path}/rfcs/{slug}` → single RFC content
   - `PATCH /projects/{path}/rfcs/{slug}` → update status
   - Same pattern for spikes, ADRs, audits, PRDs
   - `GET /projects/{path}/tasks` → plan file tasks
   - `PATCH /projects/{path}/tasks/{id}` → update task status

2. **Project discovery** — the daemon needs to know about local projects:
   - `GET /projects` → list all `.blue/`-containing directories
   - Or: register projects explicitly, like realms

3. **Real-time updates** — kanban needs live state:
   - WebSocket at `ws://localhost:7865/ws` or Server-Sent Events at `/events`
   - Push events: document created, status changed, task moved, session started/ended
   - The app subscribes and updates the board without polling

4. **File output path** — from the ClaudeBot spike, Blue MCP tools can write full content to temp files and return paths. The app could watch those paths and render rich previews instead of raw markdown in the terminal.

**SwiftUI app structure**:

```
BlueApp/
├── BlueApp.swift              (app entry, scene)
├── Models/
│   ├── BlueClient.swift       (HTTP + WebSocket client)
│   ├── Project.swift           (project model)
│   ├── Document.swift          (RFC, spike, ADR, etc.)
│   └── Task.swift              (plan file tasks)
├── Views/
│   ├── Sidebar/
│   │   ├── ProjectListView.swift
│   │   └── ProjectRow.swift
│   ├── Board/
│   │   ├── KanbanBoardView.swift
│   │   ├── KanbanColumnView.swift
│   │   └── CardView.swift
│   ├── Detail/
│   │   ├── DocumentDetailView.swift
│   │   └── MarkdownRenderer.swift
│   └── Status/
│       ├── SessionBadge.swift
│       └── NotificationList.swift
└── BlueApp.entitlements        (network client)
```

**Shared codebase**: SwiftUI compiles for both macOS and iOS. One codebase, two targets. The board layout adapts — multi-column on Mac, scrollable columns on iPhone.

### Phase 2: iPhone App (Still Local Network)

```
┌─────────────┐     HTTP      ┌──────────────┐
│  iPhone App  │ ◄──────────► │ Blue Daemon   │
│  (SwiftUI)   │  LAN/WiFi   │ (macbook:     │
│              │   :7865      │   7865)        │
└─────────────┘               └──────────────┘
```

**What changes**:

1. **Bind address**: Daemon needs to listen on `0.0.0.0:7865` instead of `127.0.0.1:7865` (or a configurable address)
2. **mDNS/Bonjour discovery**: The iPhone app discovers the daemon on the local network via Bonjour (`_blue._tcp`). No manual IP entry.
3. **Auth**: Even on LAN, some auth is needed. A shared secret or pairing code (like AirDrop) — show a code on the Mac, enter it on the iPhone. Generates a session token.
4. **Read-only first**: The iPhone app starts read-only — view the board, see notifications, read documents. Write operations (move cards, create docs) come later.

### Phase 3: Blue on EC2 (muffinlabs)

```
┌─────────────┐     HTTPS     ┌──────────────────────┐
│  macOS App   │ ◄───────────► │  EC2 (muffinlabs)    │
│  iPhone App  │    :443      │  ┌────────────────┐   │
│              │              │  │ Blue Daemon     │   │
└─────────────┘              │  │ (0.0.0.0:7865)  │   │
                              │  └────────┬───────┘   │
                              │           │            │
                              │  ┌────────▼───────┐   │
                              │  │ EBS Volume      │   │
                              │  │ /data/.blue/    │   │
                              │  │ projects/       │   │
                              │  └────────────────┘   │
                              │                        │
                              │  ┌────────────────┐   │
                              │  │ nginx/caddy     │   │
                              │  │ TLS termination │   │
                              │  │ + auth proxy    │   │
                              │  └────────────────┘   │
                              └──────────────────────┘
```

**What changes**:

1. **TLS**: Reverse proxy (nginx or Caddy) terminates TLS. Daemon stays HTTP internally. The app connects over HTTPS.
2. **Auth**: Token-based authentication. API keys or OAuth. The daemon validates tokens in middleware.
3. **Git sync**: Projects on EC2 need git access. The daemon clones/pulls repos, or projects are pushed to the EC2 instance. Blue's realm sync already handles multi-repo coordination — extend it.
4. **Latency**: HTTP is fine for kanban interactions. WebSocket keeps the board live without polling.
5. **Multi-user**: Multiple people could connect to the same Blue instance. Session tracking already exists. Add user identity to sessions.

**Daemon configuration** (needed for all phases):

```yaml
# ~/.blue/daemon.yaml or .blue/config.yaml
daemon:
  address: "0.0.0.0"          # default: 127.0.0.1
  port: 7865                   # default: 7865
  tls:
    enabled: false             # handled by reverse proxy in prod
  auth:
    enabled: false             # phase 1: off. phase 2+: on
    method: "token"            # token | oauth
    tokens:
      - name: "macbook"
        hash: "sha256:..."
      - name: "iphone"
        hash: "sha256:..."
  cors:
    allowed_origins: ["*"]     # restrict in prod
```

### Thin App / Fat Daemon

This maps directly to the thin-plugin/fat-binary strategy:

| Layer | What It Shows | Intelligence? |
|-------|-------------|--------------|
| **App UI** (SwiftUI) | Cards, columns, status badges, markdown | No — pure presentation |
| **Daemon API** (HTTP) | JSON responses with document data, status, tasks | Minimal — routing + serialization |
| **Blue Core** (Rust) | Document parsing, status transitions, validation, voice | Yes — all business logic |
| **Filesystem** (.blue/) | Markdown files, YAML config | Source of truth |

The app is a **thin visual client**. It doesn't parse markdown, validate status transitions, or enforce business rules. It sends `PATCH /rfcs/{slug} {"status": "final"}` and the daemon validates, transitions, and returns the result. Same principle as the plugin: the surface is dumb, the binary is smart.

### What the App Shows That the Terminal Can't

The kanban board solves real problems that Claude Code's terminal can't:

1. **Spatial overview** — see all RFCs, their statuses, and tasks at a glance. Terminal gives you one document at a time.
2. **Drag-and-drop state transitions** — drag a card from "Draft" to "Dialogue". Faster than `blue_rfc_update_status`.
3. **Persistent visibility** — the board is always open on your second monitor or phone. Claude Code sessions end.
4. **Notifications** — push notifications on iPhone when a spike time-box expires or an RFC is approved.
5. **Offline reading** — cache documents locally. Read RFCs on the train.
6. **Multi-project switching** — sidebar with all projects. Click to switch boards. No `cd` or MCP reconnection.

### Technology Choices

| Choice | Recommendation | Why |
|--------|---------------|-----|
| **App framework** | SwiftUI | Native Apple, shared macOS/iOS, great for kanban |
| **Networking** | URLSession + Combine/async-await | Built-in, no dependencies |
| **WebSocket** | URLSessionWebSocketTask | Native, no library needed |
| **Markdown rendering** | swift-markdown + AttributedString | Apple's own parser |
| **Local discovery** | NWBrowser (Network framework) | Bonjour/mDNS, native |
| **Persistence** | SwiftData or Core Data | Offline cache for documents |
| **Distribution** | TestFlight → App Store | Standard Apple path |

### Implementation Order

**Step 1 — Daemon API expansion** (Rust side):
- Add document CRUD endpoints to `blue-core/src/daemon/server.rs`
- Add project discovery endpoint
- Make bind address/port configurable
- Add WebSocket support for live updates

**Step 2 — macOS app (read-only)**:
- Connect to local daemon
- Display kanban board with documents as cards
- Render document detail with markdown
- Live updates via WebSocket

**Step 3 — macOS app (read-write)**:
- Drag-and-drop status transitions
- Create documents from the app
- Task management within RFC plans

**Step 4 — iPhone app**:
- Shared SwiftUI codebase, iPhone target
- Bonjour discovery for local daemon
- LAN pairing with auth token
- Push notifications via APNs

**Step 5 — EC2 deployment** (muffinlabs):
- Daemon config for remote hosting
- TLS via reverse proxy
- Token auth middleware
- Git sync for project access

## Findings

| Question | Answer |
|----------|--------|
| Can a native app talk to Blue today? | Partially. The daemon HTTP API exists but only exposes realms/sessions/notifications. Documents are MCP-only. |
| What's the biggest gap? | Document CRUD over HTTP. The daemon needs 15-20 new endpoints to expose what MCP already serves. |
| Can macOS and iOS share code? | Yes. SwiftUI compiles for both. One codebase, two targets. |
| How does local → EC2 work? | Same HTTP API, different address. Add TLS + auth when moving off localhost. Daemon config file controls the transition. |
| Does this fit thin-plugin/fat-binary? | Exactly. The app is another thin surface. All intelligence stays in the compiled Rust daemon. |
| What about the temp-file output idea? | The app is the better output surface. Instead of writing temp files, the daemon serves rich content to the app directly. Terminal gets the summary, app gets the full view. |

## Outcome

The kanban app is viable and architecturally clean. The daemon is the right integration point — it's already a long-running HTTP server. The work is:
1. **Expand the daemon API** with document endpoints and WebSocket (Rust)
2. **Build the SwiftUI app** with shared macOS/iOS codebase
3. **Add daemon configuration** for address/port/auth
4. **Deploy to EC2** when ready for remote access

The app doesn't add complexity to Blue's core — it's a presentation layer over the same data the MCP server already manages. The daemon just needs to expose it over HTTP the way it already exposes realms and sessions.
