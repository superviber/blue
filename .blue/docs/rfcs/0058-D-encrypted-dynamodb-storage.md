# RFC 0058: Encrypted DynamoDB Storage with True Local-Production Parity

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-03 |
| **Supersedes** | RFC 0056, RFC 0057 |
| **Implements** | RFC 0051 (Global Perspective & Tension Tracking) schema |
| **Dialogue** | ALIGNMENT score 289, 10 experts, 3 rounds to 100% convergence |
| **Amendment** | Supersession dialogue: ALIGNMENT 415, 12 experts, 4 rounds, 17/17 tensions resolved |

---

## Summary

Create a new Rust crate (`blue-encrypted-store`) implementing client-side encrypted DynamoDB storage with **true local-production parity**. The crate implements the full RFC 0051 schema (14 entity types: dialogues, experts, rounds, verdicts, perspectives, tensions, recommendations, evidence, claims, moves, refs, and audit events) using DynamoDB's single-table design pattern.

The same code runs locally (against DynamoDB Local) and in production (against AWS DynamoDB). Docker is required for local development. No fallbacks, no tiers, no simulation modes.

## Amendment: Supersession Dialogue (2026-02-06)

A 12-expert, 4-round alignment dialogue evaluated whether RFC 0058 should be superseded by a hybrid relational + DynamoDB architecture. The panel achieved 100% convergence (ALIGNMENT 415, 17/17 tensions resolved) with the following verdict and amendments.

### Verdict: Do NOT Supersede

RFC 0058 proceeds. A hybrid architecture was unanimously rejected. The panel identified three amendments to the implementation sequence and schema.

### Amendment 1: Three-Phase Implementation Sequence

The original 4-phase migration path (Weeks 1-9) is replaced by a prerequisite-respecting three-phase gate:

**Phase A — Build the Trait Boundary (RFC 0053)**
- Extract `DialogueStore` trait from the 32 existing `&Connection` functions in `alignment_db.rs`
- Implement `SqliteDialogueStore` as the reference implementation
- Convert all `dialogue.rs` handler call sites to use the trait
- **Exit gate**: Zero bare `pub fn ...(conn: &Connection)` signatures in `alignment_db.rs`
- **Rationale**: The trait boundary does not exist in code today (30+ direct `rusqlite::Connection` call sites). Building it forces concrete design decisions and makes the backend pluggable before any DynamoDB work begins.

**Phase B — Define Portable Encryption Envelope**
- AAD = `sha256(canonical_entity_address)` where canonical address = `dialogue:{id}/entity:{type}/{subkey}`
- The canonical address is backend-independent by construction — the same string regardless of whether the physical key is DynamoDB pk/sk or SQL columns
- **Exit gate**: Envelope spec passes round-trip encrypt/decrypt test across both SQLite and DynamoDB backends
- **Rationale**: The original `aad_hash = sha256(pk||sk)` implicitly couples the encryption envelope to DynamoDB's key structure. If deferred past the first encrypted write, every future backend swap becomes a re-encryption-of-all-data project.

**Phase C — Implement DynamoDB Behind the Trait (this RFC)**
- `DynamoDialogueStore` implements the stable `DialogueStore` trait
- Full-partition load + in-memory graph assembly as the read pattern
- DynamoDB Local integration tests pass the same generic test suite as SQLite
- Refs table design (inline vs cleartext items) resolved empirically in this phase
- Hash chain boundary event specified in migration protocol
- **Exit gate**: Dual-implementation CI passes (both `SqliteDialogueStore` and `DynamoDialogueStore` pass identical test suites)

### Amendment 2: Eliminate Verdict Denormalization

The verdict entity's pre-computed arrays (`tensions_resolved`, `recommendations_adopted`, `key_evidence`, `key_claims`) are removed from the schema. Instead, verdict context is assembled at read time:

1. Full-partition load: `Query(PK=dialogue#{id})` returns all entities (~100 items, <10KB)
2. In-memory graph assembly: build adjacency map from refs, traverse in microseconds
3. No write-amplification, no staleness risk, no consistency mechanism needed

This change applies to both the DynamoDB and SQLite implementations. Verdicts remain immutable INSERT-only snapshots; the denormalized fields were redundant given the full-partition-load pattern.

**Affected schema**: Remove `tensions_resolved`, `recommendations_adopted`, `key_evidence`, `key_claims` from the encrypted verdict payload (lines 157-158 of the Verdicts entity).

### Amendment 3: Trait Governance via ADR + PartitionScoped Marker

A new ADR (extending ADR-0018) governs the `DialogueStore` trait surface:

1. **Partition-scoped rule**: Every `DialogueStore` method must accept `dialogue_id` as its partition key and return results scoped to that partition
2. **Compile-time enforcement**: A `PartitionScoped` marker trait on return types prevents cross-partition queries from being added to `DialogueStore`
3. **Separate AnalyticsStore**: Cross-partition queries (e.g., `get_cross_dialogue_stats`, `find_similar_dialogues`) are segregated to a separate `AnalyticsStore` trait with no DynamoDB implementation requirement
4. **Current compliance**: 31 of 34 existing public functions (91%) already satisfy the partition-scoped rule; only 3 functions need segregation

**Graph assembly layer**: A shared `DialogueGraph` module above the trait assembles adjacency structures from partition-scoped entity collections. This is a pure function over domain types, not a trait method — written once, shared by all backends.

### Dialogue Provenance

| Metric | Value |
|--------|-------|
| ALIGNMENT Score | 415 (W:135 C:104 T:92 R:84) |
| Rounds | 4 (R0-R3) |
| Experts Consulted | 12 unique |
| Tensions Resolved | 17/17 |
| Convergence | 100% (6/6 unanimous) |

| Expert | Key Contribution |
|--------|-----------------|
| Strudel | "The schema is the problem, not the storage engine" — reframed the debate |
| Croissant | ADR + PartitionScoped marker trait (91% of functions already comply) |
| Galette | "Building the trait IS the design decision" — prerequisite inversion |
| Cannoli | Full-partition load + in-memory assembly eliminates denormalization |
| Tartlet | Canonical entity address for AAD portability |
| Muffin | Confirmed verdict immutability; denormalization elimination is correct normalization |

Full dialogue: `.blue/dialogues/2026-02-06T1839Z-rfc-0058-supersession-hybrid-relational-dynamodb-architecture/`

---

## Problem

Previous RFCs (0056, 0057) proposed tiered architectures with progressive disclosure—SQLite for quick local dev, DynamoDB for "advanced" testing. This creates:

1. **Code path divergence** - Bugs that only manifest in production
2. **False confidence** - "Works on my machine" with different storage backend
3. **Deployment anxiety** - Can't deploy at a moment's notice

The user explicitly rejected this approach:
> "I do NOT like the tiered architecture. We want to be able to deploy at a moment's notice and have tested the same code that will run in prod locally."

## Architecture

### Core Principle: Configuration, Not Code Divergence

```
┌─────────────────────────────────────────────────────────────────┐
│                     SAME RUST CODE                               │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────┐  │
│  │ EncryptedStore  │───▶│ DynamoDialogue  │───▶│ KeyProvider │  │
│  │    <D, K>       │    │     Store       │    │   (trait)   │  │
│  └─────────────────┘    └─────────────────┘    └─────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│     LOCAL DEVELOPMENT    │     │       PRODUCTION        │
├─────────────────────────┤     ├─────────────────────────┤
│ DYNAMODB_ENDPOINT=      │     │ DYNAMODB_ENDPOINT=      │
│   http://localhost:8000 │     │   (AWS default)         │
│                         │     │                         │
│ KeyProvider=            │     │ KeyProvider=            │
│   LocalFileKeyProvider  │     │   AwsKmsKeyProvider     │
│   ~/.blue/keys/umk.key  │     │   arn:aws:kms:...       │
└─────────────────────────┘     └─────────────────────────┘
```

### KeyProvider Trait

```rust
/// Abstracts key source. Crypto operations are IDENTICAL.
#[async_trait]
pub trait KeyProvider: Send + Sync {
    /// Retrieve the User Master Key handle
    async fn get_umk(&self) -> Result<UmkHandle, KeyError>;

    /// Derive Key Encryption Key from UMK + context
    async fn derive_kek(&self, umk: &UmkHandle, context: &[u8]) -> Result<KekHandle, KeyError>;

    /// Generate a new Data Encryption Key, return handle + wrapped form
    async fn generate_dek(&self) -> Result<(DekHandle, EncryptedDek), KeyError>;

    /// Unwrap a Data Encryption Key using KEK
    async fn decrypt_dek(&self, kek: &KekHandle, encrypted: &EncryptedDek) -> Result<DekHandle, KeyError>;
}

/// Local development: reads UMK from file, does HKDF locally
pub struct LocalFileKeyProvider {
    umk_path: PathBuf,
}

/// Production: UMK lives in KMS, derivation uses KMS operations
pub struct AwsKmsKeyProvider {
    kms_client: aws_sdk_kms::Client,
    cmk_arn: String,
}
```

### Three-Tier Key Hierarchy

| Layer | Local | Production | Derivation |
|-------|-------|------------|------------|
| **UMK** | 256-bit file (`~/.blue/keys/umk.key`) | KMS CMK | - |
| **KEK** | HKDF-SHA256(UMK, user_id) | KMS-derived | Per-user |
| **DEK** | AES-256 key | Same | Per-dialogue |

The hierarchy is exercised identically in both environments. Only the UMK source differs.

### DynamoDB Schema

Single-table design mapping the full RFC 0051 schema. Same schema everywhere.

```
Table: blue_dialogues

Primary Key:
  PK: dialogue#{dialogue_id}
  SK: {entity_type}#{subkey}

═══════════════════════════════════════════════════════════════════════════════
ENTITY MAPPING (14 SQLite tables → 1 DynamoDB table)
═══════════════════════════════════════════════════════════════════════════════

┌─────────────────────────────────────────────────────────────────────────────┐
│ DIALOGUES (root entity)                                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: meta                                                                     │
│                                                                              │
│ Cleartext: status, created_at, converged_at, total_rounds, total_alignment  │
│            calibrated, domain_id, ethos_id                                   │
│ Encrypted: title, question, output_dir                                       │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ EXPERTS (participation per dialogue)                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: expert#{expert_slug}                                                     │
│     expert#muffin                                                            │
│     expert#cupcake                                                           │
│                                                                              │
│ Cleartext: tier, source, relevance, first_round, total_score, created_at    │
│ Encrypted: role, description, focus, creation_reason, color, scores,        │
│            raw_content (JSON: per-round responses)                           │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ ROUNDS (metadata per round)                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: round#{round:02d}                                                        │
│     round#00                                                                 │
│     round#01                                                                 │
│                                                                              │
│ Cleartext: score, status, created_at, completed_at                           │
│ Encrypted: title, summary                                                    │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ VERDICTS (first-class verdict entities)                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: verdict#{verdict_id}                                                     │
│     verdict#final                                                            │
│     verdict#minority                                                         │
│     verdict#V01                                                              │
│                                                                              │
│ Cleartext: verdict_type, round, confidence, created_at                       │
│ Encrypted: author_expert, recommendation, description, conditions,           │
│            vote, supporting_experts, ethos_compliance                        │
│            [AMENDED: tensions_resolved, recommendations_adopted,             │
│             key_evidence, key_claims REMOVED — computed at read time          │
│             via full-partition load + in-memory graph assembly]              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ PERSPECTIVES                                                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: perspective#{round:02d}#{seq:02d}                                        │
│     perspective#00#01  → P0001                                               │
│     perspective#01#02  → P0102                                               │
│                                                                              │
│ Cleartext: status, created_at                                                │
│ Encrypted: label, content, contributors, references                          │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ TENSIONS                                                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: tension#{round:02d}#{seq:02d}                                            │
│     tension#00#01  → T0001                                                   │
│     tension#01#03  → T0103                                                   │
│                                                                              │
│ Cleartext: status, created_at                                                │
│ Encrypted: label, description, contributors, references                      │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ RECOMMENDATIONS                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: recommendation#{round:02d}#{seq:02d}                                     │
│     recommendation#00#01  → R0001                                            │
│                                                                              │
│ Cleartext: status, adopted_in_verdict, created_at                            │
│ Encrypted: label, content, contributors, parameters, references              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ EVIDENCE                                                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: evidence#{round:02d}#{seq:02d}                                           │
│     evidence#00#01  → E0001                                                  │
│                                                                              │
│ Cleartext: status, created_at                                                │
│ Encrypted: label, content, contributors, references                          │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ CLAIMS                                                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: claim#{round:02d}#{seq:02d}                                              │
│     claim#00#01  → C0001                                                     │
│                                                                              │
│ Cleartext: status, created_at                                                │
│ Encrypted: label, content, contributors, references                          │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ MOVES (dialogue moves: defend, challenge, bridge, etc.)                      │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: move#{round:02d}#{expert_slug}#{seq:02d}                                 │
│     move#01#muffin#01                                                        │
│                                                                              │
│ Cleartext: move_type, created_at                                             │
│ Encrypted: targets, context                                                  │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ REFS (cross-references between entities)                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: ref#{source_id}#{ref_type}#{target_id}                                   │
│     ref#P0101#support#P0001                                                  │
│     ref#R0001#address#T0001                                                  │
│     ref#P0102#resolve#T0002                                                  │
│                                                                              │
│ Cleartext: source_type, target_type, ref_type, created_at                    │
│ (No encrypted fields - refs are structural metadata)                         │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ EVENTS (unified audit trail for all entity types)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│ SK: event#{entity_type}#{entity_id}#{timestamp}                              │
│     event#perspective#P0001#2026-02-03T10:15:00Z                             │
│     event#tension#T0001#2026-02-03T10:20:00Z                                 │
│     event#recommendation#R0001#2026-02-03T10:25:00Z                          │
│                                                                              │
│ Cleartext: event_type, event_round, created_at                               │
│ Encrypted: actors, reason, reference, result_id                              │
│                                                                              │
│ Maps: perspective_events, tension_events, recommendation_events              │
└─────────────────────────────────────────────────────────────────────────────┘

═══════════════════════════════════════════════════════════════════════════════
GLOBAL SECONDARY INDEXES
═══════════════════════════════════════════════════════════════════════════════

GSI-1 (ByStatus): Query dialogues by status
  PK: status#{status}
  SK: updated_at#{dialogue_id}

GSI-2 (ByTensionStatus): Query open tensions across dialogues
  PK: tension_status#{status}
  SK: dialogue_id#{tension_id}

GSI-3 (ByExpert): Query all contributions by expert
  PK: expert#{expert_slug}
  SK: dialogue_id#{round}

═══════════════════════════════════════════════════════════════════════════════
ENCRYPTION ENVELOPE
═══════════════════════════════════════════════════════════════════════════════

All items with encrypted fields include:
  - content_encrypted: Binary (AES-256-GCM ciphertext of JSON payload)
  - content_nonce: Binary (12 bytes, unique per item)
  - key_id: String (DEK reference: "dek#{dialogue_id}#{version}")
  - aad_hash: String (SHA-256 of canonical entity address — see Amendment 2)

The encrypted payload is a JSON object containing all "Encrypted" fields
listed above for each entity type.

Example for a perspective:
  Cleartext item:
    PK: dialogue#nvidia-analysis
    SK: perspective#01#02
    status: "open"
    created_at: "2026-02-03T10:00:00Z"
    content_encrypted: <binary>
    content_nonce: <12 bytes>
    key_id: "dek#nvidia-analysis#1"
    aad_hash: "sha256(dialogue:nvidia-analysis/entity:perspective/01#02)"

  Decrypted payload:
    {
      "label": "Valuation premium justified",
      "content": "The 35x forward P/E is justified by...",
      "contributors": ["muffin", "cupcake"],
      "references": [{"type": "support", "target": "E0001"}]
    }
```

### Storage Implementation

```rust
pub struct DynamoDialogueStore {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
}

impl DynamoDialogueStore {
    pub async fn new(config: DynamoConfig) -> Result<Self> {
        let sdk_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(&config.region));

        // Endpoint override is the ONLY difference between local and prod
        let sdk_config = match &config.endpoint {
            Some(ep) => sdk_config.endpoint_url(ep).load().await,
            None => sdk_config.load().await,
        };

        Ok(Self {
            client: Client::new(&sdk_config),
            table_name: config.table_name,
        })
    }
}

pub struct EncryptedStore<S, K> {
    inner: S,
    key_provider: K,
    audit_logger: CryptoAuditLogger,
}
```

## Infrastructure

### Docker Compose (Required)

```yaml
# docker-compose.yml
version: "3.8"

services:
  dynamodb:
    image: amazon/dynamodb-local:2.2.1
    container_name: blue-dynamodb
    command: "-jar DynamoDBLocal.jar -sharedDb -dbPath /data"
    ports:
      - "8000:8000"
    volumes:
      - dynamodb-data:/data
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:8000/shell/ || exit 1"]
      interval: 5s
      timeout: 3s
      retries: 5

volumes:
  dynamodb-data:
    name: blue-dynamodb-data
```

### Developer Workflow (justfile)

```just
# Start local infrastructure
up:
    docker compose up -d
    @just wait-ready

# Wait for DynamoDB health
wait-ready:
    @echo "Waiting for DynamoDB..."
    @until docker inspect blue-dynamodb --format='{{.State.Health.Status}}' | grep -q healthy; do sleep 1; done
    @echo "Ready."

# Run all tests
test: up
    cargo test

# First-time setup
setup:
    @command -v docker >/dev/null || (echo "Install Docker first" && exit 1)
    docker compose pull
    just up
    cargo run --bin setup-tables
    @echo "Setup complete. Run 'just test' to verify."

# Clean everything
nuke:
    docker compose down -v
    rm -rf ~/.blue/keys/
```

### First-Time Experience

```bash
git clone <repo>
cd blue-encrypted-store
just setup    # ~2 minutes: pulls Docker image, creates tables, generates local key
just test     # All tests pass
```

## Test Strategy

### Three-Layer Test Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: Pure Unit Tests (cargo test --lib)                     │
│ - Crypto primitives with NIST KAT vectors                       │
│ - Serialization, schema validation                              │
│ - NO Docker required                                            │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: Integration Tests (cargo test, Docker required)        │
│ - DynamoDB Local via docker-compose                             │
│ - Same code path as production                                  │
│ - Envelope format conformance with reference vectors            │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: Property Tests (cargo test --features=proptest)        │
│ - Round-trip: decrypt(encrypt(x)) == x                          │
│ - Fuzzing key/plaintext combinations                            │
└─────────────────────────────────────────────────────────────────┘
```

### Test Vector Generation (Hybrid Approach)

| Source | Purpose | Location |
|--------|---------|----------|
| NIST CAVP KAT | Verify AES-256-GCM primitives | `tests/fixtures/nist_kat_aes_gcm.json` |
| Python `cryptography` | Envelope format conformance | `tests/fixtures/envelope_vectors.json` |
| `scripts/generate_vectors.py` | Reproducible generation | Committed with hash |

```rust
// tests/crypto/test_nist_kat.rs
#[test]
fn test_aes_gcm_nist_vectors() {
    let vectors: Vec<NistVector> = load_nist_kat();
    for v in vectors {
        let ciphertext = aes_gcm_encrypt(&v.plaintext, &v.key, &v.iv, &v.aad);
        assert_eq!(ciphertext, v.expected_ciphertext);
    }
}
```

## Local Key Management

### Key Rotation: Never (By Design)

Local development keys are disposable test data. They are never rotated.

```rust
impl LocalFileKeyProvider {
    pub async fn get_umk(&self) -> Result<UmkHandle, KeyError> {
        match fs::read(&self.umk_path) {
            Ok(bytes) => Ok(UmkHandle::from_bytes(&bytes)?),
            Err(e) if e.kind() == NotFound => {
                // First run: generate new key
                let key = generate_random_key();
                fs::create_dir_all(self.umk_path.parent().unwrap())?;
                fs::write(&self.umk_path, &key)?;
                Ok(UmkHandle::from_bytes(&key)?)
            }
            Err(e) => Err(KeyError::Io(e)),
        }
    }
}
```

### Key Loss Recovery

```
If UMK deleted or corrupted:
  1. CLI detects decrypt failure on next operation
  2. Prompts: "Local encryption key changed. Reset database? [y/N]"
  3. Yes → wipe ~/.blue/db/, generate fresh UMK
  4. No → abort with instructions
```

**Documentation requirement**: Clear warning that local data is not durable.

## Observability

### Crypto Audit Logging

```rust
pub struct CryptoAuditEvent {
    pub event_id: Uuid,           // UUIDv7 for ordering
    pub timestamp: DateTime<Utc>,
    pub trace_id: String,         // OpenTelemetry correlation
    pub span_id: String,
    pub operation: CryptoOp,      // encrypt|decrypt|derive|wrap|unwrap
    pub key_id: String,
    pub principal: String,
    pub outcome: Outcome,         // success|failure|denied
    pub previous_hash: String,    // Chain integrity
    pub event_hash: String,       // SHA-256(all above)
}

pub enum CryptoOp {
    Encrypt,
    Decrypt,
    DeriveKek,
    WrapDek,
    UnwrapDek,
}
```

### Hash Chain for Tamper-Evidence

- Every audit event includes `previous_hash` (hash of prior event)
- Append-only storage: `dynamodb:PutItem` only, no modify/delete
- Daily verification job validates chain integrity
- Alert on any hash discontinuity

### Trace Context Correlation

Audit events include `trace_id` and `span_id` for correlation with application traces. The hash chain provides tamper-evidence independent of trace integrity.

## Migration Path

> **AMENDED**: The original 4-phase migration (Weeks 1-9) is replaced by a prerequisite-respecting
> three-phase gate. See "Amendment 1: Three-Phase Implementation Sequence" above for full details.

### Phase A: Trait Boundary (RFC 0053) — Prerequisite

- [ ] Extract `DialogueStore` trait from 32 existing `&Connection` functions
- [ ] Implement `SqliteDialogueStore` as reference implementation
- [ ] Convert all `dialogue.rs` handler call sites
- [ ] Segregate 3 cross-partition functions to `AnalyticsStore` trait
- [ ] Add `PartitionScoped` marker trait governance (ADR)
- [ ] Remove verdict denormalization arrays from domain model
- [ ] **Gate**: Zero bare `pub fn ...(conn: &Connection)` in `alignment_db.rs`

### Phase B: Portable Encryption Envelope — Prerequisite

- [ ] Define canonical entity address format: `dialogue:{id}/entity:{type}/{subkey}`
- [ ] Implement `AAD = sha256(canonical_entity_address)` in encryption layer
- [ ] Implement `KeyProvider` trait + `LocalFileKeyProvider`
- [ ] Implement `EncryptedStore<S, K>` wrapper
- [ ] NIST KAT tests for crypto primitives
- [ ] **Gate**: Envelope round-trip test passes across both SQLite and DynamoDB backends

### Phase C: DynamoDB Implementation (this RFC)

- [ ] Create `blue-encrypted-store` crate
- [ ] Implement `DynamoDialogueStore` behind stable `DialogueStore` trait
- [ ] Docker Compose + justfile setup for DynamoDB Local
- [ ] All 14 entity types with encryption (verdict WITHOUT denormalized arrays)
- [ ] Full-partition load + in-memory graph assembly pattern
- [ ] Refs table design resolved empirically (inline vs cleartext items)
- [ ] GSI implementations (ByStatus, ByTensionStatus, ByExpert)
- [ ] Implement `AwsKmsKeyProvider`
- [ ] Hash chain boundary event in migration protocol
- [ ] Property-based tests
- [ ] Crypto audit logging with hash chain
- [ ] **Gate**: Dual-implementation CI passes (SQLite + DynamoDB identical test suites)

### Phase D: Blue Integration

- [ ] Import crate into Blue MCP server
- [ ] Migrate existing SQLite-backed tools to DynamoDB via trait swap
- [ ] Dashboard integration
- [ ] Production deployment to AWS

## RFC 0051 Schema Mapping

Complete mapping from SQLite tables (RFC 0051) to DynamoDB single-table design:

| SQLite Table | DynamoDB SK Pattern | Encrypted Fields |
|--------------|---------------------|------------------|
| `dialogues` | `meta` | title, question, output_dir |
| `experts` | `expert#{slug}` | role, description, focus, scores, raw_content |
| `rounds` | `round#{round:02d}` | title, summary |
| `verdicts` | `verdict#{id}` | recommendation, description, conditions, vote, tensions_*, key_* |
| `perspectives` | `perspective#{round:02d}#{seq:02d}` | label, content, contributors, references |
| `tensions` | `tension#{round:02d}#{seq:02d}` | label, description, contributors, references |
| `recommendations` | `recommendation#{round:02d}#{seq:02d}` | label, content, contributors, parameters, references |
| `evidence` | `evidence#{round:02d}#{seq:02d}` | label, content, contributors, references |
| `claims` | `claim#{round:02d}#{seq:02d}` | label, content, contributors, references |
| `moves` | `move#{round:02d}#{expert}#{seq:02d}` | targets, context |
| `refs` | `ref#{source}#{type}#{target}` | *(none - structural metadata)* |
| `perspective_events` | `event#perspective#{id}#{ts}` | actors, reason, reference, result_id |
| `tension_events` | `event#tension#{id}#{ts}` | actors, reason, reference, result_id |
| `recommendation_events` | `event#recommendation#{id}#{ts}` | actors, reason, reference, result_id |

**Cleartext fields** (always visible for queries): `pk`, `sk`, `status`, `created_at`, `updated_at`, `entity_type`, numeric scores, tier, source.

**Why these are cleartext**: Enables DynamoDB queries and GSI projections without decryption. Status-based queries (`status#open`), expert leaderboards (`total_score`), and tension tracking (`tension_status#open`) work without key access.

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| DynamoDB Local divergence | Pin image version, monitor AWS release notes |
| Key loss = data loss (local) | Clear documentation, `just nuke` for intentional reset |
| Docker requirement friction | `just setup` handles everything, clear error messages |
| Audit chain corruption | Daily verification, immutable storage, alerts |

## Non-Goals

- **SQLite fallback** - Rejected by design
- **Progressive disclosure tiers** - Rejected by user
- **Local key rotation** - Local data is disposable
- **Optional Docker** - Required for true parity

## Compliance

- **GDPR Article 17**: Key deletion = cryptographic erasure
- **SOC2 CC6.6**: Audit trail with hash chain, tamper-evident
- **Zero-Knowledge**: Same guarantee locally and in production

---

## Dialogue Provenance

This RFC was drafted through a 3-round ALIGNMENT dialogue achieving 100% convergence, then updated to implement the full RFC 0051 schema (14 entity types).

| Expert | Role | Key Contribution |
|--------|------|------------------|
| Cupcake | Security Architect | KeyProvider trait, local key story |
| Muffin | Platform Engineer | Docker infrastructure, justfile workflow |
| Palmier | QA Engineer | Test vector strategy (NIST + reference + property) |
| Cannoli | SRE Lead | Audit logging with hash chain + trace correlation |
| Eclair | Database Architect | Full RFC 0051 → DynamoDB single-table mapping |
| Strudel | Infrastructure Engineer | docker-compose.yml, first-time setup |
| Scone | DevEx Engineer | Progressive disclosure (rejected, informed final design) |
| Macaron | Startup CTO | Simplicity advocate (rejected, validated true parity need) |
| Croissant | Crypto Engineer | Key hierarchy, algorithm identity |
| Brioche | Secrets Engineer | LocalSecretsProvider pattern |

**ALIGNMENT Score**: 289 | **Rounds**: 3 | **Convergence**: 100%

**Post-Dialogue Update**: Schema expanded from simplified dialogue storage to full RFC 0051 implementation (dialogues, experts, rounds, verdicts, perspectives, tensions, recommendations, evidence, claims, moves, refs, events).

### Supersession Dialogue (2026-02-06)

A second 12-expert dialogue evaluated whether RFC 0058 should be superseded by a hybrid relational + DynamoDB architecture. Result: **Do NOT supersede.** Amend with three-phase sequencing, portable encryption envelope, and verdict denormalization elimination. See Amendment section above.

| Expert | Role | Key Contribution |
|--------|------|------------------|
| Strudel | Contrarian | Schema reframing: "the problem is the schema, not the engine" |
| Croissant | Rust Systems Engineer | ADR + PartitionScoped trait governance (91% compliance) |
| Galette | Developer Experience | "Building the trait IS the design decision" |
| Cannoli | Serverless Advocate | Full-partition load eliminates denormalization |
| Tartlet | Migration Specialist | Canonical entity address for AAD portability |
| Muffin | Relational Architect | Verdict immutability confirms denormalization removal |

**ALIGNMENT Score**: 415 | **Rounds**: 4 | **Convergence**: 100% (6/6 unanimous) | **Tensions**: 17/17 resolved

---

*"The code you test is the code you ship."*
