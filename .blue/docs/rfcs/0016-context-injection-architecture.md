# RFC 0016: Context Injection Architecture

| | |
|---|---|
| **Status** | Draft |
| **Created** | 2026-01-25 |
| **Source** | Alignment Dialogue (12 experts, 95% convergence) |

---

## Summary

Unified architecture for injecting knowledge into Claude's context, consolidating session hooks, MCP resources, and knowledge files into a manifest-driven system with three cognitive tiers.

## Motivation

Blue currently has multiple context injection mechanisms that evolved organically:
- SessionStart hooks inject `knowledge/*.md` files
- Project-specific `.blue/workflow.md` auto-injects
- MCP Resources (`blue://rfc/*`) designed but not implemented
- Worktree context via hooks

This creates a "scattered hydration" pattern with no unified model. Users cannot audit what context Claude receives, and the system doesn't scale.

## Principles

1. **Context flows through explicit boundaries** - System/project/session tiers with clear ownership
2. **Artifacts are the only learning mechanism** - Sessions don't learn; projects do through explicit artifacts
3. **Manifest declares intent; visibility reveals reality** - Single source of truth with audit capability
4. **Push for bootstrap, pull for depth** - Hooks provide essentials, MCP Resources provide enrichment

## Design

### Three-Tier Model

| Tier | Name | Injection | Content | Budget |
|------|------|-----------|---------|--------|
| 1 | **Identity** | Always (SessionStart) | ADRs, voice patterns | ~500 tokens |
| 2 | **Workflow** | Activity-triggered | Current RFC, active tasks | ~2000 tokens |
| 3 | **Reference** | On-demand (MCP) | Full docs, dialogues | ~4000 tokens |

Cognitive framing from Strudel: Identity = "who am I", Workflow = "what should I do", Reference = "how does this work".

### Manifest Format

```yaml
# .blue/context.manifest.yaml
version: 1
generated_at: 2026-01-25T12:00:00Z
source_commit: abc123

identity:
  - uri: blue://docs/adrs/
  - uri: blue://context/voice
  max_tokens: 500

workflow:
  sources:
    - uri: blue://state/current-rfc
    - uri: blue://docs/rfcs/{active}
  refresh_triggers:
    - on_rfc_change
    - every_10_turns
  max_tokens: 2000

reference:
  graph: blue://context/relevance
  max_tokens: 4000
  staleness_days: 30

plugins:
  - uri: blue://jira/
    salience_triggers:
      - commit_msg_pattern: "^[A-Z]+-\\d+"
```

### URI Addressing

| Pattern | Description |
|---------|-------------|
| `blue://docs/{type}/` | Document collections (adrs, rfcs, spikes) |
| `blue://docs/{type}/{id}` | Specific document |
| `blue://context/{scope}` | Injection bundles (voice, relevance) |
| `blue://state/{entity}` | Live state (current-rfc, active-tasks) |
| `blue://{plugin}/` | Plugin-provided context |

### Injection Flow

```
SessionStart Hook
       │
       ▼
┌──────────────────┐
│ Read manifest    │
│ Resolve Tier 1   │
│ Declare Tier 2   │
└──────────────────┘
       │
       ▼
┌──────────────────┐
│ MCP Resource     │
│ Resolver         │
│ (lazy Tier 2/3)  │
└──────────────────┘
       │
       ▼
┌──────────────────┐
│ Claude Context   │
└──────────────────┘
```

Hooks push **URIs** (references), MCP pulls **content**. This resolves the layering violation concern.

### Visibility Commands

```bash
# Quick summary
blue context
# → Identity: 3 sources (487 tokens) | Workflow: 2 sources (1.2k tokens)

# Full manifest view
blue context show
# → Shows manifest with injection status per source

# Verbose audit
blue context show --verbose
# → Complete audit trail with timestamps and hashes
```

MCP equivalent: `blue_context_status` tool.

### Security Model

1. **Checksum verification** - Manifest changes are detectable
2. **Scope boundaries** - Cannot reference outside project root without `allow_external: true`
3. **Sensitive pattern deny-list** - `.env`, `*credentials*`, `*secret*` blocked by default
4. **Audit logging** - Every injection logged: timestamp, source, content_hash, session_id

### Generated Artifacts

Condensed knowledge files (e.g., `knowledge/blue-adrs.md`) must be generated, not hand-edited:

```yaml
# Header in generated file
# Generated: 2026-01-25T12:00:00Z
# Source: .blue/docs/adrs/*.md
# Commit: abc123
# Regenerate: blue knowledge build
```

Build step updates manifest atomically with artifact regeneration.

### Plugin Architecture

Plugins register URI schemes and salience triggers:

```yaml
plugins:
  - uri: blue://jira/
    provides: [ticket-context, acceptance-criteria]
    salience_triggers:
      - commit_msg_pattern: "^[A-Z]+-\\d+"
      - file_annotation: "@jira"
```

Orchestrator handles budget allocation based on active triggers.

## Implementation

### Phase 1: Foundation
- [ ] Define manifest schema (JSON Schema)
- [ ] Implement `blue context show` command
- [ ] Refactor `hooks/session-start` to read manifest
- [ ] Add audit logging

### Phase 2: MCP Resources
- [ ] Implement `resources/list` handler
- [ ] Implement `resources/read` handler for `blue://` URIs
- [ ] Add URI resolution for all document types

### Phase 3: Dynamic Activation
- [ ] Implement refresh triggers
- [ ] Add relevance graph computation
- [ ] Implement staleness detection and warnings

## Consequences

### Positive
- Single source of truth for injection policy
- Auditable, debuggable context delivery
- Graceful degradation if MCP fails
- Plugin extensibility without forking
- Token budget management

### Negative
- Additional complexity vs. simple file concatenation
- Requires manifest maintenance
- MCP Resource implementation effort

### Neutral
- Shifts context curation from implicit to explicit

## Related

- [Spike: Context Injection Mechanisms](../spikes/2025-01-25-context-injection-mechanisms.md)
- [Spike: ADR Porting Inventory](../spikes/2025-01-25-coherence-adr-porting-inventory.md)
- [Dialogue: RFC Consolidation](../dialogues/rfc-context-injection-consolidation.dialogue.md)
- ADR 0005: Single Source
- ADR 0004: Evidence

---

*Drafted from alignment dialogue with 12 domain experts achieving 95% convergence.*
