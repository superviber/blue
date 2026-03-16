# RFC 0070: Components & Areas — Two-Tier PM Organization

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-15 |
| **Story** | — |
| **Supersedes** | RFC 0069 |
| **Extends** | RFC 0068 |

---

## Summary

Replace the single-tier key model (RFC 0068: repo keys, RFC 0069: domain keys) with a two-tier model: **components** for functional ownership and **areas** for product/work-area scoping. Components map to Jira components and answer "who owns this?" Areas provide story key prefixes and answer "what part of the product is this?"

## Problem

RFC 0068 keyed stories to repos (`BKD-001`). RFC 0069 proposed keying stories to functional domains (`ENG-001`). Both are one-dimensional — they force a single axis of organization when real work has two:

1. **Functional ownership** — Is this engineering, design, product, or legal work? Who's responsible? What team reviews it?
2. **Product surface** — Is this the consumer app, the merchant dashboard, the landing page, or shared infrastructure?

A consumer app feature might need engineering, design, and product work. An engineering story might touch the consumer app, the merchant app, and shared infra. One axis isn't enough.

**What's already happening organically:** The Move Social's PM repo already uses area-style keys — `PTY` for party system, `MRC` for merchant dashboard, `DSC` for discovery. But these keys are welded to epics: `PTY` is both the epic and the key namespace. This means:
- You can't have two epics in the same area (e.g., "Party v2" needs a new key)
- Areas disappear when epics complete
- Cross-area work is awkward

## Design

### Two-Tier Model

```
Component (Jira Component)     Area (Story Key Prefix)
─────────────────────────      ─────────────────────────
"Who owns this work?"          "What product surface?"
Engineering, Product,          Consumer App (CON),
Design, Finance, Legal...      Merchant App (MER),
                               Landing Page (LND),
↓                              Backend API (API)...
Jira component field           ↓
Default assignees              Story ID prefix
Component-level reports        CON-001, MER-003, API-012
Board filters
```

**Components** are functional domains — the teams or roles responsible for the work. They map 1:1 to Jira components, which gives you default assignees, component-level burndowns, and admin-controlled categorization.

**Areas** are product surfaces, services, or workstreams — the thing being built. They provide the story key prefix (`CON-001`) and persist across epics. An area like "Consumer App" spans dozens of epics over the product's lifetime.

### Key Properties

| Property | Component | Area |
|----------|-----------|------|
| Maps to | Jira component | Story key prefix |
| Granularity | Broad (functional team) | Specific (product surface) |
| Cardinality | ~10 standard | Org-specific, grows with product |
| Lifespan | Permanent | Long-lived (outlives epics) |
| Cross-cutting | A story can have multiple components | A story has one area |
| Example | Engineering | Consumer App (CON) |

### How They Compose

```
Epic: TMS-03 "Party System v2"
├── CON-015  [Engineering]              "Party creation API v2"
├── CON-016  [Engineering]              "Party invite deeplinks"
├── CON-017  [Design]                   "Party creation flow redesign"
├── CON-018  [Product]                  "Party v2 user stories"
├── CON-019  [Engineering, Security]    "Party auth token hardening"
└── API-008  [Engineering]              "Rate limiting for party endpoints"

Epic: TMS-04 "Merchant Analytics"
├── MER-009  [Engineering]              "Analytics aggregation pipeline"
├── MER-010  [Design]                   "Dashboard chart components"
├── MER-011  [Product, Finance]         "Merchant analytics requirements"
└── MER-012  [Finance]                  "Revenue attribution model"
```

Filtering in Jira:
- `component = Engineering` → all engineering work across all areas
- `component in (Engineering, Security)` → all eng + security work
- `summary ~ "CON-*"` → all consumer app work across all components
- `component = Engineering AND summary ~ "CON-*"` → engineering work on consumer app

### Standard Components

Blue provides 10 standard components (matching RFC 0069's domains). Orgs can add more.

| Component | Covers |
|-----------|--------|
| Engineering | Backend, frontend, infrastructure, DevOps, CI/CD |
| Product | Feature specs, roadmap, user research, requirements |
| Design | UI/UX, visual identity, brand assets, design system |
| Business | Strategy, partnerships, monetization, go-to-market, sales |
| Finance | Budgets, runway, pricing models, projections, accounting |
| Operations | Processes, workflows, documentation, team coordination |
| Legal | Contracts, compliance, privacy, IP, regulatory |
| Growth | Acquisition, retention, A/B testing, content, marketing |
| People | Hiring, culture, onboarding, contractors, performance |
| Security | App security, infra security, audits, pen testing |

Components are **not keyed** — they don't appear in story IDs. They exist as Jira components and in domain.yaml for routing and ownership.

### Areas Are Org-Specific

Unlike components (which are standard), areas are defined per org based on what they're building:

**The Move Social example:**

| Key | Area | Description | Primary Components |
|-----|------|-------------|-------------------|
| CON | Consumer App | Consumer-facing mobile/web experience | Engineering, Product, Design |
| MER | Merchant App | Merchant dashboard and analytics tools | Engineering, Product, Design, Finance |
| LND | Landing Page | Marketing site and onboarding funnel | Engineering, Design, Growth |
| API | Backend API | Core shared services and platform APIs | Engineering, Security |
| INF | Infrastructure | CI/CD, cloud, monitoring, deployment | Engineering, Security |
| BRD | Brand | Visual identity, marketing assets | Design, Growth |
| BIZ | Business Ops | Strategy docs, partnership materials | Business, Finance, Legal |

**Key constraints:**
- 2-4 uppercase letters
- Unique within org
- Descriptive of product surface, not feature or epic

### What Happens to Epic-Scoped Keys?

The existing PM repo keys (`PTY`, `DSC`, `MRC`, etc.) are feature-scoped — they describe a feature, not a product surface. Under this model:

```
Old: PTY-001 "Create Party API"          → Party System epic, party key
New: CON-001 "Create Party API"          → Consumer App area, any epic
     Component: Engineering

Old: MRC-001 "Merchant Onboarding"       → Merchant Dashboard epic, merchant key
New: MER-001 "Merchant Onboarding"       → Merchant App area, any epic
     Component: Engineering

Old: DSC-001 "AI Move Suggestions"       → Move Discovery epic, discovery key
New: CON-005 "AI Move Suggestions"       → Consumer App area (it's a consumer feature)
     Component: Engineering
```

The area key is durable. When "Party System v2" comes along, its stories are still `CON-0xx` — because they're consumer app work. The epic provides the initiative context, the area provides the product context.

## domain.yaml Schema

```yaml
org: the-move-social
key: TMS                              # org-wide key for epics

jira:
  domain: themovesocial.atlassian.net
  project_key: SCRUM
  drift_policy: warn

# Functional ownership → Jira components
# Blue creates these as Jira components during setup
components:
  - name: Engineering
    description: "Backend, frontend, infrastructure, DevOps, CI/CD"
    lead: null                        # Jira component lead / default assignee
  - name: Product
    description: "Feature specs, roadmap, user research, requirements"
  - name: Design
    description: "UI/UX, visual identity, brand assets, design system"
  - name: Business
    description: "Strategy, partnerships, monetization, go-to-market, sales"
  - name: Finance
    description: "Budgets, runway, pricing models, projections, accounting"
  - name: Operations
    description: "Processes, workflows, documentation, team coordination"
  - name: Legal
    description: "Contracts, compliance, privacy, IP, regulatory"
  - name: Growth
    description: "Acquisition, retention, A/B testing, content, marketing"
  - name: People
    description: "Hiring, culture, onboarding, contractors"
  - name: Security
    description: "App security, infra security, audits, pen testing"

# Product surfaces / workstreams → story key prefixes
areas:
  - key: CON
    name: Consumer App
    description: "Consumer-facing mobile and web experience"
    components: [Engineering, Product, Design]     # typical components for this area
    repos: [themove-backend, themove-frontend]      # code repos this area touches
  - key: MER
    name: Merchant App
    description: "Merchant dashboard, analytics, and self-serve tools"
    components: [Engineering, Product, Design, Finance]
    repos: [themove-backend, merchant-frontend]
  - key: LND
    name: Landing Page
    description: "Marketing site, onboarding funnel, public pages"
    components: [Engineering, Design, Growth]
    repos: [themove-frontend]
  - key: API
    name: Backend API
    description: "Core shared services — auth, notifications, data pipeline"
    components: [Engineering, Security]
    repos: [themove-backend]
  - key: INF
    name: Infrastructure
    description: "CI/CD, cloud resources, monitoring, deployment"
    components: [Engineering, Security]
    repos: [infra]
  - key: BRD
    name: Brand
    description: "Visual identity, style guide, marketing assets"
    components: [Design, Growth]
    repos: []
  - key: BIZ
    name: Business Ops
    description: "Strategy docs, partnership materials, financial models"
    components: [Business, Finance, Legal]
    repos: []

# Code repositories (for linking RFCs and locating PM repo)
repos:
  - name: themove-backend
    url: git@github.com:the-move-social/themove-backend.git
    description: "Backend API services — REST endpoints, auth, database"
  - name: themove-frontend
    url: git@github.com:the-move-social/themove-frontend.git
    description: "React web application — UI components, routing, state"
  - name: merchant-frontend
    url: git@github.com:the-move-social/merchant-frontend.git
    description: "Merchant dashboard SPA"
  - name: infra
    url: git@github.com:the-move-social/infra.git
    description: "Terraform, CI/CD, monitoring"
  - name: project-management
    url: git@github.com:the-move-social/project-management.git
    description: "PM repo — epics, stories, sprints, releases"
```

### Key Schema Changes from RFC 0068/0069

| RFC 0068 | RFC 0069 | RFC 0070 |
|----------|----------|----------|
| `repos[].key` → story prefix | `domains[].key` → story prefix | `areas[].key` → story prefix |
| Repos are primary unit | Domains are primary unit | Areas are primary unit for keys, components for ownership |
| No Jira component mapping | Labels for domain | Jira components for functional ownership |
| — | `repos[].domain` links to domain | `areas[].repos` links to code repos |
| — | — | `areas[].components` declares typical ownership |

## Story Front Matter

```yaml
---
type: story
id: CON-015
title: "Party creation API v2"
area: CON                             # product surface (key prefix source)
components: [Engineering]              # functional ownership (Jira components, multi-select)
epic: TMS-03
repo: themove-backend                 # code repo (optional, for eng/design work)
status: backlog
points: 3
sprint: s01
depends_on:
  - CON-018                           # product spec must come first
labels: [api, party, v2]
jira_url: null
---
```

New fields vs RFC 0068:
- `area` — replaces implicit key-prefix parsing; explicit product surface
- `component` — Jira component for functional ownership

The `repo` field remains optional — only meaningful for work that lives in a code repo.

## Epic Structure

Epics keep the org key (`TMS-01`, `TMS-02`) and can contain stories from any area:

```
epics/
  TMS-01-party-system/
    _epic.md
    CON-001-create-party-api.md         # Consumer App, Engineering
    CON-002-mutual-connections.md       # Consumer App, Engineering
    CON-003-party-creation-flow.md      # Consumer App, Design
    CON-004-party-user-stories.md       # Consumer App, Product
    API-001-rate-limiting.md            # Backend API, Engineering
  TMS-02-move-discovery/
    _epic.md
    CON-005-ai-suggestions.md           # Consumer App, Engineering
    CON-006-discovery-ui.md             # Consumer App, Design
  TMS-03-merchant-dashboard/
    _epic.md
    MER-001-merchant-onboarding.md      # Merchant App, Engineering
    MER-002-analytics-dashboard.md      # Merchant App, Engineering
    MER-003-dashboard-charts.md         # Merchant App, Design
    MER-004-revenue-attribution.md      # Merchant App, Finance
```

## Auto-Categorization in `blue rfc create`

When `blue rfc create "Party Invite Deeplinks"` runs in `themove-backend`:

**Step 1 — Resolve area from context:**
- Current repo is `themove-backend` → areas that touch this repo: `CON`, `MER`, `API`
- Title keywords: "party" + "invite" → likely consumer-facing
- Suggest `CON`

```
Area for this work:
  [1] CON - Consumer App (Recommended — party + invite context)
  [2] API - Backend API
  [3] MER - Merchant App
  [4] Other area...
  [5] Help me decide
```

**Step 2 — Resolve component:**
- Current repo is code → likely Engineering
- Could also be Security if keywords match

```
Component:
  [1] Engineering (Recommended — you're in a code repo)
  [2] Security
  [3] Other...
```

**Step 3 — Link to epic and create story:**

```
→ Created CON-016 "Party Invite Deeplinks"
  Area: Consumer App | Component: Engineering | Epic: TMS-01
```

## Jira Sync Behavior

### Component Sync

On `blue sync` or `blue jira setup`:
1. Read `components` from domain.yaml
2. Create missing Jira components in the project (idempotent)
3. Set component leads if configured
4. Each story's `component` field → Jira component assignment

### Story Sync

When syncing a story to Jira:

```
Jira issue:
  Summary:     [CON-015] Party creation API v2
  Type:        Story
  Components:  [Engineering]            ← from story.components (multi-select)
  Labels:      [area:CON, api, party]   ← area as label + story labels
  Epic Link:   TMS-03                   ← from story.epic
  Sprint:      s01                      ← from story.sprint
  Story Points: 3                       ← from story.points
```

### Filtering

| View | JQL |
|------|-----|
| All engineering work | `component = Engineering` |
| All consumer app work | `labels = "area:CON"` |
| Engineering on consumer app | `component = Engineering AND labels = "area:CON"` |
| All work in Party v2 epic | `"Epic Link" = TMS-03` |
| Consumer app in current sprint | `labels = "area:CON" AND sprint in openSprints()` |

## Blue Code Changes

### domain.rs

```rust
pub struct PmDomain {
    pub org: String,
    pub key: String,                    // org epic key (TMS)
    pub jira: Option<JiraConfig>,
    pub components: Vec<Component>,     // NEW: functional ownership
    pub areas: Vec<Area>,               // NEW: product surfaces
    pub repos: Vec<RepoEntry>,          // simplified: no more key field
}

pub struct JiraConfig {
    pub domain: String,                 // themovesocial.atlassian.net
    pub project_key: String,            // SCRUM
    pub drift_policy: String,
}

pub struct Component {
    pub name: String,                   // "Engineering"
    pub description: Option<String>,
    pub lead: Option<String>,           // Jira default assignee
}

pub struct Area {
    pub key: String,                    // "CON" — 2-4 uppercase letters
    pub name: String,                   // "Consumer App"
    pub description: Option<String>,
    pub components: Vec<String>,        // typical components for this area
    pub repos: Vec<String>,             // code repos this area touches
}

pub struct RepoEntry {
    pub name: String,
    pub url: Option<String>,
    pub description: Option<String>,
    // key field REMOVED — areas own the key prefixes now
}
```

### id.rs

- `next_story_id()` namespaces by **area key** (not repo key or domain key)
- `format_story_id()` unchanged: `{KEY}-{N:03}`
- Collision check scans local PM repo + Jira for `area:CON` labeled issues

### sync.rs

- `PmDocSource::discover()` reads `component` and `area` from story front matter
- Issue creation sets Jira component field (multiple components per issue)
- `area:{key}` label added alongside story labels
- Component creation/validation during setup phase

### locator.rs

- `locate_pm_repo()` returns available areas for current repo (via `areas[].repos`)
- Used by `blue rfc create` to suggest relevant areas

## Implementation Phases

### Phase 1: Schema — domain.yaml with Components & Areas

- `Component` struct: name, description, lead
- `Area` struct: key, name, description, components, repos
- Remove `key` from `RepoEntry`
- Extract `JiraConfig` sub-struct
- Validation: area keys unique, 2-4 uppercase, components reference valid names
- Parse and load updated domain.yaml

### Phase 2: Story Front Matter — area + component fields

- Add `area` and `component` fields to story YAML parsing
- `area` is required (determines ID prefix)
- `components` is a list (one or more Jira components)
- `repo` becomes optional (only for code-linked work)
- Update `next_story_id()` to namespace by area key

### Phase 3: Jira Component Sync

- `blue jira setup-components` — create/update Jira components from domain.yaml
- Set component leads as default assignees
- Story sync sets Jira component field (multiple components supported)
- Add `area:{key}` label to synced issues

### Phase 4: Auto-Categorization

- `blue rfc create` resolves area from current repo + title keywords
- Interactive area and component selection (multi-select for components)
- Area suggestion based on `areas[].repos` containing current repo
- Component suggestion based on repo type and area's typical components

### Phase 5: Documentation — README Domain Setup Guide

Update the Blue README (or a dedicated `docs/pm/` guide) with a worked example of setting up a domain with the two-tier model:
- What `domain.yaml` looks like for a real org
- How to choose components (standard 10 as starting point)
- How to identify areas from your product surfaces
- Walkthrough: from blank PM repo to first `blue sync`
- Example: The Move Social's CON/MER/API/INF area breakdown with rationale

### Phase 6: Claude Skill — `domain-setup`

Create a Claude Code skill (`skills/domain-setup/SKILL.md`) that guides users through domain setup interactively. Installed via `blue install`.

**Input:** A product description (what you're building, who it's for, what surfaces exist).

**Output:** A proposed `domain.yaml` with:
- Relevant components (subset of standard 10 — don't include People if you're solo)
- Suggested areas with keys, names, descriptions
- Area-to-component mapping
- Area-to-repo mapping (if repos are known)

**Flow:**

```
User: /domain-setup

Blue: Tell me about your product. What are you building?

User: A social app for coordinating group outings. Consumer mobile app,
      merchant dashboard for venues, and a marketing landing page.
      Backend is a single API repo. Using Jira Cloud.

Blue: Here's what I'd propose:

      Components (functional teams):
        Engineering, Product, Design, Finance, Growth

      Areas (product surfaces):
        CON  Consumer App     — mobile/web experience for end users
        MER  Merchant App     — venue dashboard, analytics, billing
        LND  Landing Page     — marketing site, onboarding funnel
        API  Backend API      — shared services, auth, data pipeline
        INF  Infrastructure   — CI/CD, cloud, monitoring

      Does this look right? I can adjust areas, add/remove components,
      or tweak descriptions.

User: Add a Security component, and split API into API and DATA
      (we have a separate data pipeline).

Blue: Updated. Writing domain.yaml...
```

**Skill behavior:**
- Reads existing `domain.yaml` if present (edit mode vs create mode)
- Proposes only relevant components (solo founder doesn't need People/Operations)
- Derives areas from product surfaces described by the user
- Suggests 2-4 letter keys that are short, memorable, and unambiguous
- Writes `domain.yaml` to the PM repo root
- Optionally runs `blue jira setup-components` to create Jira components

## Open Questions

- [x] ~~Should `component` on a story be single-select or multi-select?~~ **Multi-select.** A story can have multiple components (e.g., Engineering + Security for an auth story). Jira supports multiple components natively.
- [x] ~~Should areas declare `components` as a hard constraint or soft suggestion?~~ **Soft suggestion.** The `components` list on an area is for auto-categorization hints, not enforcement.
- [x] ~~Epic IDs stay org-scoped (`TMS-01`) or also get area prefixes?~~ **Stay org-scoped.** Epics are cross-area by design.

## Test Plan

- [ ] domain.yaml parses with components, areas, and repos (no repo keys)
- [ ] Area key validation: 2-4 uppercase, unique
- [ ] Component names validated against domain.yaml list
- [ ] `next_story_id("CON")` increments within CON namespace
- [ ] `next_story_id("MER")` increments within MER namespace (independent)
- [ ] Jira collision check scans for `area:CON` labeled issues
- [ ] Story YAML with `area` + `components` (list) fields parsed correctly
- [ ] Multi-component story: `components: [Engineering, Security]` → both set in Jira
- [ ] `blue jira setup-components` creates Jira components idempotently
- [ ] Story sync sets Jira component field (multiple) and `area:` label
- [ ] `blue rfc create` in `themove-backend` suggests CON, MER, API areas
- [ ] Auto-categorization: "merchant" in title → suggests MER
- [ ] Component auto-suggestion uses area's typical components list
- [ ] README/docs include domain setup walkthrough with worked example
- [ ] `domain-setup` skill installed via `blue install`
- [ ] Skill proposes components and areas from a product description
- [ ] Skill writes valid domain.yaml that passes schema validation
- [ ] Skill handles edit mode (existing domain.yaml) and create mode (blank)

---

*"Right then. Let's get to it."*

— Blue
