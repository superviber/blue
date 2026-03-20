# RFC 0057: DynamoDB with End-to-End Encryption and User-Controlled Keys

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-02 |
| **ADRs** | 0018 (DynamoDB-Portable Schema) |
| **RFCs** | 0053 (Storage Abstraction Layer) |
| **Dialogue** | ALIGNMENT score 214, 8 experts, 2 rounds to convergence |

---

## Summary

Implement client-side encryption for DynamoDB with user-controlled keys, enabling zero-knowledge architecture where Blue infrastructure never sees plaintext user data. Integrate Infisical for operational secrets while maintaining strict separation from user encryption keys.

## Problem

The current SQLite-based storage provides no encryption. As Blue moves to DynamoDB for cloud deployment:

1. **Data at rest** must be encrypted before reaching DynamoDB
2. **Data in flight** must be encrypted beyond TLS
3. **Users must control their own keys** - Blue should be cryptographically incapable of accessing user data
4. **Secrets management** needs centralization via Infisical
5. **Offline-first** development must work without cloud dependencies

## Architecture

### Zero-Knowledge Guarantee

```
┌─────────────────── USER DOMAIN (never leaves) ───────────────────┐
│  User Master Key (UMK)                                           │
│  ├── Generated locally: `blue crypto init`                       │
│  ├── Stored in: user's password manager / HSM / secure location  │
│  └── NEVER touches Blue infrastructure                           │
│        │                                                         │
│        ▼ (HKDF derivation)                                       │
│  Key Encryption Key (KEK)                                        │
│  ├── Per-tenant, derived from UMK + tenant_id + epoch            │
│  └── Cached in memory during session                             │
│        │                                                         │
│        ▼ (wraps)                                                 │
│  Data Encryption Keys (DEKs)                                     │
│  ├── Per-partition (per-dialogue in Blue's case)                 │
│  ├── Wrapped DEK stored alongside ciphertext                     │
│  └── AES-256-GCM with AAD binding to pk+sk                       │
└──────────────────────────────────────────────────────────────────┘
                              │
                              │ (only ciphertext + wrapped DEKs)
                              ▼
┌─────────────────── BLUE INFRASTRUCTURE ──────────────────────────┐
│  Infisical (T2/T3 secrets only)                                  │
│  ├── AWS role ARNs, API endpoints                                │
│  ├── Key metadata (version, rotation schedule)                   │
│  └── NEVER user encryption key material (T1)                     │
│                                                                  │
│  DynamoDB                                                        │
│  ├── Cleartext: pk, sk, tenant_id, created_at (for queries)     │
│  ├── Encrypted: encrypted_data (single blob)                     │
│  └── Metadata: key_id, encryption_version, nonce, aad_hash       │
│                                                                  │
│  Cryptographic Audit Log                                         │
│  ├── All key operations logged (creation, rotation, destruction) │
│  ├── Immutable, 7-year retention (SOC2)                          │
│  └── Exportable for external audit                               │
└──────────────────────────────────────────────────────────────────┘
```

### Three-Tier Key Hierarchy

```rust
/// User Master Key - User-controlled, never leaves client
pub struct UserMasterKey {
    material: Zeroizing<[u8; 32]>,
    // Stored in: user's password manager, HSM, or local keyring
    // NEVER in Infisical or any Blue infrastructure
}

/// Key Encryption Key - Derived per-tenant with epoch for revocation
pub struct KeyEncryptionKey {
    material: Zeroizing<[u8; 32]>,
    tenant_id: String,
    epoch: u64,  // Incremented on revocation for key invalidation
}

impl KeyEncryptionKey {
    pub fn derive(umk: &UserMasterKey, tenant_id: &str, epoch: u64) -> Self {
        let epoch_bytes = epoch.to_le_bytes();
        let salt = format!("blue:kek:{}:{}", tenant_id, epoch);
        let material = hkdf_sha256(&umk.material, salt.as_bytes());
        Self { material, tenant_id: tenant_id.to_string(), epoch }
    }
}

/// Data Encryption Key - Per-partition (per-dialogue)
pub struct DataEncryptionKey {
    material: Zeroizing<[u8; 32]>,
    partition_id: String,
    version: u32,
}
```

### Secret Classification (T1/T2/T3)

| Tier | Classification | Examples | Storage | Blue Access |
|------|----------------|----------|---------|-------------|
| **T1** | Critical | User MEK, DEKs, KEKs | User-controlled only | **NEVER** |
| **T2** | Sensitive | AWS role ARNs, service tokens | Infisical | Read-only |
| **T3** | Config | Table prefixes, regions | Infisical or config | Read-only |

### DynamoDB Item Schema

```json
{
  "pk": "dialogue#<dialogue-id>",           // Cleartext for routing
  "sk": "PERSP#<round>#<expert>#<seq>",     // Cleartext for range queries
  "entity_type": "perspective",              // Cleartext for GSI
  "tenant_id": "tenant-<uuid>",             // Cleartext for isolation
  "created_at": "2026-02-02T12:00:00Z",     // Cleartext for TTL/queries
  "updated_at": "2026-02-02T14:30:00Z",     // Cleartext for sync

  "encrypted_data": "<base64-ciphertext>",  // AES-256-GCM encrypted blob
  "key_id": "dek#<dialogue-id>#v<version>", // DEK reference for rotation
  "encryption_version": 1,                   // Schema version
  "nonce": "<base64-12-bytes>",             // Unique per-encryption
  "aad_hash": "<base64-sha256>"             // Hash of pk||sk for integrity
}
```

## Key Decisions (Resolved by ALIGNMENT Dialogue)

### 1. Infisical Integration

**Resolution**: Two-domain architecture with strict separation.

- **Infisical manages**: T2/T3 secrets (service credentials, API endpoints, key metadata)
- **Infisical NEVER manages**: T1 secrets (user encryption keys)
- **Workspace-per-client**: Each client gets isolated Infisical workspace

### 2. Disaster Recovery

**Resolution**: Blue holds ZERO Shamir shares by default.

```
Default (Self-Sovereign):
  - User generates 3-of-5 Shamir shares
  - All shares user-controlled (password manager, hardware key, paper, etc.)
  - Blue provides tooling only, holds no key material

Optional (Assisted Recovery) - Explicit Opt-In:
  - User may grant Blue ONE share (not enough alone)
  - Labeled "Managed Recovery" not "Zero-Knowledge"
  - 72-hour retrieval delay with notifications
```

### 3. Key Mode Selection

**Resolution**: User's choice, not Blue's mandate.

```rust
pub enum KeyMode {
    /// Local keys, no cloud dependency (privacy-maximalist)
    Local { passphrase_derived: bool },

    /// AWS KMS in user's account (enterprise convenience)
    AwsKms { key_arn: String },

    /// Bring Your Own Key (regulated industries)
    Byok { source: ByokSource },
}
```

### 4. Progressive Key Sovereignty

**Resolution**: Four-tier model for onboarding to production.

| Tier | Name | Use Case | Key Setup | Blue Access |
|------|------|----------|-----------|-------------|
| 0 | Ephemeral Dev | Local development | None (auto-gen) | N/A |
| 1 | Sovereign Lite | Real usage default | `blue crypto init` | Never |
| 2 | Managed Sovereign | Opt-in recovery | User + recovery shard | Cannot alone |
| 3 | Enterprise | Organization-managed | IT procedures | Never direct |

### 5. Offline-First with Revocation

**Resolution**: Epoch-based key derivation.

```rust
// KEK derivation includes epoch - different epoch = different keys
let kek = derive_kek(&umk, &tenant_id, epoch);

// Revocation: increment epoch in Infisical
// - Online clients get new epoch, old keys stop working
// - Offline clients use cached epoch, accept delayed revocation
```

### 6. Key Export Security

**Resolution**: Allow with defense-in-depth controls.

1. **Rate limiting**: 3 exports per 24 hours
2. **MFA required**: Passphrase + TOTP/hardware key
3. **Time delay**: 24-hour default with notification
4. **Audit trail**: Immutable, user-visible export history
5. **Encrypted format**: Password-protected archive default

### 7. Zero-Knowledge vs Compliance

**Resolution**: Audit logs for operations, not content.

**Auditors CAN see**:
- Cryptographic operation logs (key_id, timestamp, success/failure)
- Key lifecycle documentation
- Architecture diagrams
- Encryption algorithm attestations

**Auditors CANNOT see**:
- Plaintext user data
- Encryption keys
- Decrypted content

### 8. GDPR Erasure

**Resolution**: Key deletion = cryptographic erasure (legally sufficient).

```rust
pub struct ErasureCertificate {
    pub certificate_id: Uuid,
    pub data_subject: String,
    pub statement: String,  // "All data encrypted with [keys] is now cryptographically inaccessible"
    pub algorithm_attestation: String,  // "AES-256-GCM, NIST-approved"
    pub key_destruction_log_reference: String,
}
```

### 9. Search

**Resolution**: Client-side only with local SQLite FTS5.

- Server-side search would break zero-knowledge
- Local index populated as documents are decrypted
- Each device maintains its own search index

## Implementation

### Storage Trait Extension

```rust
/// Encryption wraps storage, not the other way around
pub struct EncryptedStore<S: Store<Vec<u8>>, K: KeyProvider> {
    inner: S,
    key_provider: K,
    audit_emitter: Arc<dyn AuditEmitter>,
}

#[async_trait]
impl<S, K> Store<EncryptedPayload> for EncryptedStore<S, K>
where
    S: Store<Vec<u8>>,
    K: KeyProvider,
{
    async fn put(&self, item: &EncryptedPayload) -> Result<(), Self::Error> {
        let dek = self.key_provider.get_dek(&item.partition_id).await?;
        let aad = format!("{}||{}", item.pk, item.sk);
        let encrypted = aes_gcm_encrypt(&item.data, &dek, &aad)?;

        self.audit_emitter.emit(CryptoAuditEvent::encrypt(&item.key_id));
        self.inner.put(&encrypted).await
    }
}
```

### KeyProvider Trait

```rust
#[async_trait]
pub trait KeyProvider: Send + Sync {
    async fn get_dek(&self, partition_id: &str) -> Result<DataEncryptionKey, KeyError>;
    async fn wrap_dek(&self, dek: &DataEncryptionKey) -> Result<WrappedKey, KeyError>;
    async fn unwrap_dek(&self, wrapped: &WrappedKey) -> Result<DataEncryptionKey, KeyError>;
    fn security_level(&self) -> SecurityLevel;
}

pub enum SecurityLevel {
    Development,  // Local/ephemeral keys
    Production,   // Cloud-backed or local-file
    Regulated,    // HSM-backed, BYOK
}
```

### Auto-Detection for Local Development

```rust
impl KeyProvider {
    pub fn auto_detect() -> Self {
        if std::env::var("BLUE_PRODUCTION").is_ok() {
            Self::production_or_panic()
        } else if std::env::var("BLUE_UNSAFE_NO_ENCRYPTION").is_ok() {
            warn!("Running WITHOUT encryption");
            Self::passthrough()
        } else {
            // Default: SimulatedZeroKnowledge with auto-generated key
            let key_path = dirs::home_dir().unwrap().join(".blue/auto.key");
            Self::simulated_zero_knowledge(key_path)
        }
    }
}
```

## Migration Path

### Phase 1: Traits & Local Provider
- [ ] Define `KeyProvider` trait
- [ ] Implement `LocalKeyProvider` (passphrase-derived)
- [ ] Implement `EncryptedStore<S, K>` wrapper
- [ ] Add `blue crypto init/rotate/export` CLI commands

### Phase 2: DynamoDB Integration
- [ ] Implement `DynamoDialogueStore` with encryption
- [ ] Add key table schema for wrapped KEKs/DEKs
- [ ] Implement lazy re-encryption for key rotation
- [ ] Integration tests with DynamoDB Local

### Phase 3: Infisical Integration
- [ ] Implement `InfisicalSecretProvider` for T2/T3
- [ ] Add workspace-per-client provisioning
- [ ] Implement epoch-based revocation
- [ ] Add secret rotation workflows

### Phase 4: Compliance & Audit
- [ ] Implement cryptographic audit log
- [ ] Add GDPR erasure certification
- [ ] Create SOC2 documentation package
- [ ] External security assessment

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Key loss = data loss | Shamir tooling, clear documentation, optional assisted recovery |
| Offline revocation delay | Epoch-based derivation, cached epoch with TTL |
| Query pattern leakage | Accept documented tradeoff (metadata visible, content protected) |
| DAX caching incompatible | No DAX for encrypted data, app-level caching post-decryption |

## Non-Goals

- **Server-side search**: Breaks zero-knowledge
- **Blue-managed keys by default**: Users must control their keys
- **Automatic key recovery**: User responsibility by design
- **Per-item DEKs**: Per-partition (per-dialogue) is sufficient

## Compliance

- **GDPR Article 17**: Key deletion = cryptographic erasure
- **GDPR Article 25**: Privacy by design (client-side encryption)
- **SOC2 CC6.6**: Key lifecycle documented, audit trails immutable
- **Zero-Knowledge**: Blue cannot produce plaintext, cannot be compelled

---

## Dialogue Provenance

This RFC was drafted through a 2-round ALIGNMENT dialogue with 8 expert agents:

| Expert | Role | Key Contribution |
|--------|------|------------------|
| Muffin | Cryptography Engineer | Three-tier hierarchy, epoch-based revocation |
| Cupcake | Cloud Security Architect | KeyProvider trait, Shamir distribution |
| Scone | Secrets Management | T1/T2/T3 classification, Infisical separation |
| Eclair | DynamoDB Architect | Split attribute model, AAD binding |
| Donut | Platform Security | Progressive key sovereignty, export controls |
| Brioche | Developer Experience | SimulatedZeroKnowledge mode, auto-detection |
| Croissant | Compliance Officer | Audit visibility boundary, GDPR erasure |
| Macaron | Privacy Advocate | Zero-knowledge validation, no Blue shares |

**ALIGNMENT Score**: 214 | **Rounds**: 2 | **Convergence**: 100%

---

*"Cannot betray because cannot access."*
