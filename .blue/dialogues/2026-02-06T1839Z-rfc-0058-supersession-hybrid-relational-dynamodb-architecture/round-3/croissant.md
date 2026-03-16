[PERSPECTIVE P01: Trait layering resolves Strudel's graph assembly tension without new architecture]
Strudel's R2-T01 asked who owns graph assembly. The answer falls naturally out of the converged trait design: `DialogueStore` returns partition-scoped entity collections (perspectives, tensions, recommendations, verdicts, refs), and a shared `DialogueGraph` module above the trait assembles the adjacency structure in memory. This is not a trait method -- it is a pure function over domain types, testable without any backend. Both `SqliteDialogueStore` and `DynamoDialogueStore` return the same domain structs; the graph assembly code is written once and shared. No new RFC is needed; this is a natural consequence of the partition-scoped CRUD trait shape already agreed.

[PERSPECTIVE P02: The refs table question does not block convergence]
Strudel wants inline-only refs; Cannoli wants cleartext DynamoDB ref items. Both designs feed the same in-memory assembly pattern, both satisfy the `PartitionScoped` constraint, and neither leaks into the trait contract. This is a Phase C implementation choice that the dual-implementation CI gate will arbitrate -- whichever representation passes the shared test suite with simpler code wins. It need not be settled in this dialogue.

[RESOLVED STRUDEL R2-T01]
Graph assembly as a shared library above the trait, operating on domain types returned by partition-scoped CRUD methods, is the consensus position and requires no additional specification.

[MOVE:CONVERGE]

---
