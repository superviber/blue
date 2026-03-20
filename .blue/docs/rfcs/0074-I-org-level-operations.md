# RFC 0074: Org-Level Operations

**Status**: Implemented
**Created**: 2026-03-18
**Last Updated**: 2026-03-20

| Depends On | RFC 0067 (org layout), RFC 0063/0068/0070 (PM + Jira) |
|------------|--------------------------------------------------------|
| Story      | —                                                      |

## Problem

Blue currently operates from within a single repo. You run `blue status` inside `~/code/superviber/blue/` and it knows about that repo only. But real work spans repos — an initiative might touch multiple repos simultaneously. The PM system (domain.yaml, epics, stories) exists but lives in a single repo with no connection to the org-level view.

Today's gaps:

1. **No org-level awareness** — Blue doesn't know when you're in the org root vs inside a repo. It only works inside repos.
2. **PM repo is invisible** — The domain.yaml and epic/story markdown files exist somewhere, but blue doesn't know which repo is the PM repo or how to find it.
3. **RFCs are repo-local** — An RFC in a repo's `.blue/docs/rfcs/` has no link to Jira tickets, epics, or cross-repo work items.
4. **No work organization assistance** — When a user creates an RFC, there's no suggestion to link it to existing Jira work or propose new epics/stories.
5. **Context switching is manual** — Moving between repos within an org requires the user to mentally track what's happening where.

## Decision

Use the existing PM repo as the org-level coordination point. Add `org.yaml` to the PM repo and use Claude Code hooks (session start, post-compact) to dynamically inject org context — no per-repo `CLAUDE.md` needed.

Blue should:

1. **Discover the org** via `org.yaml` at the org root, pointing to the PM repo
2. **Detect context** — org root, repo, or worktree — and adapt behavior accordingly
3. **Use hooks for context injection** — session start and post-compact hooks read `org.yaml` and inject org awareness into the Claude session dynamically
4. **Link RFCs to PM** — When creating or approving an RFC, offer to link it to existing Jira tickets or create new ones
5. **Suggest organizational artifacts** — Analyze RFC scope and suggest epics, stories, component assignments
6. **Aggregate across repos** — `blue status` from the org root shows all repos, all active RFCs, all in-flight work

## Scope

### In Scope
- `org.yaml` at org root pointing to PM repo
- PM repo as the org coordination point (it already holds domain.yaml, epics, sprints, Jira config)
- Context detection (org root vs repo vs worktree)
- Hook-based org context injection (session start, post-compact)
- RFC ↔ Jira ticket linking (bidirectional)
- Work organization skill (suggest epics, stories, component assignments)
- Org-level aggregation commands (status, rfc list, etc.)
- `blue org bootstrap` — clone all org repos from PM repo's domain.yaml
- Skills that are org-aware without requiring per-repo configuration

### Out of Scope
- Multi-org operations (operating across different GitHub orgs simultaneously)
- Automated Jira ticket creation without user confirmation
- Changing the existing single-repo workflow (this extends it, doesn't replace it)
- CI/CD or GitHub Actions integration
- Real-time cross-repo event sync (daemon scope — future RFC)
- Git submodules (see Resolved Questions)

## Approach

### The PM Repo as Org Coordination Point

Most orgs already have a PM repo. For example, `the-move-social` has:

```
the-move-social/
├── project-management/           ← PM repo — already the org brain
│   ├── domain.yaml               ← repos, areas, components, Jira config
│   ├── jira.toml                 ← Jira connection details
│   ├── epics/                    ← 12 epics, 59+ stories
│   ├── sprints/                  ← 9 sprints
│   ├── releases/                 ← release definitions
│   └── plans/                    ← infrastructure and design plans
├── themove-backend/              ← repo
├── themove-consumer/             ← repo
├── themove-landing/              ← repo
├── themove-product/              ← repo
├── the-move-consumer-demo/       ← repo
├── the-move-merchant-demo/       ← repo
└── corporate-docs/               ← repo
```

The PM repo already holds `domain.yaml` which declares all repos, areas, components, and Jira config. Rather than creating a new repo or duplicating this structure, we promote the PM repo to be the org coordination point by adding `org.yaml`.

### org.yaml — The Org Manifest

Lives at the org root (e.g., `~/code/the-move-social/org.yaml`):

```yaml
# ~/code/the-move-social/org.yaml
org: the-move-social
pm_repo: project-management
```

That's it. `org.yaml` is intentionally minimal — it exists only to answer: "which directory is the PM repo?" Everything else (repos, areas, components, Jira config) is already in the PM repo's `domain.yaml` and `jira.toml`.

**org.yaml lives at the org root, not checked in:**
- Location: `~/code/the-move-social/org.yaml`
- Not version-controlled — it describes the local org folder structure
- `blue init` (from org root) creates it interactively
- Each developer's org.yaml points to their local PM repo clone

**Discovery chain:**
1. Is there an `org.yaml` in the current directory? → org root
2. Is there an `org.yaml` in a parent directory? → inside a repo within an org
3. Is `.git` a file (not dir)? → worktree (existing RFC 0073 logic)
4. Is there a `.blue/` folder? → standalone repo (existing behavior)
5. None of the above → not a blue-managed directory

### Hook-Based Context Injection

Instead of requiring `CLAUDE.md` files at the org root or in every repo, Blue uses Claude Code hooks to dynamically inject org context. This keeps `org.yaml` as the single source of truth.

**Session start hook** (`/lc-start` or equivalent):

```
/lc-start (from anywhere in the org)
  ├── walk up directories looking for org.yaml
  ├── if found:
  │   ├── read org.yaml → resolve PM repo path
  │   ├── read PM repo's domain.yaml → repos, areas, components
  │   ├── read PM repo's jira.toml → Jira connection
  │   ├── detect current context (org root, which repo, worktree)
  │   └── inject context into session:
  │       "Org: the-move-social
  │        PM repo: ./project-management
  │        Repos: themove-backend, themove-consumer, ...
  │        Areas: CON (Consumer), MER (Merchant), API (Backend), ...
  │        Jira: TMS @ themovesocial.atlassian.net
  │        You're at: org root"
  └── if not found:
      └── standalone repo mode (existing behavior)
```

**Post-compact hook** re-reads `org.yaml` and re-injects context so it survives context compaction.

**Why hooks instead of CLAUDE.md:**
- **Single source of truth** — `org.yaml` + `domain.yaml` drive everything, no duplication
- **Dynamic** — add a repo to `domain.yaml`, next session picks it up automatically
- **Location-aware** — same hook, different output depending on where you are (org root vs repo vs worktree)
- **No per-repo config** — repos don't need to know about the org; the hook figures it out

### Context Detection

Blue detects context and adapts:

| Context | Detection | Behavior |
|---------|-----------|----------|
| **Org root** | `org.yaml` in cwd | Aggregate view, cross-repo operations, PM access |
| **Repo** | Ancestor has `org.yaml` | Repo-specific + org-aware (can reach PM repo) |
| **Worktree** | `.git` is file | Implementation-only (existing RFC 0073 behavior) |
| **Standalone repo** | `.blue/` exists, no org.yaml above | Legacy single-repo mode (backward compatible) |

### PM Repo as Source of Truth

The PM repo already holds everything needed for org-level coordination:

| Artifact | File | Purpose |
|----------|------|---------|
| Repos | `domain.yaml` → `repos:` | Which repos exist, URLs, descriptions |
| Areas | `domain.yaml` → `areas:` | Domain areas (CON, MER, API, etc.) with component and repo mappings |
| Components | `domain.yaml` → `components:` | Functional areas (Engineering, Product, Design, etc.) |
| Jira | `jira.toml` | Jira domain, project key, drift policy |
| Epics | `epics/` | Epic definitions with stories |
| Sprints | `sprints/` | Sprint plans |
| Releases | `releases/` | Release definitions |

**Example from `the-move-social`:**

```yaml
# project-management/domain.yaml (already exists)
org: the-move-social
key: TMS

jira:
  domain: themovesocial.atlassian.net
  project_key: TMS
  drift_policy: warn

repos:
  - name: themove-consumer
    url: git@github.com:the-move-social/themove-consumer.git
    description: "Consumer app — Next.js (web) + Expo (iOS/Android)"
  - name: themove-backend
    url: git@github.com:the-move-social/themove-backend.git
    description: "Rust API server — Axum + PostgreSQL + Nitro Enclaves"
  # ... 6 more repos

areas:
  - key: CON
    name: Consumer App
    components: [Engineering, Product, Design]
    repos: [themove-backend, themove-consumer]
  - key: MER
    name: Merchant App
    components: [Engineering, Product, Design, Finance]
    repos: [themove-backend, themove-merchant]
  # ... 5 more areas
```

**No new configuration format needed.** The PM repo's existing `domain.yaml` already declares the full org structure. `org.yaml` just says "look over there."

### Bootstrap Flow

Onboarding a new developer:

```bash
# Step 1: Clone the PM repo into the org folder
mkdir ~/code/the-move-social
cd ~/code/the-move-social
git clone git@github.com:the-move-social/themove-project-management.git project-management

# Step 2: Create org.yaml (one-time, or blue init does it)
blue init
# → Detects no .git, scans for repos
# → Finds project-management/ with domain.yaml
# → Creates org.yaml pointing to it

# Step 3: Bootstrap all repos
blue org bootstrap
# → Reads org.yaml → finds PM repo → reads domain.yaml
# → Clones all repos as siblings:
#   git clone git@github.com:the-move-social/themove-consumer.git
#   git clone git@github.com:the-move-social/themove-backend.git
#   ... etc
# → Result: full org layout on disk
```

### RFC ↔ PM Linking

When an RFC is created or approved, blue should:

1. **Search existing work** — Query Jira and PM repo markdown for related tickets
   - Match by keywords from RFC title/problem statement
   - Match by component (if RFC touches specific areas defined in domain.yaml)
   - Match by repo (stories assigned to this repo)
2. **Present matches to user** — Show candidate tickets with relevance reasoning
3. **User chooses:**
   - Link to existing ticket(s)
   - Create new ticket(s)
   - Skip (no PM linking)
4. **Update both sides:**
   - RFC front matter gets `| Jira | TMS-123 |` (existing convention)
   - Jira ticket gets a comment or custom field linking back to the RFC
   - PM repo story/epic markdown gets updated with RFC reference

**RFC front matter extension:**

```markdown
# RFC 0074: Org-Level Operations

**Status**: Implemented
**Created**: 2026-03-18

| Jira    | TMS-456                              |
| Epic    | TMS-200 (Platform Improvements)      |
| Repos   | project-management, themove-backend   |
```

### Org-Aware Skills

Existing skills become org-aware through the hook-injected context. No per-repo configuration needed.

| Skill | Org-Aware Behavior |
|-------|-------------------|
| `/lc-start` | Reads `org.yaml`, injects org context, detects location |
| `/lc-plan` | Knows all repos and areas, can create cross-repo recipes |
| `/lc-cook` | Can dispatch work to the right repo |
| `/lc-decide` | Links decisions to PM repo epics/stories |
| `/lc-learn` | Routes lessons to the right hat, cross-links across repos |
| `blue rfc create` | Asks which repo, creates RFC there, offers PM/Jira linking |
| `blue rfc list` | From org root: aggregates across all repos |
| `blue org bootstrap` | Clones all repos from domain.yaml |
| `blue org status` | Aggregate view: all repos, RFCs, Jira state |
| `blue org sync` | Batch sync RFCs ↔ PM/Jira, report drift |

**New skill: `/blue-work`**

An interactive skill for organizing work from the org level:

```
/blue-work

Scanning org: the-move-social (8 repos)
Reading PM: project-management/domain.yaml
Checking Jira: TMS @ themovesocial.atlassian.net

Active epics: 12 (59 stories)
Current sprint: S04 (Sprint 4)

Unlinked RFCs:
  themove-backend:  003-D-passkey-challenge  — no Jira ticket

Suggestions:
  [1] Link RFC 003 to epic TMS-01 "Party System"
      → Relates to CON-002 (mutual connections) auth flow
  [2] Create new story under TMS-10 "Onboarding"
      → Passkey challenge is part of signup
  [3] Skip — review later

What would you like to do?
```

### Work Organization Skill

Integrated into the RFC creation flow:

1. **Analyzes RFC scope** — Reads phases, tasks, and affected repos
2. **Maps to domain.yaml** — Identifies which areas and components are touched
3. **Suggests organizational artifacts:**
   - "This RFC spans 3 phases across 2 repos — suggest creating an epic?"
   - "Phase 2 touches auth component — assign to area CON?"
   - "Found existing epic TMS-01 'Party System' — link this RFC as a child story?"
   - "No story exists for the passkey work — create one?"
4. **User confirms/edits** — All suggestions require user approval
5. **Executes** — Creates/updates Jira tickets, PM markdown, RFC front matter

### Org-Level Commands

When run from the org root, existing commands gain aggregate behavior:

| Command | Repo Behavior (existing) | Org Behavior (new) |
|---------|--------------------------|---------------------|
| `blue status` | Show repo status | Show all repos' status + PM summary |
| `blue rfc list` | List repo's RFCs | List all RFCs across all repos |
| `blue rfc create` | Create in current repo | Ask which repo(s), create there, link to PM |
| `blue next` | What's next for this repo | What's next across the org |
| `blue lint` | Lint current repo | Lint all repos + cross-repo consistency |

**New org-only commands:**

| Command | Purpose |
|---------|---------|
| `blue org status` | Detailed org overview (repos, RFCs, PM, Jira) |
| `blue org bootstrap` | Clone all repos from domain.yaml |
| `blue org scan` | Discover repos, suggest domain.yaml entries |
| `blue org sync` | Sync all repos' RFCs with PM/Jira |
| `blue org link <rfc>` | Link an RFC to PM/Jira interactively |

**`blue init` becomes context-aware:**

| Context | Behavior |
|---------|----------|
| Org root (no org.yaml yet) | Scan for PM repo (has domain.yaml?), create org.yaml pointing to it |
| Org root (org.yaml exists) | Existing behavior + refresh repo list from domain.yaml |
| Inside a repo | Existing repo-level init (unchanged) |

### Cross-Repo RFC Dependencies in Guard

When an RFC declares `Depends On` other RFCs (potentially in other repos), the guard should enforce this:

1. Parse RFC front matter for `Depends On` field
2. If dependency is in the same repo → existing glob check
3. If dependency is in another repo → resolve via org.yaml → PM repo → sibling repo path → glob for `*-{A|I}-slug.md`
4. Block implementation if any dependency RFC is still Draft

**Guard reads org.yaml only when cross-repo deps exist** — no performance impact for single-repo RFCs. The guard walks up to find org.yaml at the org root.

### Automatic PM Sync

RFC lifecycle events trigger automatic sync:

| Event | Sync Action |
|-------|-------------|
| `blue rfc create` | Search Jira for related tickets, present matches, offer linking |
| `blue rfc approve` | If linked to Jira, transition ticket to "In Progress". If unlinked, prompt to link. |
| `/wt done` (RFC → Implemented) | Transition linked Jira ticket to "Done". Update PM story status. |
| `blue org sync` (manual) | Batch sync all RFCs ↔ PM/Jira, report drift |

All Jira *creation* still requires user confirmation. Transitions (status changes) are automatic because they follow the RFC lifecycle the user already approved.

## Phases

### Phase 1: org.yaml and Context Detection
- [ ] Define `OrgManifest` struct (parse org.yaml — just `org` + `pm_repo` fields)
- [ ] Implement context detection chain (org root → repo → worktree → standalone)
- [ ] Implement PM repo resolution (org.yaml → pm_repo → read domain.yaml)
- [ ] Extend `blue init` — detect org root (no .git, has subdirs with domain.yaml), create org.yaml
- [ ] `blue org scan` — discover repos, suggest domain.yaml entries
- [ ] Tests for context detection across all scenarios

### Phase 2: Hook-Based Context Injection
- [ ] Session start hook — walk up to find org.yaml, read PM repo, inject context
- [ ] Post-compact hook — re-inject org context after compaction
- [ ] Location-aware context output (different info at org root vs repo vs worktree)
- [ ] Hook reads domain.yaml for repos, areas, components
- [ ] Hook reads jira.toml for Jira connection
- [ ] Skill awareness — `/lc-start`, `/lc-plan`, `/lc-cook` etc. use injected context

### Phase 3: Bootstrap and Org Commands
- [ ] `blue org bootstrap` — clone all repos from domain.yaml as siblings
- [ ] `blue org status` — aggregate view across repos
- [ ] Cross-repo RFC listing (`blue rfc list` from org root)
- [ ] Cross-repo status aggregation

### Phase 4: RFC ↔ PM Linking
- [ ] Keyword/component matching for RFC → Jira ticket suggestions
- [ ] `blue org link <rfc>` — interactive linking flow
- [ ] RFC front matter extension (Jira, Epic, Repos fields)
- [ ] Bidirectional update (RFC ↔ Jira comment/field)
- [ ] Integration into `blue rfc create` flow (suggest linking on creation)
- [ ] Auto-sync on `blue rfc create` and `blue rfc approve` (search + transition)
- [ ] Auto-sync on `/wt done` (transition linked ticket to Done)

### Phase 5: Work Organization Skill
- [ ] `/blue-work` skill — scan, suggest, organize
- [ ] Epic suggestion heuristics (multi-phase, multi-repo detection)
- [ ] Story suggestion from RFC phases
- [ ] Component/area auto-assignment from domain.yaml mapping
- [ ] User confirmation flow for all suggestions
- [ ] PM markdown + Jira creation on confirmation

### Phase 6: Org-Level Command Adaptation
- [ ] `blue status` aggregate mode (org root detection)
- [ ] `blue rfc list` cross-repo aggregation
- [ ] `blue rfc create` with repo selection and PM linking
- [ ] `blue next` org-level prioritization
- [ ] `blue lint` cross-repo consistency checks
- [ ] `blue org sync` — batch sync all RFCs ↔ PM/Jira

### Phase 7: Guard and Skill Integration
- [ ] Cross-repo RFC dependency resolution in guard (walk up to org.yaml → find sibling repo → glob)
- [ ] Update `/wt start` to show PM context (linked tickets, epic)
- [ ] Update `/wt done` to auto-sync PM status on completion
- [ ] Update RFC approval flow to trigger PM linking prompt
- [ ] Update `blue rfc create` to auto-search Jira for related work

## Related
- [RFC 0067: Org-Mapped Directory Layout](./0067-D-org-directory-layout.md) — foundational org structure
- [RFC 0063: Jira Cloud Integration](./0063-D-jira-cloud-integration.md) — tracker sync
- [RFC 0068: PM Repo Ground Truth](./0068-D-pm-repo-ground-truth.md) — PM repo as source of truth
- [RFC 0070: Components and Areas](./0070-D-components-and-areas.md) — domain.yaml structure
- [RFC 0073: Branch Workflow Enforcement](./0073-A-branch-workflow-enforcement.md) — worktree/guard system this extends

## Resolved Questions

| # | Question | Decision | Rationale |
|---|----------|----------|-----------|
| 1 | Where does org.yaml live? | At the org root, not checked in | It's a local pointer to the PM repo. Minimal — just `org` name and `pm_repo` path. Each dev creates it via `blue init`. |
| 2 | Separate org repo or reuse PM repo? | Reuse the PM repo | The PM repo (e.g., `project-management`) already holds domain.yaml, epics, sprints, Jira config. No need for a new repo — just add org.yaml at the org root to point to it. |
| 3 | Git submodules for multi-repo? | No — flat sibling layout | Submodules pin commits (opposite of what you want for active dev), interact poorly with worktrees (RFC 0073), and force rigid cloning. `org.yaml` + `blue org bootstrap` gives the same onboarding benefit without coupling git histories. |
| 4 | CLAUDE.md or hooks for org context? | Hooks (session start + post-compact) | Hooks read `org.yaml` → `domain.yaml` dynamically and inject context. Single source of truth, location-aware, no duplication. CLAUDE.md would duplicate what's already in domain.yaml and go stale. |
| 5 | Where does Jira config live? | PM repo's `jira.toml` (already exists) | Jira is an org-level concern already tracked in the PM repo. No need to duplicate it in org.yaml. |
| 6 | Cross-repo RFC dependencies in guard? | Yes — guard checks cross-repo deps | If RFC in repo A depends on RFC in repo B, guard reads org.yaml → finds repo B → globs for dependency RFC status. Blocks implementation if dependency RFC isn't Approved/Implemented. |
| 7 | Automatic or manual sync? | Automatic on RFC create/approve | `blue org sync` runs automatically when RFCs are created or approved. Reduces forgetting. User still confirms any Jira ticket creation (no silent side effects). |

## Open Questions

None — all resolved.

## Notes
- This RFC explicitly lists "multi-repo workflow coordination" as out of scope for RFC 0073. This is the follow-up.
- The PM repo approach works because most orgs already have a coordination repo. `the-move-social/project-management` is a real example with domain.yaml, 12 epics, 59 stories, 9 sprints, and Jira config already in place.
- `org.yaml` is intentionally minimal (2 fields). All the rich org structure lives in the PM repo's `domain.yaml` where it's already maintained.
- Hooks are the key architectural choice — they make every skill org-aware without any per-repo configuration. The session start hook reads `org.yaml` once and injects everything Claude needs.
- All PM suggestions require user confirmation — blue never auto-creates Jira tickets or organizational artifacts without explicit approval.
- The `/blue-work` skill is the primary interface for work organization. It should feel like a conversation, not a batch operation.
