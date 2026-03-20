# RFC 0056: Alignment Visualization Dashboard

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-02 |
| **ADRs** | 0014 (Alignment Dialogue Agents), 0018 (DynamoDB-Portable Schema) |
| **Depends On** | RFC 0051 (Global Perspective & Tension Tracking), RFC 0053 (Storage Abstraction Layer) |

---

## Summary

Build a Next.js web application to visualize alignment dialogues with three core capabilities:

1. **Real-time Monitoring** — Watch dialogues in progress via WebSocket
2. **Post-Dialogue Analysis** — Deep-dive into completed dialogues
3. **Cross-Dialogue Analytics** — Discover patterns across many dialogues

**Key design principle:** Storage-agnostic architecture that works identically with SQLite (local) or DynamoDB (AWS production).

## Problem

The alignment dialogue system generates rich structured data:
- Expert contributions and scores
- Perspectives, tensions, recommendations, evidence, claims
- Cross-references between entities
- Velocity and convergence metrics
- Verdicts and dissents

Currently this data is only accessible via:
- Raw SQLite queries
- MCP tool calls
- JSON exports

There's no visual way to:
- Monitor a dialogue in real-time during execution
- Explore the relationship graph between entities
- Compare dialogues or track patterns over time
- Share dialogue results with stakeholders

## Design

### Architecture

**Local Development:**
```
┌─────────────────────────────────────────────────────────────┐
│                    Next.js Dashboard                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Live      │  │   Analysis  │  │   Analytics         │  │
│  │   Monitor   │  │   Explorer  │  │   Dashboard         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
            │                               │
     WebSocket (ws://)              REST API (http://)
            │                               │
            ▼                               ▼
┌─────────────────────────────────────────────────────────────┐
│                 Next.js API Routes                           │
│  WS  /api/ws/dialogues/:id        GET /api/dialogues/:id    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              DialogueStore (RFC 0053)                        │
│              ┌─────────────────────┐                         │
│              │  SqliteDialogueStore │                        │
│              │   (better-sqlite3)   │                        │
│              └─────────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

**AWS Production:**
```
┌─────────────────────────────────────────────────────────────┐
│                    Next.js Dashboard                         │
│                   (Vercel / Amplify)                         │
└─────────────────────────────────────────────────────────────┘
            │                               │
     WebSocket (wss://)             REST API (https://)
            │                               │
            ▼                               ▼
┌─────────────────┐             ┌─────────────────────────────┐
│  API Gateway    │             │      Lambda Functions       │
│  WebSocket API  │             │   GET /dialogues/:id        │
│  $connect       │             │   GET /stats                │
│  $disconnect    │             └─────────────────────────────┘
│  subscribe      │                         │
└─────────────────┘                         │
            │                               │
            └───────────────┬───────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              DialogueStore (RFC 0053)                        │
│              ┌─────────────────────┐                         │
│              │ DynamoDialogueStore │                         │
│              │  (encrypted, KMS)   │                         │
│              └─────────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
```

**Key Principle:** The `DialogueStore` interface (RFC 0053) is identical in both environments. The dashboard code doesn't know or care which backend is active.

### Tech Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| Framework | Next.js 14+ (App Router) | SSR, API routes, React Server Components |
| Styling | Tailwind CSS | Rapid iteration, consistent design |
| Charts | Recharts or Victory | React-native charting |
| Graph | React Flow or D3 | Interactive node-edge visualization |
| State | Zustand or React Query | Lightweight, SSR-friendly |
| Real-time | Server-Sent Events or WebSocket | Live updates |

### Core Views

#### 1. Live Monitor (`/live/:dialogueId`)

Real-time view during active dialogue execution.

**Components:**
- **Velocity Chart** — Line chart showing ALIGNMENT score per round
- **Convergence Indicator** — Visual signal when velocity approaches zero
- **Expert Leaderboard** — Live-updating score table
- **Tension Tracker** — Open/addressed/resolved counts with progress bar
- **Activity Feed** — Stream of new perspectives, tensions, verdicts

**Data Source:** `get_dialogue_progress()` polled or streamed

**Wireframe:**
```
┌────────────────────────────────────────────────────────────┐
│  📊 Live: Investment Portfolio Analysis          [Round 3] │
├────────────────────────────────────────────────────────────┤
│  ┌──────────────────────┐  ┌────────────────────────────┐  │
│  │ ALIGNMENT Velocity   │  │ Expert Leaderboard         │  │
│  │   ▲                  │  │ 1. 🧁 Donut      33 pts    │  │
│  │   │    ●             │  │ 2. 🧁 Muffin    22 pts    │  │
│  │   │  ●   ●           │  │ 3. 🧁 Palmier   12 pts    │  │
│  │   │●                 │  │ 4. 🧁 Cupcake   13 pts    │  │
│  │   └──────────▶       │  └────────────────────────────┘  │
│  │   R0  R1  R2  R3     │                                  │
│  └──────────────────────┘  ┌────────────────────────────┐  │
│                            │ Tensions                    │  │
│  ┌──────────────────────┐  │ ████████░░ 4/5 resolved    │  │
│  │ 🟢 Converging        │  └────────────────────────────┘  │
│  │ Velocity: +2         │                                  │
│  │ Est. 1 round left    │                                  │
│  └──────────────────────┘                                  │
├────────────────────────────────────────────────────────────┤
│  Activity Feed                                             │
│  • [R3] Muffin: CONVERGE on R0001 options overlay         │
│  • [R3] Donut: RESOLVE T0001 via C0101                    │
│  • [R2] Palmier: NEW P0101 concentration risk             │
└────────────────────────────────────────────────────────────┘
```

#### 2. Analysis Explorer (`/dialogue/:dialogueId`)

Post-hoc exploration of a completed dialogue.

**Components:**
- **Summary Card** — Title, question, final verdict, total ALIGNMENT
- **Entity Graph** — Interactive visualization of P/R/T/E/C relationships
- **Round Timeline** — Expandable accordion with round details
- **Expert Profiles** — Per-expert contribution breakdown
- **Verdict Panel** — Final, minority, dissent verdicts

**Entity Graph:**
```
        ┌─────────┐
        │  P0001  │◄──── support ────┐
        │ Income  │                  │
        └────┬────┘                  │
             │                   ┌───┴────┐
         support                 │ E0101  │
             │                   │Premium │
             ▼                   └────────┘
        ┌─────────┐                  │
        │  T0001  │                depend
        │ Conflict│                  │
        └────┬────┘                  ▼
             │                   ┌────────┐
         address                 │ C0101  │
             │                   │Resolved│
             ▼                   └───┬────┘
        ┌─────────┐                  │
        │  R0001  │◄──── resolve ────┘
        │ Options │
        └─────────┘
```

**Data Source:**
- `get_dialogue()`, `get_perspectives()`, `get_tensions()`, etc.
- `expand_citation()` for hover tooltips

#### 3. Analytics Dashboard (`/analytics`)

Cross-dialogue patterns and trends.

**Components:**
- **Stats Overview** — Total dialogues, perspectives, tensions, avg ALIGNMENT
- **Top Experts** — Leaderboard across all dialogues
- **Tension Patterns** — Common tension labels/themes
- **Dialogue Comparison** — Side-by-side metrics
- **Search** — Find dialogues by topic

**Data Source:** `get_cross_dialogue_stats()`, `find_similar_dialogues()`

**Wireframe:**
```
┌────────────────────────────────────────────────────────────┐
│  📈 Analytics Dashboard                                    │
├────────────────────────────────────────────────────────────┤
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐              │
│  │   12   │ │  156   │ │   34   │ │   78   │              │
│  │Dialogues│ │Perspect│ │Tensions│ │Avg ALIGN│             │
│  └────────┘ └────────┘ └────────┘ └────────┘              │
├────────────────────────────────────────────────────────────┤
│  Top Experts (All Time)        │  Recent Dialogues         │
│  1. muffin    456 pts (12 dlg) │  • Investment Analysis ✓  │
│  2. donut     389 pts (11 dlg) │  • API Design ✓           │
│  3. cupcake   312 pts (10 dlg) │  • Risk Assessment 🔄     │
│  4. brioche   287 pts (9 dlg)  │  • Product Strategy ✓     │
├────────────────────────────────────────────────────────────┤
│  [Search dialogues...]                                     │
│  Related: "investment" → 3 matches                        │
└────────────────────────────────────────────────────────────┘
```

### API Routes

| Route | Method | Description | Data Source |
|-------|--------|-------------|-------------|
| `/api/dialogues` | GET | List all dialogues | `get_dialogues()` |
| `/api/dialogues/:id` | GET | Full dialogue with entities | `get_dialogue()` + entities |
| `/api/dialogues/:id/progress` | GET | Real-time progress | `get_dialogue_progress()` |
| `/api/dialogues/:id/graph` | GET | Entity relationship graph | entities + refs |
| `/api/dialogues/:id/stream` | SSE | Live updates | poll `get_dialogue_progress()` |
| `/api/stats` | GET | Cross-dialogue stats | `get_cross_dialogue_stats()` |
| `/api/search` | GET | Find similar dialogues | `find_similar_dialogues()` |
| `/api/citation/:id` | GET | Expand citation | `expand_citation()` |

### Database Access

Dual-mode storage matching RFC 0053 (Storage Abstraction Layer):

**Local Development: SQLite**
- Next.js API routes use `better-sqlite3`
- Read-only access to Blue's SQLite database
- Simple, fast, zero infrastructure

**Production: DynamoDB**
- Uses RFC 0053 storage abstraction
- Encrypted at rest (AWS KMS)
- Same query interface, different backend
- Authentication required

```typescript
// lib/db.ts
import { createStore } from '@blue/storage';

const store = createStore({
  backend: process.env.NODE_ENV === 'production' ? 'dynamodb' : 'sqlite',
  sqlite: { path: process.env.BLUE_DB_PATH },
  dynamodb: { table: process.env.DYNAMODB_TABLE, region: 'us-east-1' }
});

export const getDialogue = (id: string) => store.dialogue.get(id);
export const getProgress = (id: string) => store.dialogue.progress(id);
// ... etc
```

### Real-Time Updates (WebSocket)

WebSocket for live monitoring, with environment-aware implementation:

**Local: Next.js WebSocket (via socket.io or ws)**
```typescript
// pages/api/ws/dialogues/[id].ts (using next-ws or similar)
import { WebSocketServer } from 'ws';
import { store } from '@/lib/db';

export default function handler(ws: WebSocket, req: Request) {
  const dialogueId = req.params.id;

  // Subscribe to dialogue updates
  const interval = setInterval(async () => {
    const progress = await store.dialogue.progress(dialogueId);
    ws.send(JSON.stringify({ type: 'progress', data: progress }));
  }, 1000);

  ws.on('close', () => clearInterval(interval));
}
```

**Production: API Gateway WebSocket**
```typescript
// Lambda handler for API Gateway WebSocket
export const handler = async (event: APIGatewayWebSocketEvent) => {
  const { routeKey, connectionId, body } = event;

  switch (routeKey) {
    case '$connect':
      // Store connectionId in DynamoDB connections table
      await store.connections.add(connectionId);
      break;

    case 'subscribe':
      const { dialogueId } = JSON.parse(body);
      await store.subscriptions.add(connectionId, dialogueId);
      break;

    case '$disconnect':
      await store.connections.remove(connectionId);
      break;
  }

  return { statusCode: 200 };
};

// Separate Lambda triggered by DynamoDB Streams or EventBridge
export const broadcastProgress = async (dialogueId: string) => {
  const subscribers = await store.subscriptions.forDialogue(dialogueId);
  const progress = await store.dialogue.progress(dialogueId);

  for (const connectionId of subscribers) {
    await apiGateway.postToConnection({
      ConnectionId: connectionId,
      Data: JSON.stringify({ type: 'progress', data: progress })
    });
  }
};
```

**Client Hook:**
```typescript
// hooks/useDialogueProgress.ts
export function useDialogueProgress(dialogueId: string) {
  const [progress, setProgress] = useState<DialogueProgress | null>(null);

  useEffect(() => {
    const wsUrl = process.env.NEXT_PUBLIC_WS_URL || 'ws://localhost:3000/api/ws';
    const ws = new WebSocket(`${wsUrl}/dialogues/${dialogueId}`);

    ws.onmessage = (event) => {
      const { type, data } = JSON.parse(event.data);
      if (type === 'progress') setProgress(data);
    };

    return () => ws.close();
  }, [dialogueId]);

  return progress;
}
```

### Entity Graph Visualization

Using React Flow for interactive graphs:

```typescript
const entityToNode = (entity: Entity): Node => ({
  id: entity.display_id,
  type: entity.type, // perspective | tension | recommendation | evidence | claim
  data: {
    label: entity.label,
    contributors: entity.contributors,
    status: entity.status
  },
  position: calculatePosition(entity), // Force-directed or hierarchical
});

const refToEdge = (ref: Ref): Edge => ({
  id: `${ref.source_id}-${ref.target_id}`,
  source: ref.source_id,
  target: ref.target_id,
  label: ref.ref_type, // support, oppose, resolve, etc.
  animated: ref.ref_type === 'resolve',
  style: getEdgeStyle(ref.ref_type),
});
```

## Implementation Plan

### Phase 1: Foundation
- [ ] Initialize Next.js project with Tailwind
- [ ] Set up SQLite connection (read-only)
- [ ] Implement core API routes (`/dialogues`, `/stats`)
- [ ] Create basic layout and navigation

### Phase 2: Analytics Dashboard
- [ ] Stats overview cards
- [ ] Top experts leaderboard
- [ ] Dialogue list with search
- [ ] Basic filtering

### Phase 3: Dialogue Explorer
- [ ] Dialogue detail page
- [ ] Round timeline accordion
- [ ] Expert profiles
- [ ] Verdict display

### Phase 4: Entity Graph
- [ ] React Flow integration
- [ ] Entity nodes by type
- [ ] Reference edges with labels
- [ ] Hover tooltips via `expand_citation()`
- [ ] Click to expand/focus

### Phase 5: Live Monitor
- [ ] WebSocket endpoint for progress
- [ ] Velocity chart (Recharts)
- [ ] Live leaderboard
- [ ] Convergence indicator
- [ ] Activity feed

### Phase 6: Polish
- [ ] Responsive design
- [ ] Dark mode
- [ ] Export to PNG/PDF
- [ ] Shareable links

### Phase 7: AWS Deployment
- [ ] Implement `DynamoDialogueStore` (RFC 0053)
- [ ] API Gateway WebSocket API for real-time
- [ ] Lambda functions for REST endpoints
- [ ] CloudFront distribution
- [ ] Cognito authentication
- [ ] KMS encryption for DynamoDB
- [ ] CDK/Terraform infrastructure as code

## File Structure

```
blue-viz/
├── app/
│   ├── layout.tsx
│   ├── page.tsx                    # Home → redirect to /analytics
│   ├── analytics/
│   │   └── page.tsx                # Cross-dialogue dashboard
│   ├── dialogue/
│   │   └── [id]/
│   │       └── page.tsx            # Post-hoc explorer
│   ├── live/
│   │   └── [id]/
│   │       └── page.tsx            # Real-time monitor
│   └── api/
│       ├── dialogues/
│       │   ├── route.ts            # GET list
│       │   └── [id]/
│       │       ├── route.ts        # GET detail
│       │       ├── progress/route.ts
│       │       └── graph/route.ts
│       ├── ws/
│       │   └── dialogues/[id].ts   # WebSocket endpoint
│       ├── stats/route.ts
│       ├── search/route.ts
│       └── citation/[id]/route.ts
├── components/
│   ├── VelocityChart.tsx
│   ├── Leaderboard.tsx
│   ├── TensionTracker.tsx
│   ├── EntityGraph.tsx
│   ├── RoundTimeline.tsx
│   ├── VerdictPanel.tsx
│   └── CitationTooltip.tsx
├── hooks/
│   ├── useDialogueProgress.ts      # WebSocket hook
│   └── useDialogue.ts              # Data fetching
├── lib/
│   ├── store.ts                    # Storage abstraction (RFC 0053)
│   └── types.ts                    # Shared types
└── package.json
```

## AWS Infrastructure (Production)

```
┌─────────────────────────────────────────────────────────────────────┐
│                         CloudFront                                   │
│                     (CDN + HTTPS termination)                        │
└─────────────────────────────────────────────────────────────────────┘
                    │                           │
            Static Assets                 API Requests
                    │                           │
                    ▼                           ▼
┌─────────────────────────┐     ┌─────────────────────────────────────┐
│      S3 Bucket          │     │         API Gateway                 │
│   (Next.js static)      │     │  ┌─────────────┬─────────────────┐  │
└─────────────────────────┘     │  │ REST API    │ WebSocket API   │  │
                                │  │ /dialogues  │ $connect        │  │
                                │  │ /stats      │ subscribe       │  │
                                │  │ /search     │ $disconnect     │  │
                                │  └─────────────┴─────────────────┘  │
                                └─────────────────────────────────────┘
                                                │
                                                ▼
                                ┌─────────────────────────────────────┐
                                │         Lambda Functions            │
                                │  - dialogue-get                     │
                                │  - dialogue-list                    │
                                │  - progress-get                     │
                                │  - ws-connect                       │
                                │  - ws-subscribe                     │
                                │  - ws-broadcast                     │
                                └─────────────────────────────────────┘
                                                │
                                                ▼
┌─────────────────────────┐     ┌─────────────────────────────────────┐
│       Cognito           │     │          DynamoDB                   │
│   (Authentication)      │     │  - blue_dialogues (main table)      │
│   - User Pool           │     │  - blue_connections (WebSocket)     │
│   - Identity Pool       │     │  - Encrypted with KMS               │
└─────────────────────────┘     └─────────────────────────────────────┘
```

**DynamoDB Table Design (Single-Table):**

| PK | SK | Attributes |
|----|-----|------------|
| `DLG#investment-analysis` | `META` | title, status, total_alignment, ... |
| `DLG#investment-analysis` | `EXPERT#muffin` | role, tier, scores, ... |
| `DLG#investment-analysis` | `ROUND#00` | score, summary, ... |
| `DLG#investment-analysis` | `P#0001` | label, content, contributors, ... |
| `DLG#investment-analysis` | `T#0001` | label, status, ... |
| `DLG#investment-analysis` | `REF#P0001#T0001` | ref_type, ... |
| `WS#abc123` | `SUB#investment-analysis` | connectionId, subscribedAt |

**GSI for queries:**
- `GSI1`: `status` → list dialogues by status
- `GSI2`: `expert_slug` → find all dialogues for an expert

## Decisions

1. **Deployment**: Local for development/testing, hosted for production
   - Local: SQLite directly
   - Hosted: DynamoDB with encryption (see RFC 0053 Storage Abstraction)
2. **Authentication**: None locally; required for hosted (TBD)
3. **Write Operations**: Read-only for v1
4. **Embedding**: Deferred — consider later for sharing widgets in Notion, Slack, etc.

## Success Criteria

- [ ] Can monitor an active dialogue in real-time
- [ ] Can explore entity relationships visually
- [ ] Can compare multiple dialogues side-by-side
- [ ] Loads in under 2 seconds
- [ ] Works on mobile (responsive)

---

*"The elephant becomes visible — now let's draw it."*
