# Round 2 Summary — Judge's Synthesis

## What Round 2 Resolved

Round 2 resolved all 7 open tensions from Round 1 with zero or one new tension. The panel has converged on a concrete action plan.

### The Converged Position

**Do NOT supersede RFC 0058 with a hybrid architecture.** Instead, amend the implementation sequence:

1. **Phase A — Build the trait boundary first (RFC 0053)**
   - Extract `DialogueStore` trait from 32 existing `&Connection` functions
   - Implement `SqliteDialogueStore` as the reference implementation
   - Convert `dialogue.rs` handler call sites
   - **Exit gate:** Zero bare `pub fn ...(conn: &Connection)` signatures in `alignment_db.rs`
   - **Forced decisions:** Which methods survive (CROISSANT R1-T01), domain-shaped trait (STRUDEL R1-T01)

2. **Phase B — Define portable encryption envelope**
   - AAD = `sha256(canonical_entity_address)` where canonical address is backend-independent
   - E.g., `dialogue:{id}/entity:{type}/{subkey}` — same string regardless of DynamoDB pk/sk or SQL columns
   - **Exit gate:** Envelope spec passes round-trip test across both backends
   - **Must happen before first encrypted write** (TARTLET R1-T01)

3. **Phase C — Implement DynamoDB behind the trait (RFC 0058)**
   - `DynamoDialogueStore` implements the stable trait
   - Full-partition load + in-memory graph assembly (no verdict denormalization)
   - DynamoDB Local integration tests pass the same generic test suite as SQLite
   - **Exit gate:** Dual-implementation CI passes

### Key Design Decisions Converged

| Decision | Resolution | Settled By |
|----------|-----------|------------|
| Hybrid architecture | **Rejected** | Near-unanimous (R0-R2) |
| Trait governance gate | **ADR + PartitionScoped marker trait** | Croissant |
| Trait shape | **Domain access patterns (partition-scoped CRUD)** | Croissant + Strudel |
| Verdict denormalization | **Eliminated** (in-memory assembly instead) | Cannoli + Muffin |
| Encryption portability | **Canonical entity address for AAD** | Tartlet |
| Implementation sequencing | **Trait first, envelope second, DynamoDB third** | Galette |
| Cross-partition queries | **Separate AnalyticsStore trait** | Croissant |

### One Remaining Disagreement

**Refs table design** — Strudel and Cannoli disagree on where edge data lives within the partition:
- **Strudel:** Entities already carry inline refs as JSON arrays. The refs table is a redundant SQLite artifact. Drop it.
- **Cannoli:** Keep refs as cleartext DynamoDB items. One item per edge, no encrypted payload to re-seal on mutation. Cheapest representation.

Both positions are compatible with the agreed read pattern (full-partition load + in-memory assembly). This is a schema implementation detail, not an architectural tension.

### Strudel's Minor New Tension

**Graph assembly belongs in a shared library, not a trait method.** If the trait exposes `fn get_verdict_with_context(...)`, every backend must independently implement graph assembly. If instead the trait returns raw entities and a shared library assembles the graph, the trait stays thin. Croissant's response addresses this: trait returns domain types, assembly logic lives in the shared layer above the trait.

## Convergence Assessment

| Metric | R0 | R1 | R2 |
|--------|----|----|-----|
| Open Tensions | 10 | 7 | 1 (minor) |
| New Perspectives | 20 | 9 | 3 |
| Velocity | 30 | 16 | 4 |
| Converge % | 0% | 0% | ~83% |

Velocity has dropped from 30 → 16 → 4. Five of six experts raised zero new tensions. The dialogue is ready for a convergence round.
