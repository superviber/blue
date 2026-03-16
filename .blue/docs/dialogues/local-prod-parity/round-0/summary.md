# Round 0 Summary: Local-Production Parity

**ALIGNMENT Score**: +87 | **Panel**: 8 experts | **Status**: Strong opening, key tensions surfaced

## Perspectives Registered

| ID | Label | Contributors |
|----|-------|--------------|
| P0001 | Docker Compose as orchestration standard | Muffin |
| P0002 | Encryption algorithms must be identical (sources can differ) | Cupcake, Croissant |
| P0003 | Three explicit security modes (PROD/STAGING/LOCAL) | Cupcake |
| P0004 | LocalSecretsProvider replaces Infisical locally | Cupcake, Brioche, Eclair |
| P0005 | Two-minute onboarding rule | Scone |
| P0006 | DynamoDB Local has 99% parity | Eclair |
| P0007 | Dashboard decryption via backend mode switching | Donut |
| P0008 | Full production parity is unnecessary | Macaron |
| P0009 | Progressive disclosure (Level 0-3) | Scone |
| P0010 | DashboardEnvelope pattern for encrypted/plaintext | Donut |
| P0011 | Key hierarchy must be fully exercised locally | Croissant |
| P0012 | Docker Compose profiles for tiered parity | Brioche |

## Tensions

| ID | Tension | Status |
|----|---------|--------|
| T0001 | Dashboard decryption vs zero-knowledge guarantee | OPEN |
| T0002 | Full parity vs developer ergonomics | OPEN |
| T0003 | Auto-generated keys vs reproducible testing | OPEN |
| T0004 | Docker requirement vs "just clone and run" | OPEN |
| T0005 | Infisical SDK code path not exercised locally | OPEN |

## Recommendations

| ID | Recommendation | From |
|----|----------------|------|
| R0001 | Standard directory layout (deploy/local/) | Muffin |
| R0002 | Docker healthchecks for service ordering | Muffin |
| R0003 | LocalSecretsProvider implementation | Cupcake |
| R0004 | DecryptionContext abstraction | Cupcake |
| R0005 | Unified dev server command (`npm run dev`) | Scone |
| R0006 | DynamoDB with graceful fallback | Scone |
| R0007 | KeyProvider abstraction (Local vs KMS) | Eclair |
| R0008 | WebSocket server for local dashboard | Eclair |
| R0009 | DashboardEnvelope pattern | Donut |
| R0010 | secrets.schema.json for validation | Brioche |
| R0011 | Crypto conformance test suite | Croissant |
| R0012 | Three-tier environment (Local/CI/Staging) | Macaron |

## Emerging Camps

**Camp A: Minimal Viable Parity (Muffin, Cupcake, Croissant, Donut)**
- Run DynamoDB Local + dashboard in Docker
- Exercise full encryption code paths
- Local keys, same algorithms

**Camp B: Skip Infrastructure Parity (Macaron, Scone)**
- SQLite is sufficient for most developers
- Docker is optional overhead
- CI handles DynamoDB testing

## Key Question for Round 1

> Should local development REQUIRE Docker, or should Docker be optional for "advanced" testing?

## Panel Evolution for Round 1

- **Retain**: Cupcake, Scone, Macaron (core debate participants)
- **Add from pool**: QA Engineer (testing perspective), SRE Lead (observability)
- **Create**: DevOps Pragmatist (bridge the Docker debate)
