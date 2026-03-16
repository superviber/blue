# RFC 0069: Multi-Domain PM Keys

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-15 |
| **Story** | — |
| **Extends** | RFC 0068 |

---

## Summary

Extend the PM repo to manage all business domains — not just engineering. Each functional domain gets a standard key prefix. Stories are categorized by domain, epics can span domains, and `blue rfc create` auto-categorizes work into the right domain.

## Motivation

RFC 0068 introduced repo-based keys (BKD, FRD, PRD) for engineering work. But a solo founder or small team manages far more than code: financial planning, legal compliance, design systems, go-to-market strategy, hiring, and operations all need the same structured tracking.

By giving each functional domain its own key, the PM repo becomes the single source of truth for **all** work, not just engineering. Jira stays organized because every issue is namespaced to its domain.

## Standard Domain Keys

10 standard domains, each with a 3-letter key:

| Key | Domain | Covers | Example Stories |
|-----|--------|--------|-----------------|
| **ENG** | Engineering | Backend, frontend, infrastructure, DevOps, CI/CD | ENG-001: "Create Party API endpoint" |
| **PRD** | Product | Feature specs, roadmap, user research, requirements | PRD-001: "Party system user stories" |
| **DSN** | Design | UI/UX, visual identity, brand assets, design system | DSN-001: "Party creation flow mockups" |
| **BIZ** | Business | Strategy, partnerships, monetization, go-to-market | BIZ-001: "Merchant partnership outreach" |
| **FIN** | Finance | Budgets, runway, pricing models, projections, accounting | FIN-001: "Q2 runway projection" |
| **OPS** | Operations | Processes, workflows, documentation, team coordination | OPS-001: "Sprint cadence setup" |
| **LGL** | Legal | Contracts, compliance, privacy, IP, regulatory | LGL-001: "Terms of service draft" |
| **GRW** | Growth | Acquisition, retention, A/B testing, content, marketing | GRW-001: "Launch landing page" |
| **PPL** | People | Hiring, culture, onboarding, contractors, performance | PPL-001: "Backend engineer job posting" |
| **SEC** | Security | App security, infra security, audits, pen testing | SEC-001: "OAuth flow security review" |

### Why 10?

- **Not too few**: Lumping finance into "business" or security into "engineering" loses signal. When scanning Jira, `FIN-003` instantly tells you it's financial work.
- **Not too many**: Each domain must carry enough work to justify its own namespace. Sales, support, data, and content are folded into the closest domain (sales → BIZ, support → OPS, data → ENG, content → GRW) until they grow large enough to warrant their own key.
- **Expandable**: New domains can be added to `domain.yaml` at any time. The system doesn't hard-code the list.

### Relationship to Let'em Cook Hats

| Domain Key | Closest Hat(s) |
|------------|---------------|
| ENG | dev, (data when technical) |
| PRD | product |
| DSN | design |
| BIZ | business, sales |
| FIN | finance |
| OPS | ops, support |
| LGL | legal |
| GRW | growth, content |
| PPL | people |
| SEC | security |

The chef hat orchestrates across domains — chef-level epics span multiple domain keys.

## Domain.yaml Schema (Updated)

```yaml
org: the-move-social
key: TMS                          # org-wide key for epics

domain: themovesocial.atlassian.net
project_key: SCRUM
drift_policy: warn

# Functional domains (standard keys)
domains:
  - key: ENG
    name: Engineering
    description: "Backend, frontend, infrastructure, DevOps, CI/CD"
  - key: PRD
    name: Product
    description: "Feature specs, roadmap, user research, requirements"
  - key: DSN
    name: Design
    description: "UI/UX, visual identity, brand assets, design system"
  - key: BIZ
    name: Business
    description: "Strategy, partnerships, monetization, go-to-market"
  - key: FIN
    name: Finance
    description: "Budgets, runway, pricing models, projections"
  - key: OPS
    name: Operations
    description: "Processes, workflows, documentation, team coordination"
  - key: LGL
    name: Legal
    description: "Contracts, compliance, privacy, IP, regulatory"
  - key: GRW
    name: Growth
    description: "Acquisition, retention, A/B testing, content, marketing"
  - key: PPL
    name: People
    description: "Hiring, culture, onboarding, contractors"
  - key: SEC
    name: Security
    description: "App security, infra security, audits, pen testing"

# Code repos (for linking RFCs to stories)
repos:
  - name: themove-backend
    key: BKD
    domain: ENG
    url: git@github.com:the-move-social/themove-backend.git
    description: "Backend API services"
  - name: themove-frontend
    key: FRD
    domain: ENG
    url: git@github.com:the-move-social/themove-frontend.git
    description: "React web application"
  - name: themove-product
    key: PRD-REPO
    domain: PRD
    url: git@github.com:the-move-social/themove-product.git
    description: "Product specs and feature docs"
```

### Key changes from RFC 0068

1. **`domains` list** replaces the flat `repos` as the primary organizational unit
2. **Repos link to domains** via `domain: ENG` field — a repo belongs to exactly one domain
3. **Story IDs use domain keys** (ENG-001) not repo keys (BKD-001) — simpler, more meaningful in Jira
4. **Repo keys are still valid** for code-level cross-referencing but stories are tracked by domain

## Epic and Story Structure

### Epics span domains

Epics use the org key (TMS-01, TMS-02) and can contain stories from any domain:

```
epics/
  TMS-01-party-system/
    _epic.md
    ENG-001-create-party-api.md
    ENG-002-party-invites-api.md
    DSN-001-party-creation-flow.md
    PRD-001-party-user-stories.md
  TMS-02-move-discovery/
    _epic.md
    ENG-003-ai-pipeline.md
    DSN-002-move-list-ui.md
    BIZ-001-merchant-outreach.md
```

### Story front matter

```yaml
---
type: story
id: ENG-001
title: "Create Party API endpoint"
domain: ENG
epic: TMS-01
repo: themove-backend          # which code repo (for ENG/DSN stories)
status: backlog
points: 3
sprint: s01
labels: [api, auth]
---
```

The `domain` field is the authoritative categorization. The `repo` field is optional and only relevant for domains that map to code repos (ENG, DSN).

## Auto-Categorization in `blue rfc create`

When creating an RFC, Blue determines the domain automatically:

### Step 1: Detect from context

Blue infers the domain from:
- **Current repo**: If in `themove-backend`, default to ENG
- **RFC title/content keywords**: "budget" → FIN, "hiring" → PPL, "compliance" → LGL
- **Explicit flag**: `--domain ENG` overrides auto-detection

### Step 2: Confirm with user

```
Creating RFC: "Party Invite Rate Limiting"

Detected domain: ENG (you're in themove-backend)
  [1] ENG - Engineering (Recommended)
  [2] SEC - Security
  [3] Other domain...
  [4] Help me decide

Domain: _
```

### Step 3: Link to story and epic

Per RFC 0068, every RFC belongs to a story. Blue creates the story in the correct domain:

```
Story: ENG-015 "Party Invite Rate Limiting"
Epic:
  [1] TMS-01 Party System (Recommended - matches context)
  [2] TMS-05 Live Move
  [3] Create new epic
  [4] Help me decide

→ Created ENG-015 under TMS-01
→ RFC linked: .blue/docs/rfcs/0070-party-invite-rate-limiting.draft.md
```

### Keyword-to-domain mapping

| Keywords | Domain |
|----------|--------|
| api, endpoint, database, migration, deploy, CI, infra, backend, frontend, bug, refactor | ENG |
| spec, requirement, user story, roadmap, feature, PRD | PRD |
| mockup, wireframe, UI, UX, design system, figma, brand | DSN |
| strategy, partnership, deal, pitch, GTM, market, sales | BIZ |
| budget, runway, revenue, pricing, invoice, tax, funding | FIN |
| process, workflow, documentation, onboarding, SOP | OPS |
| contract, compliance, privacy, GDPR, terms, IP, license | LGL |
| growth, acquisition, retention, A/B test, landing page, SEO, content, blog | GRW |
| hiring, culture, interview, contractor, performance review | PPL |
| security, audit, vulnerability, pen test, OAuth, encryption | SEC |

## Jira Organization

All stories sync to the same Jira project (e.g., SCRUM) but are organized by:

1. **Labels**: Every issue gets a `domain:{key}` label (e.g., `domain:ENG`, `domain:FIN`)
2. **Summary prefix**: `[ENG-001] Create Party API endpoint`
3. **Jira filters**: Pre-built JQL filters per domain:
   - `project = SCRUM AND labels = "domain:ENG"` → All engineering work
   - `project = SCRUM AND labels = "domain:FIN"` → All finance work

This keeps everything in one project (simple board, one backlog) while making domain-specific views trivial.

## Migration from RFC 0068

Existing PM repos with repo-based keys (PTY, DSC, VOT, etc.) need migration:

### Option A: Rename to domain keys (recommended)

```
PTY-001 → ENG-001 (engineering story)
DSC-001 → ENG-002 (engineering story)
PTY-001 had a design story → DSN-001
```

### Option B: Keep existing as-is, new work uses domain keys

Existing stories keep their keys. New stories use domain keys. Blue handles both formats during sync.

### Migration command

```
blue pm migrate-keys --from repo --to domain
```

Scans all stories, maps repo keys to domain keys using `domain.yaml` repo-to-domain mapping, renames files, updates IDs, and re-syncs to Jira.

## Implementation Phases

### Phase 1: Domain registry in domain.yaml
- Add `domains` list to `PmDomain` schema
- Add `domain` field to `RepoEntry`
- Story ID auto-increment per domain key namespace
- Validate domain keys are unique, 3 uppercase letters

### Phase 2: Auto-categorization in `blue rfc create`
- Keyword-to-domain detection
- Current-repo-to-domain detection via `domain.yaml`
- Interactive domain selection with "help me decide"
- Story creation in correct domain namespace

### Phase 3: Jira label organization
- Add `domain:{key}` labels on issue creation
- Sync domain labels for existing issues
- `blue jira filters` command to set up per-domain JQL saved filters

### Phase 4: Migration tooling
- `blue pm migrate-keys` for repo-key → domain-key migration
- Handles file renames, ID updates, Jira re-sync
- Dry-run mode for preview

## Open Questions

- [ ] Should domains be hard-coded (standard 10) or fully user-configurable? (Recommendation: standard 10 as defaults, user can add/remove)
- [ ] Should Jira components map to domains? (Components are another Jira organization primitive)
- [ ] Should domain keys be 3 letters or allow 2-4? (Recommendation: exactly 3 for consistency)
