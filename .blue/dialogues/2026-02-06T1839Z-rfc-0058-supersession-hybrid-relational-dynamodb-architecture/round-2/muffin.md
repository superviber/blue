[PERSPECTIVE P01: Verdict immutability dissolves MUFFIN R1-T01 but reveals a different consistency gap]
I examined the codebase: `register_verdict` is INSERT-only with no UPDATE path, and `tensions_resolved`/`key_evidence`/`key_claims` are point-in-time snapshots frozen at verdict creation. This means my R1 staleness concern was wrong -- verdicts are append-only records, not live materialized views, so they cannot drift from the refs graph because they were never meant to track it continuously. However, the DynamoDB denormalized model embeds these arrays *inside the encrypted verdict item*, which means a reader cannot verify whether a verdict's `tensions_resolved` list is complete relative to the current refs graph without loading both the verdict and all refs, then diffing -- exactly the cross-entity join that DynamoDB denormalization was supposed to eliminate. In SQLite this verification is a single query (`SELECT ... FROM refs WHERE target_type = 'tension' AND NOT IN (SELECT ... FROM verdict.tensions_resolved)`); in DynamoDB it requires the full-partition load Cannoli already endorses, which makes the denormalized arrays redundant rather than stale.

[PERSPECTIVE P02: Redundant denormalization is a trait governance test case]
Croissant's proposed O(1)-partition-read gate for trait methods should extend to schema: if every read already requires full-partition load (as Cannoli argues), then pre-computed verdict arrays add write complexity for zero read benefit. The first governance decision the trait review gate should make is whether `tensions_resolved`/`key_evidence`/`key_claims` belong in the verdict entity at all, or whether they should be computed at read time from the in-memory entity graph. This concretely answers the Judge's Q2 ("What does the trait governance gate look like?") -- it starts with schema review, not just method review.

[RESOLVED MUFFIN R1-T01]
Verdicts are immutable snapshots, not live views; they cannot drift from the refs graph because they are not designed to track it. The consistency risk I raised was based on a misreading of the write model.

[REFINEMENT: ECLAIR R1-T01 narrowed to redundancy, not leakage]
Eclair's concern about denormalization artifacts leaking into the domain model is valid but the framing should shift: the issue is not that future backends must maintain dead denormalization logic, but that the denormalized fields are already redundant given Cannoli's full-partition-load pattern -- making them unnecessary complexity in any backend, not just a DynamoDB-specific tax.

---
