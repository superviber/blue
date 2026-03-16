[PERSPECTIVE P01: Migration cost of supersession dwarfs the graph-query pain it solves]
RFC 0058 already has a 9-week phased rollout with 14 entity types, encryption envelope, and audit chain. Superseding it with a hybrid architecture means migrating not just the schema but also the encryption layer (client-side AES-256-GCM envelope must be re-integrated with a relational backend that has its own TDE/column encryption story), the audit hash chain (which binds to DynamoDB's append-only PutItem semantics), and the KeyProvider trait (which currently assumes DynamoDB's item-level encryption granularity). The zero-downtime migration path from "RFC 0058 in progress" to "hybrid relational+DynamoDB" is at minimum a second 9-week effort, and during the transition you have exactly the code-path divergence RFC 0058 was designed to prevent -- two live storage engines with partial data in each.

[PERSPECTIVE P02: The graph traversal problem is a read-path optimization, not an architecture decision]
The verdict denormalization Muffin identifies is a write-time pre-computation that costs O(refs) extra writes per verdict but eliminates unbounded read-time graph traversal. This is a standard materialized-view pattern. If reverse-ref lookups become a hot path, a single GSI on `ref#{target_id}#{ref_type}#{source_id}` solves it within DynamoDB without introducing a second database. The question is whether the 3-5 additional GSIs this might eventually require cost more operationally than running, securing, and migrating to an entirely separate PostgreSQL cluster.

[TENSION T01: No rollback plan exists if the hybrid migration stalls mid-flight]
Neither RFC 0053's abstraction layer nor RFC 0058 addresses what happens if a superseding hybrid architecture is partially adopted and then abandoned -- partial data in two backends with no single source of truth is the worst possible outcome, and the dialogue has not yet surfaced a concrete rollback strategy.

---
