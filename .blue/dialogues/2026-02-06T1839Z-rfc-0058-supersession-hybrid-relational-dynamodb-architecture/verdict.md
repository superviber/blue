# Final Verdict — RFC 0058 Supersession Dialogue

## 100% CONVERGENCE ACHIEVED

```
Final Dialogue Summary
┌───────────────────────┬──────────────────────────────┐
│        Metric         │           Value               │
├───────────────────────┼──────────────────────────────┤
│ Rounds                │ 4 (R0-R3)                    │
├───────────────────────┼──────────────────────────────┤
│ Total ALIGNMENT       │ 415                          │
│   (W:135 C:104 T:92 R:84)                            │
├───────────────────────┼──────────────────────────────┤
│ Experts Consulted     │ 12 unique                    │
├───────────────────────┼──────────────────────────────┤
│ Tensions Resolved     │ 17/17                        │
├───────────────────────┼──────────────────────────────┤
│ Final Velocity        │ 0                            │
└───────────────────────┴──────────────────────────────┘
```

## Answer to the Question

**Should RFC 0058 (Encrypted DynamoDB Storage) be superseded by a hybrid architecture that uses a scalable relational database alongside DynamoDB?**

### NO.

RFC 0058 should NOT be superseded by a hybrid architecture. Instead, the implementation sequence should be amended to build the trait boundary (RFC 0053) first, define a portable encryption envelope second, and implement DynamoDB behind the stable trait third.

## Converged Decisions

```
┌────────────────────────────┬──────────────────────────────────────────────────────────┐
│         Topic              │                        Decision                           │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Hybrid architecture        │ REJECTED — unanimous across all 12 experts               │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Implementation sequence    │ Three-phase gate: Trait → Envelope → DynamoDB             │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Trait governance           │ ADR + PartitionScoped marker trait + AnalyticsStore split  │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Trait shape                │ Domain access patterns (partition-scoped CRUD)             │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Verdict denormalization    │ ELIMINATED — in-memory assembly from full-partition load   │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Encryption portability     │ Canonical entity address AAD: dialogue:{id}/entity:{t}/{s} │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Graph assembly             │ Shared library above trait, not a trait method              │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Refs table design          │ Deferred to Phase C (both designs equivalent under         │
│                            │ full-partition-load + in-memory assembly)                  │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Cross-partition queries    │ Separate AnalyticsStore trait (optional per backend)        │
├────────────────────────────┼──────────────────────────────────────────────────────────┤
│ Hash chain boundary        │ Phase C migration protocol detail, not a blocker           │
└────────────────────────────┴──────────────────────────────────────────────────────────┘
```

## Three-Phase Implementation Sequence

### Phase A — Build the Trait Boundary (RFC 0053)
- Extract `DialogueStore` trait from 32 existing `&Connection` functions
- Implement `SqliteDialogueStore` as reference implementation
- Convert all `dialogue.rs` handler call sites
- Strip verdict denormalization arrays from domain model
- **Exit gate:** Zero bare `pub fn ...(conn: &Connection)` signatures in `alignment_db.rs`
- **Governance:** `PartitionScoped` marker trait on return types; methods that violate partition scope go to `AnalyticsStore`

### Phase B — Define Portable Encryption Envelope
- AAD = `sha256(canonical_entity_address)` where canonical address = `dialogue:{id}/entity:{type}/{subkey}`
- Backend-independent by construction — same string regardless of physical key structure
- **Exit gate:** Envelope spec passes round-trip test across both SQLite and DynamoDB backends
- **Critical:** Must ship before first encrypted write

### Phase C — Implement DynamoDB Behind the Trait (RFC 0058)
- `DynamoDialogueStore` implements the stable `DialogueStore` trait
- Full-partition load + in-memory graph assembly (no verdict denormalization)
- Refs table design resolved empirically (inline vs cleartext items)
- Hash chain boundary event specified in migration protocol
- DynamoDB Local integration tests pass the same generic test suite as SQLite
- **Exit gate:** Dual-implementation CI passes

## Resolved Tensions

```
┌──────────────────────┬───────────────────────────────────────────────────────────────┐
│         ID           │                        Resolution                              │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ MUFFIN-T01 (R0)      │ Evolved → eliminated: denormalized arrays removed entirely    │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ SCONE-T01 (R0)       │ Narrowed: only refs table is genuine mismatch                 │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ STRUDEL-T01 (R0)     │ Narrowed → resolved: refs-only redesign, Phase C detail       │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ ECLAIR-T01 (R0)      │ Narrowed: parity is code-path, not DynamoDB-specific          │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ CROISSANT-T01 (R0)   │ Refined → resolved: ADR + PartitionScoped marker trait        │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ BRIOCHE-T01 (R0)     │ Resolved: YAGNI — analytics is speculative                    │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ CANNOLI-T01 (R0)     │ Resolved: serverless is implicit baseline                     │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ MACARON-T01 (R0)     │ Resolved: trait's single-active-backend = rollback plan       │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ CUPCAKE-T01 (R0)     │ Resolved: AnalyticsStore trait for cross-partition queries     │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ DONUT-T01 (R0)       │ Resolved: cost is negligible at current scale                 │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ CROISSANT R1-T01     │ Resolved: ADR + PartitionScoped + dual-impl CI                │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ STRUDEL R1-T01       │ Resolved: trait shaped by domain, workarounds behind impl      │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ MUFFIN R1-T01        │ Resolved: verdicts immutable, denormalization eliminated       │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ ECLAIR R1-T01        │ Resolved: denormalized fields redundant, removed               │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ MACARON R1-T01       │ Resolved: cost negligible, denormalization eliminated          │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ TARTLET R1-T01       │ Resolved: canonical entity address AAD in Phase B              │
├──────────────────────┼───────────────────────────────────────────────────────────────┤
│ GALETTE R1-T01       │ Resolved: trait-first sequencing adopted                      │
└──────────────────────┴───────────────────────────────────────────────────────────────┘
```

## Key Insights (by contribution)

1. **Strudel** (R0): "The schema is the problem, not the storage engine" — reframed the entire debate away from DynamoDB-vs-PostgreSQL
2. **Croissant** (R0-R2): Trait abstraction as resolution mechanism → ADR + PartitionScoped marker → 91% of functions already comply
3. **Galette** (R1): Prerequisite inversion — the trait doesn't exist in code yet; building it IS the design decision
4. **Cannoli** (R1-R2): Full-partition load + in-memory assembly eliminates denormalization entirely
5. **Tartlet** (R1-R2): Canonical entity address for AAD portability — one-line spec, prevents re-encryption catastrophe
6. **Muffin** (R2): Self-corrected on verdict consistency — verdicts are immutable snapshots, not live views
7. **Strudel** (R1): Narrowed from "redesign everything" to "redesign only the refs table"

## What This Means for RFC 0058

RFC 0058 is **NOT superseded**. It proceeds with three amendments:

1. **Sequencing**: Build RFC 0053's trait boundary first (Phase A), then encryption envelope (Phase B), then DynamoDB implementation (Phase C)
2. **Schema**: Eliminate verdict denormalization arrays; use full-partition load + in-memory assembly instead
3. **Encryption**: Use canonical entity address `dialogue:{id}/entity:{type}/{subkey}` for AAD binding, not DynamoDB pk||sk

---

All 6 experts signaled [MOVE:CONVERGE]. Velocity = 0. 17/17 tensions resolved.

🧁🧁🧁💙🧁🧁🧁
