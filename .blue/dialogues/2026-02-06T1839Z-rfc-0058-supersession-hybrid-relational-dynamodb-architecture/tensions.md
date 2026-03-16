# Accumulated Tensions

## RESOLVED

### MACARON-T01 (R0): No rollback plan if hybrid migration stalls
**Resolved in R1** by Macaron: RFC 0053's single-active-backend factory pattern provides the rollback mechanism — revert the config, keep SQLite as fallback, no split-brain.

### BRIOCHE-T01 (R0): Operational simplicity vs analytical query flexibility
**Resolved in R1** by Muffin's concession: Cross-dialogue analytics is speculative and violates YAGNI. Withdrawn as justification for hybrid.

### STRUDEL-T01 (R0): Event-sourced schema redesign vs sunk-cost commitment to RFC 0051
**Narrowed in R1** by Strudel: Full event-sourcing is disproportionate. Only the refs table needs redesign. The other 13 entity types map cleanly to DynamoDB. Subsumed by STRUDEL R1-T01.

## PARTIALLY ADDRESSED

### SCONE-T01 (R0): RFC 0051 schema strains single-table DynamoDB design
**Partially addressed** by Strudel's scoping: only the refs table is the genuine mismatch. The other 13 entity types fit DynamoDB's partition-scoped model.
**Status: NARROWED** — tension reduced to refs table specifically

### ECLAIR-T01 (R0): Local-prod parity vs query model fitness
**Partially addressed** by Cannoli: parity requires a single code path, not DynamoDB specifically. Eclair refined: parity doesn't require DynamoDB.
**Status: NARROWED** — reduced to deployment target question (serverless vs containerized)

### CANNOLI-T01 (R0): Serverless deployment constraints vs relational query expressiveness
**Partially addressed** by Cannoli R1: RFC 0058's architecture implicitly targets serverless. Panel should treat serverless as baseline.
**Status: PARTIALLY RESOLVED** — serverless baseline implied but not formally established

### DONUT-T01 (R0): Scale threshold undefined
**Partially addressed** by Tartlet's concession: at sub-10K dialogues, DynamoDB query cost premium is negligible. Cost is not a decision driver.
**Status: PARTIALLY RESOLVED** — cost argument neutralized, but volume assumptions unstated

### CUPCAKE-T01 (R0): Unbounded future query patterns vs locked access patterns
**Status: OPEN** — no R1 expert addressed this directly. Muffin withdrew the analytics argument but the general question remains.

### CROISSANT-T01 (R0): Trait completeness vs leaky abstraction risk
**Refined in R1** by Croissant into CROISSANT R1-T01. The abstract risk is now concrete: trait method governance.
**Status: SUPERSEDED** by CROISSANT R1-T01

### MUFFIN-T01 (R0): Denormalized verdict fields prove graph too expensive in DynamoDB
**Evolved in R1** into MUFFIN R1-T01 (consistency risk) and ECLAIR R1-T01 (domain model leakage).
**Status: SUPERSEDED** by MUFFIN R1-T01 and ECLAIR R1-T01

## OPEN — Round 1 Tensions

### Cluster D: Trait Governance & Shape (2 tensions)

#### CROISSANT R1-T01: Trait method governance gap
No RFC or ADR governs what methods may be added to `DialogueStore`. Without a review gate requiring each new method to demonstrate O(1)-partition-read implementability on DynamoDB, the trait will accumulate relational-only methods and silently make the DynamoDB backend unviable.
**Status: OPEN** — actionable, needs concrete gate proposal

#### STRUDEL R1-T01: Trait-shaped-by-workarounds vs trait-shaped-by-domain
Should the `DialogueStore` trait be designed from domain access patterns (backend-neutral) or from the DynamoDB implementation experience (which leaks GSI-shaped query methods)?
**Status: OPEN** — design philosophy question

### Cluster E: Denormalization Consequences (3 tensions)

#### MUFFIN R1-T01: Verdict denormalization consistency risk
If a recommendation is adopted by a verdict and later a new tension is linked to that recommendation, the verdict's pre-computed arrays are stale. RFC 0058 specifies no consistency mechanism.
**Status: OPEN** — correctness risk, needs solution

#### ECLAIR R1-T01: Denormalization artifacts leak through trait into domain model
DynamoDB-specific denormalized fields (verdict arrays, pre-computed ref chains) exist in the domain model, not behind the trait. A future backend swap would require maintaining dead denormalization logic or a second schema migration.
**Status: OPEN** — architectural concern

#### MACARON R1-T01: Denormalization cost unmeasured
Write-amplification concern on verdict denormalization is valid in theory but nobody has measured: with ~10-20 verdicts per dialogue, the amplification is bounded and small.
**Status: OPEN** — needs quantification

### Cluster F: Migration & Encryption (1 tension)

#### TARTLET R1-T01: Encryption envelope portability unspecified
Neither RFC 0053 nor RFC 0058 defines whether the AES-256-GCM envelope format, AAD binding (which includes pk||sk), and hash chain structure are backend-portable or implicitly coupled to DynamoDB's key structure.
**Status: OPEN** — critical gap, blocks future backend portability

### Cluster G: Prerequisites (1 tension)

#### GALETTE R1-T01: Prerequisite inversion
RFC 0053's trait boundary does not exist in the codebase. `alignment_db.rs` has 30+ direct `rusqlite::Connection` call sites with zero trait indirection. The supersession debate is premature until this foundation is built.
**Status: OPEN** — sequencing problem, affects all other decisions

## Tension Summary

| Category | R0 Open | R0 Resolved | R1 New | R1 Total Open |
|----------|---------|-------------|--------|----------------|
| Schema-Storage Mismatch | 5 | 1 narrowed, 2 superseded | — | 1 (narrowed) |
| Operational & Deployment | 3 | 2 resolved | — | 1 (partial) |
| Cost & Scale | 2 | — | — | 1 (partial) |
| Trait Governance & Shape | — | — | 2 | 2 |
| Denormalization Consequences | — | — | 3 | 3 |
| Migration & Encryption | — | — | 1 | 1 |
| Prerequisites | — | — | 1 | 1 |
| **Total** | **10** | **3 resolved, 2 superseded** | **7** | **7 open + 3 partial** |
