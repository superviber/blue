# Round 0 Summary — Judge's Synthesis

## Question
Should RFC 0058 (Encrypted DynamoDB Storage) be superseded by a hybrid architecture that pairs a scalable relational database with DynamoDB?

## Three Camps Emerged

### Camp 1: Pro-Relational (Muffin, Eclair)
RFC 0051's 14-entity typed directed graph with 8 edge types and M:N cross-references is a fundamental mismatch for DynamoDB's single-table design. Multi-hop traversals (verdict → recommendations → tensions → perspectives) require sequential round-trips with client-side assembly. The denormalized verdict fields (`tensions_resolved`, `key_evidence`, `key_claims`) are an admission that the graph cannot be traversed at query time. PostgreSQL with recursive CTEs and CHECK constraints handles this natively.

### Camp 2: Pro-DynamoDB Status Quo (Cupcake, Scone, Brioche, Cannoli, Macaron)
RFC 0058's entire value proposition is "the code you test is the code you ship." A hybrid reintroduces code-path divergence — two query languages, two consistency models, two failure modes. DynamoDB's serverless deployment model (connectionless, scales to zero) is uniquely suited to the encryption architecture. The operational blast radius of two databases squares the outage state space. Migration cost alone (another 9-week effort) dwarfs the graph-query pain.

### Camp 3: Synthesis & Reframing (Croissant, Strudel, Donut)
**Croissant** identified RFC 0053's trait abstraction as the resolution: the `DialogueStore` trait makes the backend a pluggable detail. Build DynamoDB now per RFC 0058, swap to PostgreSQL later if needed, without touching application code. But warned that hybrid architectures fracture the `EncryptedStore<S, K>` generic parameter space.

**Strudel** reframed the entire debate: the schema is the problem, not the storage engine. RFC 0051 was designed as a relational model first (SQLite tables) then force-fitted into DynamoDB. A system that started from DynamoDB's strengths would use immutable event streams, not a mutable relational graph. Also: the encryption layer parity gap (LocalFileKeyProvider vs KMS) is the harder problem that nobody is addressing.

**Donut** showed the cost crossover is undefined: below ~10K dialogues, DynamoDB's query cost premium is under $5/month, making hybrid operational complexity unjustified at current scale.

## Key Insight
The two most integrative contributions were Croissant's trait synthesis (RFC 0053 makes this a configuration decision, not an architectural one) and Strudel's schema reframing (the relational DNA of RFC 0051 is the root cause of the storage engine mismatch).

## Open Questions for Round 1
1. Does RFC 0053's trait boundary fully resolve the supersession question, or does it just defer it?
2. Is Strudel right that the schema should be redesigned around event streams?
3. What's the concrete rollback plan if any migration stalls mid-flight?
4. Is the target deployment serverless or containerized? This changes which weaknesses matter.
5. Can materialized views / additional GSIs solve the graph traversal problem within DynamoDB?
