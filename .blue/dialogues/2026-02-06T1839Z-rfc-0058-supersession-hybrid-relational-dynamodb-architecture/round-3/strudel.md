[PERSPECTIVE P01: Refs table disagreement is a Phase C implementation detail, not an architectural blocker]
Cannoli's cleartext-refs-as-separate-items argument is stronger than my inline-only proposal on one specific axis: ref mutation cost. If a ref is embedded in an encrypted entity payload, adding or removing an edge requires decrypting, mutating, re-encrypting, and re-sealing the source entity -- whereas a separate cleartext ref item is a simple put/delete with no cryptographic overhead. Since both approaches produce identical results under the agreed full-partition-load + in-memory-assembly pattern, the choice is a write-path optimization that Phase C implementation will resolve empirically. I withdraw my insistence on dropping the refs table.

[CONCESSION: Cannoli's cleartext ref items are the cheaper write-path representation]
Cannoli correctly identified that separate cleartext ref items avoid re-sealing encrypted entity payloads on edge mutation, which is a concrete cost advantage my inline-only proposal did not account for.

[RESOLVED T01: Graph assembly location]
Croissant's R2 resolution is correct: the trait returns domain types via partition-scoped CRUD, and graph assembly lives in a shared library above the trait. No RFC change needed -- this is the natural consequence of the domain-shaped trait design the panel converged on.

[MOVE:CONVERGE]

---
