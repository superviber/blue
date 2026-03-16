[PERSPECTIVE P01: Single-table design already handles the RFC 0051 graph]
The RFC 0051 schema's 14 entity types, cross-references, and lifecycle tracking are well within single-table design's proven envelope. The `ref#{source}#{type}#{target}` sort key pattern encodes directed edges natively, GSI-2 enables cross-dialogue tension queries, and every hot-path query is scoped to a single partition key (`dialogue#{id}`), which is exactly the access pattern DynamoDB optimizes for. Adding a relational database does not simplify these queries -- it adds a second operational surface, a second failure mode, and a second consistency boundary that must be kept in sync.

[PERSPECTIVE P02: The "graph complexity" argument conflates schema complexity with query complexity]
M:N cross-references and recursive dependency chains sound relational, but the actual access patterns in RFC 0051 are hierarchical reads within a dialogue partition plus three GSI-backed cross-partition lookups. No query traverses more than two hops. DynamoDB single-table design was purpose-built for exactly this: known access patterns with bounded fan-out. A relational database would only be justified if ad-hoc multi-hop graph traversals were a hot-path requirement, and nothing in the current system demands that.

[TENSION T01: Unbounded future query patterns vs. locked access patterns]
If cross-dialogue analytics (e.g., "find all tensions influenced by perspectives from expert X across 50 dialogues") becomes a production hot path, single-table design would require new GSIs or a read replica strategy, while a relational backend could answer ad-hoc queries natively.

---
