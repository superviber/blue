# RFC 0068: PM Repo as Ground Truth

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-13 |
| **Depends On** | [RFC 0063](0063-jira-cloud-integration.draft.md), [RFC 0067](0067-org-directory-layout.draft.md) |
| **Dialogue** | [PM Repo Sync Architecture](../dialogues/2026-03-12T2026Z-pm-repo-sync-architecture.dialogue.recorded.md) |
| **ALIGNMENT** | 231 (3 rounds, 8 experts, 9/9 tensions resolved) |

---

## Summary

The PM repo is the single source of truth for an org's project artifacts. Every RFC must belong to a story, every story must belong to an epic. Epics use the org-wide key (e.g., `TMS`). Stories use the repo key where the work lives (e.g., `BKD-001` for backend). `blue rfc create` enforces this hierarchy at creation time. Blue sync generalizes via a `DocSource` trait to project PM artifacts to Jira.

## Problem

1. **RFC 0063 sync is repo-scoped.** It only scans `.blue/docs/rfcs/` in the current repo. Orgs with a dedicated PM repo have no sync path.
2. **RFCs are unlinked.** `blue rfc create` produces standalone documents with no connection to the project management hierarchy. Work starts without being planned.
3. **No repo-level work organization.** There is no mapping between repos and the type of work that belongs in each. Stories land in arbitrary locations with no convention for which repo owns what.

## Design

### Principle: Every RFC belongs to a story. Every story belongs to an epic.

This is the central constraint. All engineering work traces to planned work in the PM repo. `blue rfc create` enforces this at creation time.

### Key Hierarchy

```
Org (The Move Social)
  └── domain.yaml          # org key: TMS, repo registry with per-repo keys
       ├── repo: themove-backend    key: BKD  "Backend API services"
       ├── repo: themove-frontend   key: FRD  "React web application"
       └── repo: themove-product    key: PRD  "Product specs and docs"

Epics use the org key:     TMS-01 "Party System"
Stories use the repo key:  BKD-001 "Create Party API endpoint"
                           FRD-001 "Create Party UI component"
```

An epic can span multiple repos (cross-cutting work). A story always belongs to exactly one repo (the repo where the implementation lives).

### 1. domain.yaml (at PM repo root)

```yaml
org: the-move-social
key: TMS
domain: themovesocial.atlassian.net
project_key: SCRUM
drift_policy: warn

repos:
  - name: themove-backend
    key: BKD
    url: git@github.com:the-move-social/themove-backend.git
    description: "Backend API services — REST endpoints, auth, database"

  - name: themove-frontend
    key: FRD
    url: git@github.com:the-move-social/themove-frontend.git
    description: "React web application — UI components, routing, state"

  - name: themove-product
    key: PRD
    url: git@github.com:the-move-social/themove-product.git
    description: "Product specs, feature docs, user research"

  - name: project-management
    key: PM
    url: git@github.com:the-move-social/project-management.git
    description: "Project management — epics, stories, sprints, releases"
```

The `key` field is the story ID prefix for work in that repo. The `description` tells Blue (and humans) what type of work belongs there — Blue uses this when helping place new stories.

### 2. PM Repo Structure

```
project-management/
├── jira.toml                 # provider config, link types, status map
├── domain.yaml               # org key, repo registry (above)
├── epics/
│   ├── TMS-01-party-system/
│   │   ├── _epic.md          # type: epic, id: TMS-01
│   │   ├── BKD-001-create-party-api.md
│   │   ├── BKD-002-party-invites-api.md
│   │   └── FRD-001-create-party-ui.md
│   └── TMS-02-move-discovery/
│       ├── _epic.md
│       └── BKD-003-ai-move-generation.md
├── releases/
│   └── phase-0-mvp.yaml
├── sprints/
│   └── s01.md ... s09.md
└── .blue/
    └── blue.db
```

Auto-detected by `jira.toml` at repo root. Stories are filed under their epic directory but prefixed with their repo key.

### 3. Document Formats

#### jira.toml

```toml
provider = "jira-cloud"

[link_types]
depends_on = "Blocks"
relates_to = "Relates"

[status_map]
backlog = "To Do"
ready = "To Do"
in-progress = "In Progress"
in-review = "In Progress"
done = "Done"
blocked = "In Progress"
```

Domain and project_key moved to `domain.yaml` (single source).

#### Epic (`_epic.md`)

```yaml
---
type: epic
id: TMS-01
title: "Party System"
status: backlog
priority: 1
labels: [phase-0, core-social]
release: phase-0-mvp
jira_url: https://themovesocial.atlassian.net/browse/SCRUM-10
---
```

Epics use the org key (`TMS-01`, `TMS-02`, ...). Auto-incremented by Blue with Jira collision check — before assigning an ID, Blue queries the Jira project for existing issues to ensure the local sequence doesn't collide with issues created directly in Jira.

#### Story

```yaml
---
type: story
id: BKD-001
title: "Create Party API endpoint"
epic: TMS-01
repo: themove-backend
status: backlog
points: 3
sprint: s01
assignee: null
depends_on:
  - FRD-001
  - id: BKD-002
    link_type: relates-to
labels: [api, auth]
jira_url: https://themovesocial.atlassian.net/browse/SCRUM-42
---
```

Stories use the repo key (`BKD-001`). The `repo` field explicitly declares which repo this work belongs to. The `epic` field references the parent by org-key ID.

#### RFC (in code repo)

```
| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-13 |
| **Story** | BKD-001 |
| **Jira** | SCRUM-42 |
```

### 4. RFC Creation Flow (`blue rfc create`)

When `blue rfc create "Create Party API"` runs in `themove-backend`:

**Step 1 — Locate PM repo.** Resolve via RFC 0067 org layout. Read `domain.yaml`, find current repo's key (`BKD`) and description.

**Step 2 — Match existing story.** Search PM repo for stories with `repo: themove-backend` (or key prefix `BKD-*`). Present matches:

```
Stories for themove-backend (BKD):
  [1] BKD-001: Create Party API endpoint (backlog, epic: TMS-01 Party System)
  [2] BKD-002: Party Invites API (backlog, epic: TMS-01 Party System)
  [3] None of these — create a new story

Link to which story?
```

**Step 3a — Link to existing.** Write `| **Story** | BKD-001 |` into RFC. Done.

**Step 3b — Create new story.** Blue scaffolds interactively, using the repo key for the ID:

```
Creating story in PM repo for themove-backend (BKD)...

Title: Create Party API
Epic:
  [1] TMS-01: Party System
  [2] TMS-02: Move Discovery
  [3] Create new epic
Sprint: [1] s01  [2] s02  [3] unassigned
Points: [1] 1  [2] 2  [3] 3  [5] 5  [8] 8
Labels: (e.g. api,auth)

→ Created BKD-003 under TMS-01
```

Blue auto-increments the story ID within the repo key namespace (`BKD-003`), validating against Jira to avoid collisions with issues created outside Blue. Writes the story file to the PM repo under the epic directory, commits with `blue-pm: add BKD-003`, and links the RFC.

**Step 3c — Create new epic.** If no epic fits, Blue scaffolds one with the org key:

```
Creating epic in PM repo...

Title: Notifications System
Labels: (e.g. phase-1,infrastructure)

→ Created TMS-03: Notifications System
```

Blue auto-increments the epic ID within the org key namespace (`TMS-03`), validating against Jira to avoid collisions. Creates the directory and `_epic.md`, then proceeds with story creation.

### 5. DocSource Trait

```rust
trait DocSource {
    fn discover(&self) -> Result<Vec<SyncItem>, SyncError>;
    fn resolve_links(&self, mapping: &IdMap) -> Vec<LinkRequest>;
    fn writeback(&self, results: &SyncResults) -> Result<WritebackSet, SyncError>;
}
```

| DocSource | Discovery | Parsing | Writeback |
|-----------|-----------|---------|-----------|
| `RfcDocSource` | `.blue/docs/rfcs/*.md` | Markdown table | `jira_key` in table |
| `PmDocSource` | `epics/**/*.md` | YAML front matter | `jira_url` in YAML |

### 6. Sync Engine

```
discover() → push() → resolve_links() → push_links() → verify() → writeback()
```

Two-pass: create all issues (pass 1), then resolve `depends_on` links (pass 2). Mode-unaware — consumes DocSource uniformly.

### 7. Writeback

- Inline `jira_url` as last YAML field (machine-owned)
- Batch atomic commit after all API calls succeed (`blue-sync: writeback`)
- `.gitattributes` `ours` merge strategy on `jira_url:` lines
- Idempotent on re-run

### 8. First-Sync Safety

1. No `jira_url` in any file → `blue sync` defaults to `--dry-run` with preview
2. `blue sync --confirm` → executes
3. Any `jira_url` exists → subsequent syncs implicit

### 9. DriftDetector

Separate post-sync component. Not bidirectional sync.

- **Scoped read-back**: `status` and `sprint` Jira-authoritative (patchable via `drift_policy`)
- **All other fields**: git-authoritative, drift = warning only
- **Modes**: `--verify` (print), `--check` (CI gate), `--check-drift` (standalone)

### 10. depends_on & Links

- Single `depends_on` field, default `Blocks`
- Per-edge `link_type` override for non-blocking relationships
- DAG cycle detection on `Blocks` edges only
- Sprint ordering validation: story cannot precede its blocking dependency
- Cross-repo dependencies work naturally (`BKD-001` depends on `FRD-001`)

### 11. Security

- **Domain pinning**: `.sync/domain-pin.sha256` (tracked), mandatory pre-sync gate
- **No fourth credential tier**: `.env.local` for non-secret config only
- **Lint**: `blue lint` scans PM repo for credential patterns and `.gitignore` integrity

## Implementation Plan

### Phase 1: domain.yaml & Repo Registry

- `domain.yaml` schema: org key, repo list with keys + descriptions
- PM repo locator from any code repo via RFC 0067 org layout
- `blue init` writes repo entry into `domain.yaml` if missing
- Story ID auto-increment per repo key namespace with Jira collision check
- Epic ID auto-increment per org key namespace with Jira collision check

### Phase 2: RFC-Story Linking

- `blue rfc create` requires story link (interactive or `--story`)
- Story matching by repo key filter + title similarity
- Interactive story scaffolding (epic, sprint, points, labels)
- Interactive epic scaffolding with org key
- Write `| **Story** | {id} |` to RFC front matter
- Commit to PM repo with `blue-pm:` prefix

### Phase 3: DocSource Trait & PmDocSource

- Extract `DocSource` trait from existing RFC sync
- `RfcDocSource` wrapping current behavior
- `PmDocSource` with YAML front matter parsing
- Auto-detection via `jira.toml`

### Phase 4: Two-Pass Sync & Links

- Sync engine consumes DocSource
- Two-pass orchestration (create → link)
- `depends_on` parsing + DAG validation

### Phase 5: Writeback & Safety

- Inline `jira_url`, batch atomic commit
- `.gitattributes` merge driver
- First-sync safety gate

### Phase 6: DriftDetector

- `DriftReport` with severity levels
- Scoped read-back for status + sprint
- `--verify`, `--check`, `--check-drift`

### Phase 7: Security

- jira.toml domain pinning
- Pre-flight credential leak check
- Lint extension

## Future Work

- `blue status <id> <status>` — CLI mutation shorthand
- `blue board` — read-only TUI board view
- `blue story create` — standalone story scaffolding (outside RFC flow)
- Lock-ref delegation for restricted-access orgs
- Non-git contributor workflows
- Sprint/release YAML sync to Jira sprints/versions

## Test Plan

- [ ] `domain.yaml` parsed: org key, repo keys, descriptions
- [ ] `blue rfc create` in code repo resolves PM repo and current repo key
- [ ] `blue rfc create` refuses without story link
- [ ] `blue rfc create --story BKD-001` links to existing story
- [ ] Story matching filters by repo key (`BKD-*` in `themove-backend`)
- [ ] Story auto-increment: `BKD-003` follows `BKD-002`
- [ ] Epic auto-increment: `TMS-03` follows `TMS-02`
- [ ] Interactive story creation writes valid YAML under correct epic dir
- [ ] Interactive epic creation scaffolds directory + `_epic.md` with org key
- [ ] Cross-repo depends_on: `BKD-001` depends on `FRD-001` resolves correctly
- [ ] DocSource: RfcDocSource produces same SyncItems as current sync
- [ ] PmDocSource discovers epics and stories from PM structure
- [ ] Two-pass sync: forward references resolved
- [ ] DAG validation: Blocks cycles flagged, relates-to ignored
- [ ] Writeback: jira_url injected, idempotent, batch atomic
- [ ] First-sync safety: dry-run → confirm → implicit
- [ ] DriftDetector: status/sprint divergence, typed report
- [ ] Domain pinning: verified, hard-fail on mismatch

---

*"Right then. Let's get to it."*

— Blue
