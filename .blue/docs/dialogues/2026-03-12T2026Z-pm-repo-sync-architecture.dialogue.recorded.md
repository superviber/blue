# Alignment Dialogue: PM Repo Sync Architecture

**Draft**: Dialogue 2051
**Date**: 2026-03-12 20:26Z
**Status**: Converged
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone, 🧁 Eclair, 🧁 Donut, 🧁 Brioche, 🧁 Croissant, 🧁 Macaron

## Expert Pool

**Domain**: Project Management Integration Architecture
**Question**: How should Blue sync generalize to treat PM repos as the single source of truth for project artifacts across an org, projecting to Jira while maintaining git authority?

| Tier | Experts |
|------|--------|
| Core | DevTools Architect (CLI & Workflow Systems), Project Management Domain Expert (Jira/Linear/Shortcut), Data Modeling Specialist (YAML schemas, document formats), Distributed Systems Engineer (cross-repo coordination) |
| Adjacent | Developer Experience Advocate (ergonomics, onboarding), API Integration Specialist (REST, webhooks, sync protocols), Solo Founder Operator (wearing all hats, context switching), Security & Credential Management Engineer, Git Workflow Specialist (monorepos, worktrees, conventions) |
| Wildcard | Product Manager (roadmap planning, sprint ceremonies), Organizational Psychologist (team coordination, cognitive load), Database Migration Specialist (schema evolution, backwards compat) |

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | Data Modeling Specialist (YAML schemas, document formats) | Core | 0.90 | 🧁 |
| 🧁 Cupcake | DevTools Architect (CLI & Workflow Systems) | Core | 0.95 | 🧁 |
| 🧁 Scone | API Integration Specialist (REST, webhooks, sync protocols) | Adjacent | 0.75 | 🧁 |
| 🧁 Eclair | Developer Experience Advocate (ergonomics, onboarding) | Adjacent | 0.78 | 🧁 |
| 🧁 Donut | Git Workflow Specialist (monorepos, worktrees, conventions) | Adjacent | 0.65 | 🧁 |
| 🧁 Brioche | Security & Credential Management Engineer | Adjacent | 0.68 | 🧁 |
| 🧁 Croissant | Organizational Psychologist (team coordination, cognitive load) | Wildcard | 0.45 | 🧁 |
| 🧁 Macaron | Product Manager (roadmap planning, sprint ceremonies) | Wildcard | 0.55 | 🧁 |

## Alignment Scoreboard

| Round | W | C | T | R | Score | S | Open T | New P | Velocity | Converge % |
|-------|---|---|---|---|-------|---|--------|-------|----------|------------|
| 0     | 21| 16| 20| 8 | 65    |65 | 8      | 16    | 24       | 0%         |
| 1     | 25| 21| 22| 26| 94    |159| 3      | 8     | 11       | 0%         |
| 2     | 18| 18| 18| 18| 72    |231| 0      | 0     | 0        | 100%       |

### Per-Agent (Cumulative)

| Agent | R0 | R1 | Total |
|-------|----|----|-------|
| 🧁 Muffin | 9 | 12 | **21** |
| 🧁 Cupcake | 10 | 14 | **24** |
| 🧁 Scone | 7 | 11 | **18** |
| 🧁 Eclair | 8 | 11 | **19** |
| 🧁 Donut | 8 | 12 | **20** |
| 🧁 Brioche | 7 | 11 | **18** |
| 🧁 Croissant | 8 | 11 | **19** |
| 🧁 Macaron | 8 | 12 | **20** |

**Total ALIGNMENT**: 231 (CONVERGED)

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01 | 🧁 Muffin | Front matter schema as abstraction boundary (DocSchema trait) | 0 |
| P02 | 🧁 Muffin | depends_on demands two-pass sync with deferred linking | 0 |
| P03 | 🧁 Cupcake | Extract DocSource trait, not a second sync mode | 0 |
| P04 | 🧁 Cupcake | Writeback creates git-commit side effect needing explicit policy | 0 |
| P05 | 🧁 Scone | Writeback creates bidirectional coupling; use separate mapping file | 0 |
| P06 | 🧁 Scone | depends_on needs two-pass sync with deterministic ordering | 0 |
| P07 | 🧁 Eclair | Zero-config must mean zero-surprise (first-sync confirmation) | 0 |
| P08 | 🧁 Eclair | Writeback as onboarding contract (jira_url not just key) | 0 |
| P09 | 🧁 Donut | Writeback must be dedicated commit type with conventional prefix | 0 |
| P10 | 🧁 Donut | Cross-repo coordination via PM repo as rendezvous point | 0 |
| P11 | 🧁 Brioche | Credential isolation by domain; .env.local security concerns | 0 |
| P12 | 🧁 Brioche | jira.toml as trust anchor needing integrity verification | 0 |
| P13 | 🧁 Croissant | YAML front matter is hidden onboarding cliff | 0 |
| P14 | 🧁 Croissant | Jira-first teammates need drift detection read-back loop | 0 |
| P15 | 🧁 Macaron | Sprint/release need read-back validation (project then verify) | 0 |
| P16 | 🧁 Macaron | PM repo should own dependency DAG with validation | 0 |

## Tensions Tracker

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T01 | Writeback: inline `jira_url` in front matter | **Resolved** | R0 | R1 |
| T02 | Writeback atomicity: batch commit after all API calls | **Resolved** | R0 | R1 |
| T03 | depends_on: single field, default Blocks, per-edge override | **Resolved** | R0 | R2 |
| T04 | First-sync: three-state (dry-run → confirm → implicit) | **Resolved** | R0 | R1 |
| T05 | Read-back: DriftDetector, scoped authority (status+sprint) | **Resolved** | R0 | R1 |
| T06 | .env.local: not a credential tier, pre-flight check | **Resolved** | R0 | R1 |
| T07 | CLI/TUI: git-native MVP, `blue status` post-MVP | **Resolved** | R0 | R2 |
| T08 | jira.toml: domain pinning, mandatory pre-sync gate | **Resolved** | R0 | R1 |
| T09 | Lock-ref: push access assumption, delegation future work | **Deferred** | R1 | — |

## Round 0: Opening Arguments

> Full agent responses: `.blue/dialogues/2026-03-12T2026Z-pm-repo-sync-architecture/round-0/*.md`

### 💙 Judge Summary

**Strong convergence** on two foundational points:

1. **DocSource/DocSchema trait abstraction** (Muffin + Cupcake): No "two modes" — extract a trait at the discovery/parsing/writeback boundary yielding uniform SyncItems. IssueTracker trait and core sync logic stay untouched. A third source shape becomes a trait impl, not a feature flag.

2. **Two-pass sync for depends_on** (Muffin + Scone + Macaron): Create all issues first (collecting id-to-jira_key mapping), then resolve depends_on links in a second pass.

**Key insights**: First-sync safety gate requiring `--confirm` (Eclair). Writeback commits need conventional prefix for auto-rebase (Donut). Drift detection must be first-class, always-on (Croissant + Macaron). jira.toml domain verification as pre-sync gate (Brioche). DAG validation for impossible sprint orderings (Macaron).

**Biggest open split**: T01 — writeback location (inline vs mapping file). Load-bearing for every downstream decision.

### Round 1 Priorities
1. Resolve T01 (writeback location)
2. Refine T05 (what does "read-back" look like?)
3. Address T02 (writeback atomicity/concurrency)
4. Begin converging on DocSource trait shape

## Round 1: Convergence

> Full agent responses: `.blue/dialogues/2026-03-12T2026Z-pm-repo-sync-architecture/round-1/*.md`

### 💙 Judge Summary

**5 of 8 tensions resolved.** Velocity dropped from 24 → 11.

**T01 Resolved — Inline `jira_url`**: Panel converged. Even Scone and Brioche (who advocated mapping file) conceded inline is needed for DX. Eclair's "writeback is the onboarding contract" was the winning argument. `.gitattributes` merge driver handles conflicts. Batch commit after all API calls.

**T05 Resolved — DriftDetector, not bidirectional sync**: Post-sync read-only validation producing typed DriftReport. Three modes: `--verify`, `--check` (CI gate), `--check-drift`. Croissant scopes Jira-authoritative read-back to status + sprint only.

**T02 Resolved — Batch writeback**: All WritebackOps collected, applied only after all Jira API calls succeed, single atomic commit.

**T04 Resolved — Three-state first-sync**: dry-run default → --confirm → implicit. Presence of `jira_url` = past first-sync.

**T06/T08 Resolved — Security**: .env.local not a tier (pre-flight check). Domain pinning mandatory.

**DocSource trait converging**:
```rust
trait DocSource {
    fn discover(&self) -> Vec<SyncItem>;
    fn resolve_links(&self, mapping: &IdMap) -> Vec<LinkRequest>;
    fn writeback(&self, results: &SyncResults) -> WritebackSet;
}
```

**Remaining**: T03 (link type semantics — minor), T07 (CLI/TUI gap — design scope question), T09 (lock-ref access — future work).

### Round 2 Focus
1. Close T03 (depends_on schema: inline override vs separate field)
2. Scope T07 (CLI commands for MVP vs future)
3. Acknowledge T09 as future work
4. Signal convergence

## Round 2: Convergence (FINAL)

> Full agent responses: `.blue/dialogues/2026-03-12T2026Z-pm-repo-sync-architecture/round-2/*.md`
> Panel: Muffin, Cupcake, Scone, Eclair, Croissant, Macaron (Donut/Brioche rotated out — their tensions resolved)

### 💙 Judge Summary

**6/6 experts signal CONVERGE. Velocity = 0. All 9 tensions resolved or deferred.**

**T03 Resolved**: Single `depends_on` field, default `Blocks`, per-edge `link_type` override. DAG cycle detection scoped to Blocks edges only. Macaron conceded separate `relates_to` field was unnecessary given per-edge override covers it.

**T07 Resolved**: MVP targets git-native solo founders. `blue status` (read-only) is minimal MVP DX. CLI mutation commands (`blue board`, `blue move`) documented as post-MVP future work. DriftDetector's scoped read-back (status + sprint) bridges the gap.

**T09 Deferred**: Push access assumption documented. Delegation model is future work.

---

## Final Aligned Architecture

### DocSource Trait
```rust
trait DocSource {
    fn discover(&self) -> Vec<SyncItem>;
    fn resolve_links(&self, mapping: &IdMap) -> Vec<LinkRequest>;
    fn writeback(&self, results: &SyncResults) -> WritebackSet;
}
```

### Sync Engine Flow
`discover() → push() → resolve_links() → push_links() → verify() → writeback()`

### Key Decisions
1. **Writeback**: Inline `jira_url` in front matter, `.gitattributes` merge driver, batch atomic commit
2. **DriftDetector**: Separate post-sync component. Scoped read-back: status + sprint Jira-authoritative. Three modes: `--verify`, `--check`, `--check-drift`
3. **First-sync safety**: Three-state (dry-run default → `--confirm` → implicit)
4. **depends_on**: Single field, default Blocks, per-edge `link_type` override, DAG validation on Blocks only
5. **Security**: jira.toml domain pinning (tracked hash), .env.local not a credential tier
6. **Scope**: Git-native MVP. CLI mutation commands post-MVP. T09 lock-ref deferred.

---

*"Right then. Let's get to it."*

— Blue

