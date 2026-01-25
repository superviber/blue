# RFC 0017: Dynamic Context Activation

| | |
|---|---|
| **Status** | In-Progress |
| **Created** | 2026-01-25 |
| **Source** | Alignment Dialogue (12 experts, 95% convergence) |
| **Depends On** | RFC 0016 (Context Injection Architecture) |

---

## Summary

Implements Phase 3 of RFC 0016: refresh triggers, relevance graph computation, and staleness detection for dynamic context activation.

## Motivation

RFC 0016 established the three-tier context injection architecture with manifest-driven configuration. However, the current implementation is static:
- Triggers are defined but not activated
- `blue://context/relevance` returns empty
- `staleness_days` is declared but not enforced

Users experience context drift when working on RFCs that change state, documents that get updated, or across long sessions. The system needs to dynamically refresh context based on activity.

## Principles

1. **Staleness is content-based, not time-based** - A document unchanged for 30 days isn't stale; a document changed since injection is
2. **Predictability is agency** - Users should be able to predict what context is active without controlling every refresh
3. **ML is optimization, not feature** - Start with explicit relationships; add inference when data justifies it
4. **Event-sourced truth** - The audit log (`context_injections`) is the source of truth for staleness and refresh decisions

## Design

### Refresh Triggers

#### MVP: `on_rfc_change`

The only trigger implemented in Phase 1. Fires when:
- RFC status transitions (draft → accepted → in-progress → implemented)
- RFC content changes (hash differs from last injection)
- RFC tasks are completed

```rust
pub enum RefreshTrigger {
    OnRfcChange,           // MVP - implemented
    EveryNTurns(u32),      // Deferred - needs usage data
    OnToolCall(String),    // Deferred - needs pattern analysis
}
```

#### Trigger Evaluation

Piggyback on existing audit writes. When `resources/read` is called:

```rust
fn should_refresh(uri: &str, session_id: &str, store: &DocumentStore) -> bool {
    let last_injection = store.get_last_injection(session_id, uri);
    let current_hash = compute_content_hash(uri);

    match last_injection {
        None => true,  // Never injected
        Some(inj) => inj.content_hash != current_hash
    }
}
```

#### Rate Limiting

Prevent refresh storms with cooldown:

```rust
const REFRESH_COOLDOWN_SECS: u64 = 30;

fn is_refresh_allowed(session_id: &str, store: &DocumentStore) -> bool {
    let last_refresh = store.get_last_refresh_time(session_id);
    match last_refresh {
        None => true,
        Some(t) => Utc::now() - t > Duration::seconds(REFRESH_COOLDOWN_SECS)
    }
}
```

### Relevance Graph

#### Phased Implementation

| Phase | Scope | Trigger to Advance |
|-------|-------|-------------------|
| 0 | Explicit links only | MVP |
| 1 | Weighted by recency/access | After 30 days usage |
| 2 | Keyword expansion (TF-IDF) | <50% recall on explicit |
| 3 | Full ML (embeddings, co-access) | >1000 events AND <70% precision |

#### Phase 0: Explicit Links

Parse declared relationships from documents:

```markdown
<!-- In RFC body -->
References: ADR 0005, ADR 0007
```

Store in `relevance_edges` table:

```sql
CREATE TABLE relevance_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_uri TEXT NOT NULL,
    target_uri TEXT NOT NULL,
    edge_type TEXT NOT NULL,  -- 'explicit', 'keyword', 'learned'
    weight REAL DEFAULT 1.0,
    created_at TEXT NOT NULL,
    UNIQUE(source_uri, target_uri, edge_type)
);

CREATE INDEX idx_relevance_source ON relevance_edges(source_uri);
```

#### Resolving `blue://context/relevance`

Returns documents related to current workflow context:

```rust
pub fn resolve_relevance(state: &ProjectState) -> Vec<RelevantDocument> {
    let current_rfc = get_current_rfc(state);

    // Get explicit links from current RFC
    let edges = state.store.get_relevance_edges(&current_rfc.uri);

    // Sort by weight, limit to token budget
    edges.sort_by(|a, b| b.weight.cmp(&a.weight));

    let mut result = Vec::new();
    let mut tokens = 0;

    for edge in edges {
        let doc = resolve_uri(&edge.target_uri);
        if tokens + doc.tokens <= REFERENCE_BUDGET {
            result.push(doc);
            tokens += doc.tokens;
        }
    }

    result
}
```

### Staleness Detection

#### Content-Hash Based

Staleness = content changed since last injection:

```rust
pub struct StalenessCheck {
    pub uri: String,
    pub is_stale: bool,
    pub reason: StalenessReason,
    pub last_injected: Option<DateTime<Utc>>,
    pub current_hash: String,
    pub injected_hash: Option<String>,
}

pub enum StalenessReason {
    NeverInjected,
    ContentChanged,
    Fresh,
}
```

#### Tiered Thresholds

Different document types have different volatility:

| Doc Type | Refresh Policy | Rationale |
|----------|---------------|-----------|
| ADR | Session start only | Foundational beliefs rarely change |
| RFC (draft) | Every state change | Actively evolving |
| RFC (implemented) | On explicit request | Historical record |
| Spike | On completion | Time-boxed investigation |
| Dialogue | Never auto-refresh | Immutable record |

```rust
fn get_staleness_policy(doc_type: DocType, status: &str) -> RefreshPolicy {
    match (doc_type, status) {
        (DocType::Adr, _) => RefreshPolicy::SessionStart,
        (DocType::Rfc, "draft" | "in-progress") => RefreshPolicy::OnChange,
        (DocType::Rfc, _) => RefreshPolicy::OnRequest,
        (DocType::Spike, "active") => RefreshPolicy::OnChange,
        (DocType::Dialogue, _) => RefreshPolicy::Never,
        _ => RefreshPolicy::OnRequest,
    }
}
```

### Session Identity

Composite format for correlation and uniqueness:

```
{repo}-{realm}-{random12}
Example: blue-default-a7f3c9e2d1b4
```

- **Stable prefix** (`repo-realm`): Enables log correlation via SQL LIKE
- **Random suffix**: Cryptographically unique per MCP lifecycle

```rust
fn generate_session_id(repo: &str, realm: &str) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    let suffix: String = (0..12)
        .map(|_| CHARSET[rand::thread_rng().gen_range(0..CHARSET.len())] as char)
        .collect();
    format!("{}-{}-{}", repo, realm, suffix)
}
```

### Notification Model

Tier-based transparency:

| Tier | Behavior | User Notification |
|------|----------|-------------------|
| 1-2 (Identity/Workflow) | Automatic | Silent - user action is the notification |
| 3 (Reference) | Advisory | Inline: "Reading related tests..." |
| 4 (Expensive ops) | Consent | Prompt: "I can scan the full codebase..." |

**Honor Test**: If user asks "what context do you have?", the answer should match their intuition.

## Implementation

### Phase 1: MVP (This RFC)

- [x] Implement `on_rfc_change` trigger evaluation
- [x] Add content-hash staleness detection
- [x] Create `relevance_edges` table for explicit links
- [x] Update session ID generation
- [x] Add rate limiting (30s cooldown)
- [x] Implement `blue_context_status` MCP tool

### Phase 2: Weighted Relevance

- [ ] Add recency decay to edge weights
- [ ] Track access frequency per document
- [ ] Implement `blue context refresh` CLI command

### Phase 3: ML Integration (Gated)

Gate criteria:
- >1000 co-access events in audit log
- Explicit links precision >80%, recall <50%
- User feedback indicates "missing connections"

If gated:
- [ ] Co-access matrix factorization
- [ ] Embedding-based similarity
- [ ] Bandit learning for trigger timing

## Schema Changes

```sql
-- New table for relevance graph
CREATE TABLE relevance_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_uri TEXT NOT NULL,
    target_uri TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    weight REAL DEFAULT 1.0,
    created_at TEXT NOT NULL,
    UNIQUE(source_uri, target_uri, edge_type)
);

CREATE INDEX idx_relevance_source ON relevance_edges(source_uri);
CREATE INDEX idx_relevance_target ON relevance_edges(target_uri);

-- Add to documents table
ALTER TABLE documents ADD COLUMN content_hash TEXT;
ALTER TABLE documents ADD COLUMN last_injected_at TEXT;

-- Efficient staleness query index
CREATE INDEX idx_documents_staleness ON documents(
    doc_type,
    updated_at,
    last_injected_at
) WHERE deleted_at IS NULL;
```

## Consequences

### Positive
- Context stays fresh during active RFC work
- Explicit architectural traceability through relevance graph
- Graceful degradation: system works without ML
- Auditable refresh decisions via event log

### Negative
- Additional complexity in refresh evaluation
- Rate limiting may delay urgent context updates
- Explicit links require document authors to declare relationships

### Neutral
- ML features gated on data, may never ship if simple approach suffices

## Related

- [RFC 0016: Context Injection Architecture](./0016-context-injection-architecture.md)
- [Dialogue: Dynamic Context Activation](../dialogues/rfc-0017-dynamic-context-activation.dialogue.md)
- ADR 0004: Evidence
- ADR 0005: Single Source

---

*Drafted from alignment dialogue with 12 domain experts achieving 95% convergence.*
