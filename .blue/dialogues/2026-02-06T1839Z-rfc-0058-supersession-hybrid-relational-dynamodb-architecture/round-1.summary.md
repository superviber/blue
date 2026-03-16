# Round 1 Summary — Judge's Synthesis

## What Round 1 Answered

### Q1: Does RFC 0053's trait boundary fully resolve the supersession question?
**Partially.** Croissant showed the trait resolves supersession *conditionally* — only if trait methods are bounded to O(1) partition reads. Strudel countered that 9 weeks of DynamoDB development will shape the trait with DynamoDB workarounds, silently making the decision. Galette grounded both arguments: the trait doesn't exist in code yet (30+ direct `rusqlite::Connection` calls), making this a debate about a foundation that hasn't been built.

### Q2: Should the schema be redesigned around event streams?
**Narrowed.** Strudel scoped the claim from "redesign everything" to "redesign only the refs table." The other 13 entity types map cleanly to DynamoDB's partition model. Muffin argued event-sourcing strengthens the relational case (SQL views for materialization), while Cannoli countered that at actual volumes (~100 items per dialogue), full partition load + in-memory assembly makes the refs table's graph traversal trivially cheap.

### Q3: What's the concrete rollback plan?
**Resolved.** Macaron resolved MACARON-T01: RFC 0053's single-active-backend factory pattern provides rollback — revert config to SQLite, no split-brain. Tartlet raised a deeper version: the *encryption* rollback is the hard problem, because migrating encrypted, hash-chained data between backends breaks the audit chain at the migration boundary.

### Q4: Serverless or containerized?
**Implicitly answered.** Cannoli identified RFC 0058's architecture as implicitly serverless. Muffin conceded: if serverless, DynamoDB's operational model wins on deployment even as it loses on query expressiveness.

### Q5: Can GSIs solve graph traversal?
**No.** Muffin and Eclair showed that GSIs solve single-hop adjacency but not transitive closure (4-hop verdict assembly). Cannoli's counter: at actual volumes, you don't need the GSI — load the whole partition and traverse in memory.

## What Round 1 Surfaced

### The Prerequisite Problem (Galette)
The most consequential R1 finding: RFC 0053's trait boundary is the *mechanism* every camp depends on, but it doesn't exist in code. `alignment_db.rs` has 30+ direct `rusqlite::Connection` call sites. The entire supersession debate is premature until this foundation is built — and building it costs the same regardless of which backend wins.

### The Encryption Portability Gap (Tartlet)
The second critical finding: RFC 0058's AES-256-GCM envelope uses `pk||sk` in the AAD binding. If pk/sk are DynamoDB key structures, the encryption envelope is *implicitly coupled* to DynamoDB. No RFC specifies whether the envelope format is backend-portable.

### The Denormalization Calcification Risk (Eclair, Strudel)
The trait hides which database answers the query, but it does not hide which database *shaped the schema*. DynamoDB-specific denormalized fields (verdict arrays, pre-computed ref chains) live in the domain model, not behind the trait. A future backend swap would require maintaining dead denormalization logic.

## Emerging Resolution

The dialogue is converging toward a pragmatic sequence rather than an architectural winner:

1. **Build the trait first** — RFC 0053's `DialogueStore` trait must exist in code before any backend debate is load-bearing (Galette)
2. **Design the trait from domain patterns, not backend workarounds** — Strudel, Croissant
3. **Add a trait governance gate** — each new method must demonstrate O(1)-partition-read implementability (Croissant)
4. **Specify encryption envelope portability** — decouple AAD binding from DynamoDB key structure (Tartlet)
5. **Ship DynamoDB behind the trait** — RFC 0058 proceeds, but behind the abstraction (Macaron, Cannoli)
6. **Redesign refs table specifically** — per-entity adjacency lists vs separate refs table (Strudel)
7. **Let production data drive any future swap** — not speculation (Macaron, Cannoli)

## Open Questions for Round 2

1. Can the denormalization cluster (3 tensions) be resolved by Strudel's refs-table-only redesign + Cannoli's in-memory assembly pattern?
2. What does the trait governance gate concretely look like? An RFC? An ADR? A review checklist?
3. Is encryption envelope portability a pre-condition for shipping RFC 0058, or can it be addressed in a follow-up RFC?
4. Does the prerequisite inversion (GALETTE R1-T01) change the *conclusion* or just the *sequencing*?
