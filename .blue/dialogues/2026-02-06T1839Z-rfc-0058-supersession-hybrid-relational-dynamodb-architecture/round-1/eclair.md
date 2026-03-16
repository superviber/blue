[PERSPECTIVE P01: The trait boundary defers the decision but does not resolve the cost -- write-time denormalization is a tax paid now for a query model chosen prematurely]
Croissant is right that RFC 0053's `DialogueStore` trait makes the backend swappable, but the trait does not insulate application code from the denormalization decisions forced by DynamoDB. The verdict entity's pre-computed `tensions_resolved`, `key_evidence`, and `key_claims` arrays are not behind the trait -- they are in the domain model itself, baked into every write path. If the backend swaps to PostgreSQL later, those denormalized fields become dead weight maintained for consistency with a storage engine no longer in use. The trait hides which database answers the query; it does not hide which database shaped the schema. Strudel identified the schema as the root problem, but the practical consequence is narrower: it is specifically the write-amplification artifacts of DynamoDB that will calcify into the domain model if RFC 0058 ships first.

[PERSPECTIVE P02: Materialized views via GSIs solve adjacency but not transitive closure -- and the distinction matters for verdict construction]
Macaron and Cupcake argue that additional GSIs can solve reverse-ref lookups within DynamoDB. This is correct for single-hop queries (e.g., "what supports P0001?"), but verdict construction requires transitive closure: recommendation R01 addresses tension T01, which depends on perspective P03, which references evidence E02. Each hop is a separate DynamoDB query with its own latency and RCU cost, and the depth is unbounded by schema -- refs can chain arbitrarily. No number of GSIs collapses a variable-depth graph walk into a single query; only recursive CTEs or application-level BFS do, and the latter is exactly the client-side assembly pattern that motivated the denormalization in the first place.

[CONCESSION: Strudel is right that the schema's relational DNA is the deeper issue]
My round-0 argument framed this as DynamoDB-vs-PostgreSQL, but Strudel correctly identified that the RFC 0051 schema was designed relationally and then force-fitted into DynamoDB -- making the storage engine debate a symptom rather than the root cause.

[REFINEMENT: Local-prod parity does not require DynamoDB specifically]
Cannoli's serverless deployment argument (connectionless, scales-to-zero) is the strongest remaining justification for DynamoDB over a relational engine, but this only holds if the target deployment is Lambda/edge -- a deployment constraint the panel still has not established (CANNOLI-T01 remains open).

[TENSION T01: Denormalization artifacts will leak through the trait boundary into the domain model]
If RFC 0058 ships and the domain model absorbs DynamoDB-specific denormalized fields (verdict arrays, pre-computed ref chains), a future backend swap via RFC 0053's trait will require either maintaining dead denormalization logic or a second schema migration -- making the "pluggable backend" promise more expensive than it appears.

---
