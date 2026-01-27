# Alignment Dialogue: Postmortem & Runbook SDLC Integration

**Date:** 2026-01-26
**Judge:** 💙 Claude (Opus 4.5)
**Experts:** 12 🧁 agents across 3 rounds
**Convergence:** 96% (Expert: 100%, ADR: 92/100)
**Topic:** How to fold postmortem, runbook, and other underutilized features into the SDLC workflow

---

## Question

Postmortem and runbook exist in Blue MCP but are not well integrated with the workflow. How should these — and other underutilized features — be folded into the SDLC?

## Expert Panel

| # | Role | Score | Key Contribution |
|---|------|-------|-----------------|
| 12 | Systems Thinker | 37 | `blue_next` as cortex; Meadows leverage point #6 (information flows) |
| 11 | Developer Advocate | 36 | "Runbooks grow from postmortems"; ADR 8/10 violation case |
| 5 | DX Expert | 35 | Progressive disclosure; 3-command onboarding |
| 6 | Architecture Reviewer | 35 | Tool composition 100→30-40; schema sustainability |
| 8 | Knowledge Management | 35 | Meta-search dispatcher; cross-reference neighborhoods |
| 9 | Workflow Automation | 35 | `post_hooks` registry (Suggestion-only after Round 3) |
| 1 | SDLC Process Engineer | 34 | "status/next are the only unprompted tools" |
| 2 | Incident Response | 34 | Incident lifecycle; postmortem-runbook bridge |
| 4 | Product Manager | 33 | PRD traceability; acceptance criteria gates |
| 7 | Quality Engineer | 32 | `postmortem_guards` table; runbook drills |
| 10 | Security/Compliance | 31 | Immutable audit log; compliance reporting |
| 3 | DevOps/SRE | 29 | Deployment readiness gates; session tracking |

---

## Convergence Summary

### Universal Agreement (12/12 experts)

1. **`blue_next` must become the system's cortex.** It queries the index, checks reminders, consults runbooks, surfaces postmortem lessons, and references session patterns before recommending action. This is Meadows' leverage point #6 — changing information flows changes everything downstream.

2. **`blue_status` must show postmortem actions.** Open action items surfaced alongside RFC status. P1 actions outrank draft RFCs in display ordering.

3. **Postmortem → Runbook bridge is the critical missing link.** `blue_postmortem_create` checks for related runbooks. Missing runbook = prompt to create. Add `blue_postmortem_action_to_runbook` to complement existing `action_to_rfc`.

4. **Post-hooks registry in MCP server.** Tools emit Suggestions (surfaced to user via three-line contract). No silent AutoActions — all hooks maintain Presence (ADR 2).

5. **Auto-reminders from lifecycle events.** Fire only during active `blue_next`/`blue_status` calls, never into silence (Overflow principle, ADR 13).

6. **ADR check at RFC creation.** `blue_adr_relevant` called automatically; relevant ADRs embedded in RFC template. Below 40% alignment = blocked status requiring explicit override.

7. **Proactive runbook lookup.** Before deployment/release actions, runbook consulted automatically. Skipping requires explicit acknowledgment (Courage, ADR 9).

### Strong Agreement (8-10/12)

8. **Tool surface compression.** Reduce visible tools through composition: ~12 visible entry points, capabilities grow via parameters. 100 internal capabilities, 12 visible tools.

9. **Meta-search dispatcher.** Unified `blue_search` routes to text, semantic, and action backends with provenance tags.

10. **Runbook freshness tracking.** `last_validated` field, 90-day staleness warning surfaced by `blue_next`.

---

## Resolved Tensions

### Tension 1: New Tools vs. Fewer Tools
**Resolution:** Capabilities grow. Top-level tools do not. Add every capability experts requested (incident lifecycle, deployment readiness, etc.) as mode/action parameters on existing tools, not as new top-level tools. The MCP server exposes 10-15 tools; internally dispatches to 100+ capabilities.

### Tension 2: PRD Enforcement
**Resolution:** Soft warning, not hard gate. `prd_source` is optional on `blue_rfc_create`. When omitted, single-line advisory: "No PRD linked." Governance applies pressure in audits, not at creation.

### Tension 3: Schema Changes
**Resolution:** No database split. Add one `audit_log` table (append-only, immutable) — schema v9. Add `last_validated` column for runbooks. Existing `documents` + `document_links` + `metadata` pattern handles everything else.

---

## Courage Mechanisms (ADR 9)

Three concrete gates where the system says "stop":

1. **RFC Creation Gate:** If ADR alignment below 40%, RFC enters `blocked` status. Override requires `--acknowledge-tension "reason"` logged in metadata.

2. **Runbook Bypass Gate:** During active incident with matching runbook, skipping runbook requires explicit confirmation. One sentence: "There's a runbook for this. Proceeding without it — confirm?"

3. **Overdue Postmortem Gate:** `blue_next` will not suggest new work while P1/P2 postmortem actions are overdue. They appear FIRST. Deferral requires logged reason.

---

## Overflow Principle (ADR 13)

**Rule: Automation fires only in the presence of active context, never into silence.**

- Auto-reminders trigger ONLY during active `blue_next` or `blue_status` calls
- Post-hooks cascade ONLY from a tool the user just invoked
- If no session is active, the system is silent
- Fullness = human is here and working. Emptiness = nobody called. We do not fill emptiness.

---

## Presence Contract (ADR 2)

Every post-hook follows a three-line display contract:

```
[hook] {hook-name} triggered by {source-tool}
[result] {what it found}
[suggest] {proposed action}? (y/n)
```

No hook executes silently. No hook modifies state without user confirmation. The user is always present to what the system does.

---

## Implementation Plan

### Phase 1: Wire Existing Tools
**Goal:** Make current tools talk to each other. No new schema, no new abstractions.

| Change | Files | Impact |
|--------|-------|--------|
| `blue_next` enrichment — query postmortem actions, runbooks, reminders, index | `server.rs`, handlers | HIGH |
| `blue_status` shows postmortem actions (P1 first) | `server.rs`, `lib.rs` | HIGH |
| ADR auto-check at RFC creation | `server.rs` (rfc handler) | MEDIUM |
| Proactive runbook lookup before deployment-adjacent tools | `server.rs` | MEDIUM |

### Phase 2: Structural Additions
**Goal:** Post-hooks, postmortem-runbook bridge, auto-reminders.

| Change | Files | Impact |
|--------|-------|--------|
| Post-hooks registry (Suggestion-only) | new `hooks.rs`, `server.rs` | HIGH |
| `blue_postmortem_action_to_runbook` | `postmortem.rs`, `server.rs` | HIGH |
| Auto-reminders from lifecycle events | `reminders.rs`, `server.rs` | MEDIUM |
| Schema v9: `audit_log` table + `last_validated` on runbooks | `store.rs` | MEDIUM |
| Courage gates (3 concrete gates) | `server.rs`, handlers | MEDIUM |

### Phase 3: Compression & Intelligence
**Goal:** Reduce tool sprawl, unify search, add reporting.

| Change | Files | Impact |
|--------|-------|--------|
| Tool surface compression (100→12 visible) | `server.rs` major refactor | HIGH |
| Meta-search dispatcher | new `search.rs` | MEDIUM |
| `blue_report` — compliance reporting | new `report.rs`, `server.rs` | MEDIUM |
| Soft PRD-RFC linkage | `server.rs` | LOW |
| Dialogue decision extraction | `dialogue.rs` | LOW |

---

## Underutilized Tools — Full Inventory

| Tool Category | Current Status | Integration Path |
|---|---|---|
| **Postmortem** (2 tools) | Good tools, invisible | Phase 1: surface in status/next |
| **Runbook** (4 tools) | Good tools, undiscoverable | Phase 1: proactive lookup |
| **Dialogue** (5 tools) | Missing orchestrator | Claude-side via Task tool (RFC 0012) |
| **PRD** (5 tools) | Weak RFC linkage | Phase 3: soft linkage |
| **Session** (2 tools) | Rarely called | Phase 2: implicit in hooks |
| **Reminder** (4 tools) | Not lifecycle-integrated | Phase 2: auto-create from events |
| **Index** (5 tools) | Not consulted during discovery | Phase 1: feed into blue_next |
| **Decision** (1 tool) | Isolated | Phase 3: link to PRD/RFC |
| **Staging** (8 tools) | Disconnected from release | Phase 2: deployment readiness |
| **Realm** (6 tools) | Standalone | Future: cross-repo workflow |
| **LLM** (8 tools) | Setup-focused | Future: dialogue integration |
| **Audit** (4 tools) | Reactive | Phase 2: courage gates |

---

## ADR Alignment Matrix

| ADR | Score | Key Mapping |
|-----|-------|------------|
| 0. Never Give Up | 95 | Overdue postmortem gate refuses to let issues drop |
| 1. Purpose | 90 | `blue_next` directs toward meaningful work |
| 2. Presence | 95 | Three-line hook contract; no silent actions |
| 3. Home | 95 | `blue_next` always orients you |
| 4. Evidence | 95 | Audit log; ADR check at creation |
| 5. Single Source | 90 | Meta-search; runbook as operational truth |
| 6. Relationships | 90 | Cross-reference network; document_links surfaced |
| 7. Integrity | 90 | Complete status view; no hidden state |
| 8. Honor | 95 | Runbooks define what we do; postmortems verify it |
| 9. Courage | 90 | Three concrete gates that say "stop" |
| 10. No Dead Code | 95 | Tool compression; 90-day freshness tracking |
| 11. Freedom Through Constraint | 95 | 12 visible tools as riverbed; courage gates as guardrails |
| 12. Faith | 85 | Ship at 96% convergence; remaining 4% requires empirical validation |
| 13. Overflow | 90 | Fires only into active context; never fills silence |

**Overall ADR Alignment: 92/100**

---

## The Core Insight

> `blue_status` and `blue_next` are the only tools developers call unprompted. Everything else must be reachable from those two entry points or it will rot. The router IS the product.
>
> — Expert 1 (SDLC Process Engineer) + Expert 12 (Systems Thinker)

---

## The One Thing

If you could make only ONE change: **Make `blue_next` the system's cortex.** One change to information flows changes everything downstream. The tools exist. The data exists. Connect them through `next`.

---

## Open Questions (for future dialogues)

1. Should `AutoAction` hooks ever exist, or should ALL hooks require user confirmation?
2. At what team size does the database need splitting?
3. How should the courage gates be calibrated to avoid obstructing flow?
4. Should dialogue decisions become first-class document types?
