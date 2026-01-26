# Alignment Dialogue: RFC 0017 Dynamic Context Activation

| | |
|---|---|
| **Date** | 2026-01-25 |
| **Experts** | 12 |
| **Rounds** | 2 |
| **Convergence** | 95% |
| **Status** | Complete |

---

## Summary

Deliberation on Phase 3 features for RFC 0016 Context Injection Architecture: refresh triggers, relevance graph computation, and staleness detection.

## Panel

| Expert | Domain | Key Contribution |
|--------|--------|------------------|
| Systems Architect | Architecture | Event-sourced design via audit log; hybrid lazy evaluation |
| Performance Engineer | Efficiency | Content-addressed cache; mtime-first staleness; token budget enforcement |
| UX Designer | Experience | Context breadcrumbs; predictable refresh moments; progressive disclosure |
| Data Scientist | Algorithms | PageRank relevance; co-access matrix; Bayesian staleness |
| Security Engineer | Security | Cryptographic hashes; rate limiting; plugin sandboxing |
| Distributed Systems | Consistency | Version vectors; bounded staleness; materialized relevance view |
| Cognitive Scientist | Cognition | Hysteresis for refresh; reasoning continuity; tier-appropriate thresholds |
| Product Manager | Prioritization | MVP = on_rfc_change only; defer ML; success metrics first |
| Database Engineer | Data Model | `relevance_edges` table; staleness indexes; efficient queries |
| ML Engineer | Learning | Graceful degradation ladder; bandit trigger learning; cold start mitigation |
| DevOps Engineer | Operations | Structured audit logging; refresh metrics; circuit breakers |
| Philosophy/Ethics | Ethics | Transparency imperative; user agency; coherence constraints |

## Round 1: Perspectives

### Strong Convergence (7+ experts)

1. **Event-sourced staleness** - Use content hash comparison from audit log, not calendar time
2. **`on_rfc_change` as MVP trigger** - Ship simplest valuable trigger first
3. **Materialized relevance graph** - Compute on write, cache aggressively
4. **Tiered staleness thresholds** - ADRs stable (session-start only), RFCs volatile (every state change)
5. **Rate limiting** - Circuit breakers to prevent refresh storms
6. **Transparent context changes** - Announce what context updated and why
7. **Version vectors** - Efficient O(1) staleness checks per document

### Tensions Identified

| ID | Tension | Positions |
|----|---------|-----------|
| T1 | ML Complexity | Data Scientist wants full ML stack vs Product Manager wants explicit links only |
| T2 | User Control | Philosophy wants advisory defaults vs UX wants automation that "just works" |
| T3 | Session Identity | Security wants crypto random vs Systems needs persistence across restarts |

## Round 2: Synthesis

### T1 Resolution: Phased Relevance (ML as optimization, not feature)

**Phase 0**: Explicit links only (declared relationships)
**Phase 1**: Weighted explicit (recency decay, access frequency)
**Phase 2**: Keyword expansion (TF-IDF suggestions)
**Phase 3**: ML gate review - proceed only if:
  - Explicit links have >80% precision but <50% recall
  - >1000 co-access events logged
  - Keyword suggestions clicked >15%
**Phase 4**: Full ML (if gated)

### T2 Resolution: Predictability is agency

| Tier | Control Model | Notification |
|------|---------------|--------------|
| 1-2 | Automatic | Silent (user action is the notification) |
| 3 | Advisory | Inline subtle ("Reading related tests...") |
| 4 | Explicit consent | Prompt as capability ("I can scan...want me to?") |

**Honor Test**: If user asks "what context do you have?", answer should match intuition.

### T3 Resolution: Composite session identity

```
Session ID: {repo}-{realm}-{random12}
Example: blue-default-a7f3c9e2d1b4
```

- Stable prefix enables log correlation via SQL LIKE queries
- Random suffix ensures global uniqueness and unpredictability
- No schema changes for MVP; optional `parent_session_id` for v2

## Scoreboard

| Expert | W | C | T | R | Total |
|--------|---|---|---|---|-------|
| Systems Architect | 9 | 9 | 8 | 9 | 35 |
| Cognitive Scientist | 9 | 8 | 9 | 9 | 35 |
| Database Engineer | 9 | 8 | 9 | 9 | 35 |
| Philosophy/Ethics | 9 | 8 | 9 | 9 | 35 |
| Distributed Systems | 9 | 9 | 8 | 8 | 34 |
| DevOps Engineer | 8 | 9 | 9 | 8 | 34 |
| Performance Engineer | 8 | 8 | 9 | 8 | 33 |
| UX Designer | 8 | 9 | 8 | 8 | 33 |
| Data Scientist | 9 | 7 | 8 | 9 | 33 |
| Security Engineer | 8 | 8 | 9 | 8 | 33 |
| Product Manager | 8 | 9 | 9 | 7 | 33 |
| ML Engineer | 8 | 7 | 8 | 8 | 31 |

## Recommendations for RFC 0017

1. **MVP Scope**: Implement `on_rfc_change` trigger with content-hash staleness
2. **Architecture**: Event-sourced from `context_injections`; pluggable relevance scorer
3. **Session Identity**: Composite `{repo}-{realm}-{random12}` format
4. **Notification Model**: Tier-based (automatic → advisory → consent)
5. **Relevance Graph**: Start with explicit links; gate ML on usage metrics
6. **Staleness**: Per-document-type thresholds; hash-based, not time-based
7. **Safety**: Rate limiting (max 1 refresh per 30s); circuit breakers

---

*Dialogue orchestrated by 💙 Judge with 12 domain experts across 2 rounds.*
