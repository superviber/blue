# RFC 0025: Blue Next Cortex

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source** | [Alignment Dialogue: Postmortem & Runbook SDLC Integration](../dialogues/2026-01-26-postmortem-runbook-sdlc-integration.dialogue.md) |

---

## Summary

`blue_next` and `blue_status` only query RFC state (draft/accepted/in-progress). They are blind to postmortem actions, runbook staleness, pending reminders, index state, and ADR relevance. Since these are the only two tools developers call unprompted, every other tool remains invisible and underutilized.

This RFC wires existing query functions into `blue_next` and `blue_status` with zero new schema, zero new tools — just richer responses from the two entry points that matter.

## Problem

The current `blue_next` handler (`server.rs:2440-2483`) queries only `status_summary()`, which checks four RFC categories: stalled, ready, drafts, active. It returns a single recommendation string. It knows nothing about:

- Open postmortem action items (especially P1/P2)
- Stale or relevant runbooks
- Overdue reminders
- Index/search state
- ADR alignment concerns

The current `blue_status` handler (`server.rs:2381-2438`) returns the same four RFC categories plus index drift. Postmortems, runbooks, reminders, and other document types are invisible.

Result: 88 of 100 tools are undiscoverable unless the user already knows they exist.

## Design

### Principle

> The router IS the product. — Alignment Dialogue, 12-expert consensus

`blue_next` becomes the system's cortex by querying all existing data sources before making recommendations. `blue_status` becomes the system's dashboard by showing all active work across document types.

No new tools. No new schema. No new tables. Every query function already exists.

### Change 1: `blue_status` — Full System View

**Current response:**
```json
{
  "active": [...rfcs...],
  "ready": [...rfcs...],
  "stalled": [...rfcs...],
  "drafts": [...rfcs...],
  "hint": "...",
  "index_drift": { ... }
}
```

**New response:**
```json
{
  "active": [...rfcs...],
  "ready": [...rfcs...],
  "stalled": [...rfcs...],
  "drafts": [...rfcs...],
  "postmortem_actions": [
    { "postmortem": "Database Outage", "action": "Add connection pooling", "severity": "P1", "status": "open", "due": "2026-02-01" }
  ],
  "runbooks": {
    "total": 3,
    "stale": [ { "title": "deploy-staging", "last_updated": "2025-09-15" } ]
  },
  "reminders": {
    "overdue": [ { "title": "Review spike results", "due_date": "2026-01-20" } ],
    "due_today": [ ... ]
  },
  "hint": "...",
  "index_drift": { ... }
}
```

**Implementation in `handle_status()`:**

1. Query postmortems with open actions:
   - `state.store.list_documents(DocType::Postmortem)` — get all postmortems
   - For each, read file, call `parse_all_actions(content)` — extract action items
   - Filter to actions where status column != "done"/"closed"
   - Extract severity from postmortem markdown (P1-P4)
   - Sort: P1 first, then P2, P3, P4

2. Query runbook staleness:
   - `state.store.list_documents(DocType::Runbook)` — get all runbooks
   - Check `updated_at` field — flag any >90 days old as stale
   - Return count + stale list

3. Query reminders:
   - `state.store.list_reminders(Some(ReminderStatus::Pending), false)` — pending, not snoozed
   - Partition into `overdue` (due_date < today) and `due_today`

4. Reconciliation: add `DocType::Postmortem` and `DocType::Runbook` to the drift check loop (currently only checks RFC, Spike, ADR, Decision).

### Change 2: `blue_next` — Informed Recommendations

**Current logic** (priority chain):
```
stalled RFC → ready RFC → draft RFC → active RFC → "nothing pressing"
```

**New logic** (priority chain):
```
1. Overdue P1/P2 postmortem actions → "Postmortem action overdue: '{action}'. This takes priority."
2. Overdue reminders → "Reminder overdue: '{title}'. Check on this."
3. Stalled RFCs → (existing behavior)
4. Ready RFCs → (existing behavior, enhanced)
5. Draft RFCs → (existing behavior)
6. Active RFCs → (existing behavior)
7. Stale runbooks → "Runbook '{title}' hasn't been reviewed in {N} days. Still current?"
8. Due-today reminders → "Reminder due today: '{title}'."
9. Nothing → "Nothing pressing. Good time to plan something new."
```

**Enhanced "ready RFC" step (priority 4):**
When recommending a ready RFC, also:
- Call `get_runbook_actions()` to check if relevant runbooks exist for the RFC's domain
- If match found: append `"Runbook '{runbook}' may be relevant. Run blue_runbook_lookup."`

**Implementation in `handle_next()`:**

Replace the single `if/else if` chain with a `recommendations: Vec<String>` builder:

```rust
fn handle_next(&mut self, _args: &Option<Value>) -> Result<Value, ServerError> {
    let state = self.ensure_state()?;
    let summary = state.status_summary();
    let mut recommendations: Vec<String> = Vec::new();

    // 1. Overdue P1/P2 postmortem actions
    let postmortem_actions = collect_open_postmortem_actions(state);
    for action in postmortem_actions.iter().filter(|a| a.severity <= 2 && a.is_overdue()) {
        recommendations.push(format!(
            "P{} postmortem action overdue: '{}'. This takes priority.",
            action.severity, action.description
        ));
    }

    // 2. Overdue reminders
    if let Ok(reminders) = state.store.list_reminders(Some(ReminderStatus::Pending), false) {
        for r in reminders.iter().filter(|r| r.is_overdue()) {
            recommendations.push(format!(
                "Reminder overdue: '{}'. Check on this.",
                r.title
            ));
        }
    }

    // 3-6. Existing RFC logic (stalled → ready → drafts → active)
    if recommendations.is_empty() {
        // ... existing logic, but push to recommendations vec
    }

    // 7. Stale runbooks (only if no higher-priority items)
    if recommendations.is_empty() {
        let stale = find_stale_runbooks(state, 90);
        for rb in stale {
            recommendations.push(format!(
                "Runbook '{}' hasn't been reviewed in {} days. Still current?",
                rb.title, rb.days_since_update
            ));
        }
    }

    // 8. Due-today reminders (informational, always append if present)
    if let Ok(reminders) = state.store.list_reminders(Some(ReminderStatus::Pending), false) {
        for r in reminders.iter().filter(|r| r.is_due_today()) {
            recommendations.push(format!("Reminder due today: '{}'.", r.title));
        }
    }

    // 9. Nothing
    if recommendations.is_empty() {
        recommendations.push("Nothing pressing. Good time to plan something new.".into());
    }

    Ok(json!({
        "recommendations": recommendations,
        "hint": summary.hint
    }))
}
```

### Change 3: ADR Auto-Check at RFC Creation

**In `handle_rfc_create()`:**

After creating the RFC document, call the ADR relevance logic:

```rust
// After RFC creation succeeds
let adr_results = adr::find_relevant_adrs(state, &problem_text);
if !adr_results.is_empty() {
    // Append to response
    response["relevant_adrs"] = json!(adr_results);
    response["adr_hint"] = json!("These ADRs may apply. Consider them in your design.");
}
```

This uses the existing `load_adr_summaries()` and keyword matching. No new code path — just calling existing functions from a new location.

### Change 4: Proactive Runbook Lookup

**In `handle_spike_complete()` and `handle_rfc_update_status()` (when status → "in-progress"):**

After the primary operation succeeds, check for relevant runbooks:

```rust
// After spike completion or RFC status change
let runbooks = state.store.list_documents(DocType::Runbook)?;
let relevant = find_relevant_runbooks(state, &title_or_context);
if !relevant.is_empty() {
    response["relevant_runbooks"] = json!(relevant);
    response["runbook_hint"] = json!(format!(
        "Runbook '{}' may be relevant. Use blue_runbook_lookup.",
        relevant[0].title
    ));
}
```

Uses existing `calculate_match_score()` from `runbook.rs`.

## Helper Functions Needed

Two small helper functions, both built from existing primitives:

### `collect_open_postmortem_actions(state) -> Vec<PostmortemAction>`

```rust
struct PostmortemAction {
    postmortem_title: String,
    description: String,
    severity: u8,       // 1-4
    status: String,
    due: Option<String>,
}
```

Iterates `list_documents(DocType::Postmortem)`, reads each file, calls existing `parse_all_actions()`, filters to non-closed items. Extracts severity from the postmortem markdown header.

### `find_stale_runbooks(state, threshold_days) -> Vec<StaleRunbook>`

```rust
struct StaleRunbook {
    title: String,
    days_since_update: u64,
}
```

Iterates `list_documents(DocType::Runbook)`, checks `updated_at` against current date, returns those exceeding threshold.

## Files Changed

| File | Change |
|------|--------|
| `crates/blue-mcp/src/server.rs` | `handle_status()` and `handle_next()` enrichment |
| `crates/blue-mcp/src/handlers/postmortem.rs` | Extract `collect_open_postmortem_actions()` as pub function; make `parse_all_actions()` pub |
| `crates/blue-mcp/src/handlers/runbook.rs` | Extract `find_stale_runbooks()` as pub function; make `calculate_match_score()` pub |
| `crates/blue-mcp/src/handlers/adr.rs` | Extract `find_relevant_adrs()` wrapper as pub function |

No new files. No schema changes. No new MCP tool definitions.

## What This Does NOT Do

- No post-hooks registry (Phase 2)
- No `blue_postmortem_action_to_runbook` bridge (Phase 2)
- No auto-reminder creation from lifecycle events (Phase 2)
- No courage gates / blocking behavior (Phase 2)
- No tool surface compression (Phase 3)
- No new schema tables (Phase 2)

## ADR Alignment

| ADR | How Served |
|-----|-----------|
| 0. Never Give Up | Overdue postmortem actions surface first — issues don't get dropped |
| 2. Presence | User sees full system state, not just RFCs |
| 3. Home | `blue_next` always orients you regardless of document type |
| 4. Evidence | Status shows evidence from all sources |
| 5. Single Source | One command (`blue_status`) shows everything |
| 6. Relationships | Cross-type awareness (RFC ↔ runbook ↔ postmortem) |
| 7. Integrity | Complete picture, no hidden state |
| 10. No Dead Code | Stale runbooks surfaced for review or deletion |

## Test Plan

- [ ] `blue_status` returns postmortem_actions array with open items
- [ ] `blue_status` returns stale runbooks (>90 days since update)
- [ ] `blue_status` returns overdue and due-today reminders
- [ ] `blue_status` reconciliation includes Postmortem and Runbook types
- [ ] `blue_next` prioritizes P1/P2 postmortem actions over RFC work
- [ ] `blue_next` surfaces overdue reminders before stalled RFCs
- [ ] `blue_next` mentions stale runbooks when nothing else is pressing
- [ ] `blue_next` appends due-today reminders as informational items
- [ ] `blue_rfc_create` returns relevant ADRs in response
- [ ] `blue_spike_complete` returns relevant runbooks in response
- [ ] `blue_rfc_update_status` to "in-progress" returns relevant runbooks
- [ ] Empty state: no postmortems/runbooks/reminders produces same output as before
- [ ] `cargo test` passes
- [ ] `cargo clippy` clean

---

*"The tools exist. The data exists. Connect them through next."*

— Blue
