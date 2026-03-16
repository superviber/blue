# Round 2 Summary: Local-Production Parity (True Parity)

**ALIGNMENT Score**: +243 (Velocity: +87, Total: 243) | **Panel**: 6 experts | **Status**: STRONG CONVERGENCE

## Constraint Acknowledgment

All experts accepted the new constraints without resistance:
- Docker REQUIRED
- DynamoDB Local mandatory
- No tiers, no fallbacks
- Greenfield Rust repo

## Converged Architecture

### 1. KeyProvider Trait (Cupcake)

```rust
#[async_trait]
pub trait KeyProvider: Send + Sync {
    async fn get_umk(&self) -> Result<UmkHandle, KeyError>;
    async fn derive_kek(&self, umk: &UmkHandle, context: &[u8]) -> Result<KekHandle, KeyError>;
    async fn generate_dek(&self) -> Result<(DekHandle, EncryptedDek), KeyError>;
    async fn decrypt_dek(&self, kek: &KekHandle, encrypted: &EncryptedDek) -> Result<DekHandle, KeyError>;
}
```

Two implementations:
- `LocalFileKeyProvider` - Reads UMK from `~/.blue/keys/umk.key`
- `AwsKmsKeyProvider` - UMK lives in KMS

### 2. Docker Infrastructure (Muffin, Strudel)

**Minimal stack**: DynamoDB Local only

```yaml
services:
  dynamodb:
    image: amazon/dynamodb-local:2.2.1
    command: "-jar DynamoDBLocal.jar -sharedDb -dbPath /data"
    ports: ["8000:8000"]
    volumes: [dynamodb-data:/data]
    healthcheck:
      test: ["CMD-SHELL", "curl -sf http://localhost:8000/shell/ || exit 1"]
```

**Developer workflow**:
```bash
just setup  # First time
just dev    # Daily
just test   # Same tests as CI
```

### 3. DynamoDB Schema (Eclair)

Single table design with encryption fields:
```
PK: dialogue#{dialogue_id}
SK: {entity_type}#{timestamp}#{entity_id}

Encryption Fields:
- content_encrypted: Binary (ciphertext)
- content_nonce: Binary (12 bytes)
- key_id: String
```

Same schema, same code, endpoint override only:
```rust
let endpoint = env::var("DYNAMODB_ENDPOINT")
    .unwrap_or_else(|_| "http://localhost:8000".to_string());
```

### 4. Test Strategy (Palmier)

Two-layer model:
- **Layer 1**: Pure unit tests (no I/O, `cargo test --lib`)
- **Layer 2**: Integration tests (Docker required, same as CI)

Test vector conformance:
```rust
pub const ENCRYPTION_VECTORS: &[TestVectors] = &[
    TestVectors {
        plaintext: b"alignment_score:0.85",
        key: [0x01; 32],
        expected_ciphertext: include_bytes!("vectors/score_encrypted.bin"),
    },
];
```

### 5. Observability (Cannoli)

Configuration-driven, not code-path-driven:
- Same metrics names and labels everywhere
- Same audit event schema
- Optional Jaeger/Prometheus locally (not required)
- Crypto audit logging always on

## Perspectives Registered

| ID | Perspective | Contributors |
|----|-------------|--------------|
| P0201 | KeyProvider trait with opaque handles | Cupcake |
| P0202 | Docker compose with justfile workflow | Muffin, Strudel |
| P0203 | Two-layer test architecture | Palmier |
| P0204 | Config-driven observability | Cannoli |
| P0205 | Single DynamoDB schema everywhere | Eclair |

## Tensions Resolved

| ID | Tension | Resolution |
|----|---------|------------|
| T0002 | Full parity vs ergonomics | **RESOLVED**: Docker is acceptable cost for true parity |
| T0004 | Docker requirement | **RESOLVED**: Required, not optional |
| T0006 | Greenfield scope | **RESOLVED**: Encrypted DynamoDB storage crate |

## Remaining Open Items

| ID | Item | Owner |
|----|------|-------|
| O0001 | Test vector generation (reference impl or derived?) | Palmier |
| O0002 | Key rotation story for local keys | Cupcake |
| O0003 | Crypto audit + trace context correlation | Cannoli |

## Convergence Check

**All experts signal CONVERGE:**
- Cupcake: [MOVE:CONVERGE] "The greenfield constraint lets us do encryption right"
- Muffin: [MOVE:CONVERGE] "Same endpoint variable switches local vs prod"
- Palmier: [MOVE:CONVERGE] "If your laptop runs green, production runs green"
- Cannoli: [MOVE:CONVERGE] "Zero divergence - configuration only"
- Eclair: [MOVE:CONVERGE] "Ready to draft ADR-0019"
- Strudel: [MOVE:CONVERGE] "Infrastructure is intentionally boring"

## Verdict: CONVERGENCE ACHIEVED

**Velocity approaching zero** - No new tensions raised, all experts align.

**Recommendation**: Draft final RFC synthesizing the converged architecture for the new `blue-encrypted-store` Rust crate.
