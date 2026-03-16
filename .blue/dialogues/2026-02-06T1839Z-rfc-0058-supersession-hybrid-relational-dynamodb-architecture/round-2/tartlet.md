[PERSPECTIVE P01: AAD portability requires a logical addressing layer, not just format documentation]
The Judge's Open Question 3 asks whether encryption envelope portability is a pre-condition or a follow-up. It is a pre-condition, but a narrowly solvable one. RFC 0058 computes `aad_hash = sha256(pk||sk)` where pk = `dialogue#{id}` and sk = `{entity_type}#{subkey}` -- these are DynamoDB-native composite keys. A backend swap to SQLite or PostgreSQL would use `(dialogue_id, entity_type, subkey)` as separate columns, producing a different AAD input and making every existing ciphertext undecryptable without the original pk||sk reconstruction. The fix is a one-line specification: define AAD as `sha256(canonical_entity_address)` where `canonical_entity_address` is a backend-independent string (e.g., `dialogue:{id}/entity:{type}/{subkey}`), computed identically regardless of how the backend physically keys the row. This costs nothing to implement now but becomes a re-encryption-of-all-data project if deferred past the first encrypted write.

[PERSPECTIVE P02: Hash chain continuity across backend boundaries is the harder unsolved half]
Even with portable AAD, the audit event hash chain (`event#{entity_type}#{entity_id}#{timestamp}`) chains each event's hash to its predecessor. If a migration moves dialogues to a new backend in batches, the chain must include a signed migration-boundary event that bridges the old backend's terminal hash to the new backend's genesis hash -- otherwise an auditor cannot verify chain integrity across the cut. No RFC specifies this boundary event. Unlike AAD portability, this cannot be solved by a format change; it requires a migration protocol that RFC 0053's `DialogueStore` trait does not address.

[REFINEMENT: Galette's prerequisite inversion strengthens the case for specifying envelope portability now]
Galette showed that the trait does not exist in code yet (30+ bare `rusqlite::Connection` calls). Since the trait must be built before RFC 0058 ships, and the AAD format must be defined before the trait's encryption wrapper is implemented, specifying canonical AAD is not premature -- it is on the critical path of the work Galette says must happen first.

[RESOLVED MACARON R1-T01]
Macaron's denormalization cost concern is bounded: with 10-20 verdicts per dialogue and Cannoli's measured partition sizes under 100 items, write amplification is a constant-factor cost, not an architecture-forcing constraint. Quantification confirms it is negligible.

---
