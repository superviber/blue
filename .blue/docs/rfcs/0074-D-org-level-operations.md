# RFC 0074: Org-Level Operations

**Status**: Draft
**Created**: 2026-03-18
**Last Updated**: 2026-03-18

| Depends On | RFC 0067 (org layout), RFC 0063/0068/0070 (PM + Jira) |
|------------|--------------------------------------------------------|
| Story      | —                                                      |

## Problem

Blue currently operates from within a single repo. You run `blue status` inside `~/code/superviber/blue/` and it knows about that repo only. But real work spans repos — an initiative might touch `blue`, `coherence`, and `aperture` simultaneously. The PM system (domain.yaml, epics, stories) exists but lives in a single repo with no connection to the org-level view.

Today's gaps:

1. **No org-level awareness** — Blue doesn't know when you're in `~/code/superviber/` (the org root) vs `~/code/superviber/blue/` (a repo). It only works inside repos.
2. **PM repo is invisible** — The domain.yaml and epic/story markdown files exist somewhere, but blue doesn't know which repo is the PM repo or how to find it.
3. **RFCs are repo-local** — An RFC in `blue/.blue/docs/rfcs/` has no link to Jira tickets, epics, or cross-repo work items.
4. **No work organization assistance** — When a user creates an RFC, there's no suggestion to link it to existing Jira work or propose new epics/stories.
5. **Context switching is manual** — Moving between repos within an org requires the user to mentally track what's happening where.

## Decision

Make the org folder the primary operating context. Blue should:

1. **Always operate from an org folder** with an `org.yaml` manifest at the org root
2. **Detect context** — org root, repo, or worktree — and adapt behavior accordingly
3. **Know the PM repo** — `org.yaml` declares which repo holds domain.yaml/epics/stories
4. **Link RFCs to PM** — When creating or approving an RFC, offer to link it to existing Jira tickets or create new ones
5. **Suggest organizational artifacts** — Analyze RFC scope and suggest epics, stories, component assignments
6. **Aggregate across repos** — `blue status` from the org root shows all repos, all active RFCs, all in-flight work

## Scope

### In Scope
- `org.yaml` manifest at org root (replaces reliance on global config for org-level context)
- Context detection (org root vs repo vs worktree)
- PM repo designation and cross-repo PM linking
- RFC ↔ Jira ticket linking (bidirectional)
- Work organization skill (suggest epics, stories, component assignments)
- Org-level aggregation commands (status, rfc list, etc.)
- CLI adaptation based on detected context

### Out of Scope
- Multi-org operations (operating across different GitHub orgs simultaneously)
- Automated Jira ticket creation without user confirmation
- Changing the existing single-repo workflow (this extends it, doesn't replace it)
- CI/CD or GitHub Actions integration
- Real-time cross-repo event sync (daemon scope — future RFC)

## Approach

### org.yaml — The Org Manifest

Lives at the org root (e.g., `~/code/superviber/org.yaml`):

```yaml
# ~/code/superviber/org.yaml
org: superviber
provider: github

# Which repo holds PM artifacts (domain.yaml, epics/, stories/)
pm_repo: blue

# Repos in this org that blue manages (auto-discovered if omitted)
repos:
  - name: blue
    description: Development philosophy and toolset
  - name: coherence
    description: Knowledge management system
  - name: aperture
    description: Observability platform

# Jira connection (moved from domain.yaml to org level)
jira:
  domain: superviber.atlassian.net
  project_key: SV
  drift_policy: warn
```

**Why org.yaml instead of extending global config?**
- Global config (`~/.config/blue/config.toml`) tracks *which orgs exist* and *where they live*
- `org.yaml` tracks *what's inside the org* — repos, PM repo, Jira config
- Global config is per-machine; org.yaml is per-org

**org.yaml lives at the org root, not checked in:**
- Location: `~/code/superviber/org.yaml`
- Not version-controlled — it describes the local org folder structure, not project state
- `blue init` (from org root) creates it interactively, walking the user through PM repo selection and repo discovery
- Each developer's org.yaml may differ slightly (different repos cloned, different paths) — that's fine
- Gitignored by convention (each repo's `.gitignore` doesn't need to know about it; it's above the repo level)

**Discovery chain:**
1. Is there an `org.yaml` in the current directory? → org root
2. Is there an `org.yaml` in the parent directory? → inside a repo within an org
3. Is `.git` a file (not dir)? → worktree (existing RFC 0073 logic)
4. Is there a `.blue/` folder? → standalone repo (existing behavior)
5. None of the above → not a blue-managed directory

### Context Detection

```
~/code/superviber/              ← org root (org.yaml here)
  ├── org.yaml
  ├── blue/                     ← repo context
  │   ├── .blue/docs/rfcs/
  │   └── ...
  ├── coherence/                ← repo context
  │   ├── .blue/docs/rfcs/
  │   └── ...
  └── aperture/                 ← repo context
      ├── .blue/docs/rfcs/
      └── ...
```

Blue detects context and adapts:

| Context | Detection | Behavior |
|---------|-----------|----------|
| **Org root** | `org.yaml` in cwd | Aggregate view, cross-repo operations, PM access |
| **Repo** | Parent has `org.yaml` | Repo-specific + org-aware (can reach PM repo) |
| **Worktree** | `.git` is file | Implementation-only (existing RFC 0073 behavior) |
| **Standalone repo** | `.blue/` exists, no org.yaml above | Legacy single-repo mode (backward compatible) |

### PM Repo Integration

The PM repo (declared in `org.yaml`) holds:
- `domain.yaml` — areas, components, repos (existing RFC 0063/0070)
- `epics/` — epic markdown files with YAML front matter
- `stories/` — story markdown files with YAML front matter

**Key change:** When blue operates from any repo in the org, it can reach the PM repo via `org.yaml` → `pm_repo` → resolve path. This enables:

1. **RFC → Ticket lookup** — When creating an RFC, blue reads PM repo's epics/stories and Jira to find related work
2. **Ticket → RFC linking** — Jira tickets reference the RFC via a `blue_rfc` field or comment
3. **Cross-repo status** — `blue status` from org root aggregates RFC status across all repos + PM status

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
   - RFC front matter gets `| Jira | PROJ-123 |` (existing convention)
   - Jira ticket gets a comment or custom field linking back to the RFC
   - PM repo story/epic markdown gets updated with RFC reference

**RFC front matter extension:**

```markdown
# RFC 0074: Org-Level Operations

**Status**: Draft
**Created**: 2026-03-18

| Jira    | SV-456                          |
| Epic    | SV-200 (Platform Improvements)  |
| Repos   | blue, coherence                 |
```

### Work Organization Skill

A new skill `/blue-organize` (or integrated into RFC creation flow) that:

1. **Analyzes RFC scope** — Reads phases, tasks, and affected repos
2. **Maps to domain.yaml** — Identifies which areas and components are touched
3. **Suggests organizational artifacts:**
   - "This RFC spans 3 phases across 2 repos — suggest creating an epic?"
   - "Phase 2 touches auth component — assign to area CON?"
   - "Found existing epic SV-200 'Platform Improvements' — link this RFC as a child story?"
   - "No story exists for the PM sync work — create one?"
4. **User confirms/edits** — All suggestions require user approval
5. **Executes** — Creates/updates Jira tickets, PM markdown, RFC front matter

**Suggestion heuristics:**
- RFC with 3+ phases or 2+ repos → suggest epic
- Each phase → suggest story (if not already tracked)
- Component overlap with existing epics → suggest linking
- Unassigned area/component → suggest assignment

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
| `blue org scan` | Discover repos, suggest org.yaml entries |
| `blue org sync` | Sync all repos' RFCs with PM/Jira |
| `blue org link <rfc>` | Link an RFC to PM/Jira interactively |

**`blue init` becomes context-aware:**

| Context | Behavior |
|---------|----------|
| Org root (no org.yaml yet) | Create org.yaml interactively — scan for repos, ask which is the PM repo, set up Jira |
| Org root (org.yaml exists) | Existing behavior + refresh repo list |
| Inside a repo | Existing repo-level init (unchanged) |

### Skill: `/blue-work`

An interactive skill for organizing work from the org level:

```
/blue-work

Scanning org: superviber (3 repos, 12 active RFCs)
Checking Jira: SV project (8 open epics, 23 stories)

Unlinked RFCs:
  blue:  0071-D-remove-embedded-llm      — no Jira ticket
  blue:  0074-D-org-level-operations      — no Jira ticket

Suggestions:
  [1] Link RFC 0074 to epic SV-200 "Platform Improvements"
      → Create story "Org-level operations" under SV-200
  [2] Create new epic "CLI Simplification" for RFCs 0071, 0072
      → Group related removal/simplification work
  [3] Skip — review later

What would you like to do?
```

### `blue init` from Org Root

When run from a directory that contains repo subdirectories (but no `.git`), `blue init` enters org mode:

```
$ cd ~/code/superviber
$ blue init

No .git found — looks like an org folder.
Scanning for repos...

Found 3 git repos:
  blue/       (github:superviber/blue)
  coherence/  (github:superviber/coherence)
  aperture/   (github:superviber/aperture)

Which repo holds your project management artifacts (domain.yaml, epics, stories)?
  [1] blue
  [2] coherence
  [3] aperture
  [4] None yet — I'll set this up later
> 1

Jira integration?
  [1] Yes — connect to Jira
  [2] Not now
> 1

Jira domain: superviber.atlassian.net
Project key: SV

Created ~/code/superviber/org.yaml
```

The resulting org.yaml is a local file — not checked into any repo. It's the structure of the org on disk.

### Migration Path

**From current state:**
1. Users running blue inside a repo continue to work as-is (standalone mode)
2. `blue init` from an org folder creates `org.yaml` interactively
3. `blue org scan` discovers repos and suggests new entries
4. Existing `domain.yaml` in PM repo is referenced (not moved) via `pm_repo`
5. Jira config in `domain.yaml` is honored as fallback if org.yaml doesn't have it yet

**Backward compatibility:**
- No org.yaml? Everything works exactly as before (standalone repo mode)
- org.yaml present? Blue gains org-level awareness while repo-level commands stay identical
- domain.yaml in PM repo is authoritative for areas/components; org.yaml just points to it

### Cross-Repo RFC Dependencies in Guard

When an RFC declares `Depends On` other RFCs (potentially in other repos), the guard should enforce this:

1. Parse RFC front matter for `Depends On` field
2. If dependency is in the same repo → existing glob check
3. If dependency is in another repo → resolve via org.yaml → `pm_repo` → sibling repo path → glob for `*-{A|I}-slug.md`
4. Block implementation if any dependency RFC is still Draft

**Guard reads org.yaml only when cross-repo deps exist** — no performance impact for single-repo RFCs. The guard walks up to the parent directory to find org.yaml at the org root.

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
- [ ] Define `OrgManifest` struct (parse org.yaml)
- [ ] Implement context detection chain (org root → repo → worktree → standalone)
- [ ] Extend `blue init` — detect org root (no .git, has repo subdirs), create org.yaml interactively
- [ ] `blue org scan` — discover repos, suggest org.yaml entries
- [ ] Update `BlueGlobalConfig` to interop with org.yaml
- [ ] Tests for context detection across all scenarios

### Phase 2: PM Repo Awareness
- [ ] Resolve PM repo path from org.yaml
- [ ] Read domain.yaml, epics/, stories/ from PM repo
- [ ] `blue org status` — aggregate view across repos
- [ ] Cross-repo RFC listing (`blue rfc list` from org root)
- [ ] Cross-repo status aggregation

### Phase 3: RFC ↔ PM Linking
- [ ] Keyword/component matching for RFC → Jira ticket suggestions
- [ ] `blue org link <rfc>` — interactive linking flow
- [ ] RFC front matter extension (Jira, Epic, Repos fields)
- [ ] Bidirectional update (RFC ↔ Jira comment/field)
- [ ] Integration into `blue rfc create` flow (suggest linking on creation)
- [ ] Auto-sync on `blue rfc create` and `blue rfc approve` (search + transition)
- [ ] Auto-sync on `/wt done` (transition linked ticket to Done)

### Phase 4: Work Organization Skill
- [ ] `/blue-work` skill — scan, suggest, organize
- [ ] Epic suggestion heuristics (multi-phase, multi-repo detection)
- [ ] Story suggestion from RFC phases
- [ ] Component/area auto-assignment from domain.yaml mapping
- [ ] User confirmation flow for all suggestions
- [ ] PM markdown + Jira creation on confirmation

### Phase 5: Org-Level Command Adaptation
- [ ] `blue status` aggregate mode (org root detection)
- [ ] `blue rfc list` cross-repo aggregation
- [ ] `blue rfc create` with repo selection and PM linking
- [ ] `blue next` org-level prioritization
- [ ] `blue lint` cross-repo consistency checks
- [ ] `blue org sync` — batch sync all RFCs ↔ PM/Jira

### Phase 6: Guard and Skill Integration
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
| 1 | Where does org.yaml live? | At the org root, not checked in | It describes local org folder structure — which repos are cloned, where PM lives. Each dev may have a slightly different setup. `blue init` from org root creates it interactively. |
| 2 | Where does Jira config live? | org.yaml (not domain.yaml) | Jira is org-level concern. domain.yaml stays focused on areas/components/repos. Existing domain.yaml Jira config becomes a fallback for backward compat. |
| 3 | Cross-repo RFC dependencies in guard? | Yes — guard checks cross-repo deps | If RFC in repo A depends on RFC in repo B, guard reads org.yaml → finds repo B → globs for dependency RFC status. Blocks implementation if dependency RFC isn't Approved/Implemented. |
| 4 | Automatic or manual sync? | Automatic on RFC create/approve | `blue org sync` runs automatically when RFCs are created or approved. Reduces forgetting. User still confirms any Jira ticket creation (no silent side effects). |

## Open Questions

None — all resolved.

## Notes
- This RFC explicitly lists "multi-repo workflow coordination" as out of scope for RFC 0073. This is the follow-up.
- The org.yaml approach keeps the global config lightweight (just "which orgs exist") while org.yaml captures the rich per-org structure.
- All PM suggestions require user confirmation — blue never auto-creates Jira tickets or organizational artifacts without explicit approval.
- The `/blue-work` skill is the primary interface for work organization. It should feel like a conversation, not a batch operation.
