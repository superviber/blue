[PERSPECTIVE P01: DynamoDB cost model penalizes the graph-heavy query patterns RFC 0051 demands]
RFC 0051's 14 entity types with M:N cross-references (refs table), recursive lifecycle chains (perspective_events, tension_events), and multi-entity verdicts mean most read operations require multiple DynamoDB queries or large BatchGetItem calls across different SK prefixes. At DynamoDB on-demand pricing ($1.25/million read request units), a single dialogue export touching 6-8 entity types with fan-out on refs will cost 10-50x what a single PostgreSQL query joining those tables would cost, and that multiplier compounds as dialogue history grows. Neon or Turso serverless Postgres gives you the same pay-per-query economics DynamoDB offers, but with JOINs that collapse the fan-out to one round-trip.

[PERSPECTIVE P02: A hybrid adds operational cost that may exceed the query savings at current scale]
Running both a relational DB and DynamoDB means two backup strategies, two monitoring dashboards, two failure modes, and two sets of capacity planning assumptions. Below roughly 10,000 dialogues or 100 concurrent users, the DynamoDB query cost premium is likely under $5/month -- meaning the hybrid's operational complexity tax exceeds its savings until the project hits meaningful scale.

[TENSION T01: Scale threshold where single-engine simplicity loses to hybrid efficiency is undefined]
Without projected dialogue volume, average entities per dialogue, and read/write ratio estimates, neither architecture can be justified on cost grounds alone -- the decision defaults to developer ergonomics and query expressiveness.

---
