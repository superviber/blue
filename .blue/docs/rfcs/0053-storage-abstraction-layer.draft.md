# RFC 0053: Storage Abstraction Layer

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-02-02 |
| **ADRs** | 0018 (DynamoDB-Portable Schema) |
| **Blocks** | DynamoDB migration |

---

## Summary

Introduce a trait-based storage abstraction to enable swapping SQLite (local dev) for DynamoDB (production) without changing application code.

## Problem

The current `DocumentStore` is tightly coupled to `rusqlite::Connection`:

```rust
pub struct DocumentStore {
    conn: Connection,  // ← Direct SQLite dependency
}
```

This makes it impossible to:
- Run integration tests against DynamoDB
- Deploy to serverless environments
- Use alternative backends (Turso, PlanetScale, etc.)

## Design

### Core Traits

```rust
/// Core storage operations for any entity
#[async_trait]
pub trait Store<T>: Send + Sync {
    type Error: std::error::Error;

    async fn get(&self, pk: &str, sk: &str) -> Result<Option<T>, Self::Error>;
    async fn put(&self, item: &T) -> Result<(), Self::Error>;
    async fn delete(&self, pk: &str, sk: &str) -> Result<(), Self::Error>;
    async fn query(&self, pk: &str, sk_prefix: &str) -> Result<Vec<T>, Self::Error>;
}

/// Dialogue-specific operations
#[async_trait]
pub trait DialogueStore: Send + Sync {
    async fn create_dialogue(&self, dialogue: &Dialogue) -> Result<String, StoreError>;
    async fn get_dialogue(&self, id: &str) -> Result<Option<Dialogue>, StoreError>;
    async fn register_perspective(&self, p: &Perspective) -> Result<(), StoreError>;
    async fn register_tension(&self, t: &Tension) -> Result<(), StoreError>;
    async fn update_tension_status(&self, id: &TensionId, status: TensionStatus, event: &TensionEvent) -> Result<(), StoreError>;
    async fn export_dialogue(&self, id: &str) -> Result<DialogueExport, StoreError>;
    async fn list_dialogues(&self, status: Option<DialogueStatus>) -> Result<Vec<DialogueSummary>, StoreError>;
}

/// Document-specific operations (existing functionality)
#[async_trait]
pub trait DocumentStore: Send + Sync {
    async fn add_document(&self, doc: &Document) -> Result<i64, StoreError>;
    async fn get_document(&self, id: i64) -> Result<Option<Document>, StoreError>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Document>, StoreError>;
    // ... existing methods
}
```

### Implementations

```rust
// Local development
pub struct SqliteDialogueStore {
    conn: Connection,
}

impl DialogueStore for SqliteDialogueStore { ... }

// Production (future)
pub struct DynamoDialogueStore {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
}

impl DialogueStore for DynamoDialogueStore { ... }
```

### Configuration

```toml
# .blue/config.toml

[storage]
backend = "sqlite"  # or "dynamodb"

[storage.sqlite]
path = ".blue/blue.db"

[storage.dynamodb]
table_prefix = "blue_"
region = "us-east-1"
```

### Factory Pattern

```rust
pub fn create_dialogue_store(config: &StorageConfig) -> Box<dyn DialogueStore> {
    match config.backend {
        Backend::Sqlite => Box::new(SqliteDialogueStore::open(&config.sqlite.path)?),
        Backend::DynamoDB => Box::new(DynamoDialogueStore::new(&config.dynamodb)?),
    }
}
```

## Migration Path

### Phase 1: Define Traits (this RFC)
- [ ] Define `DialogueStore` trait
- [ ] Define `DocumentStore` trait
- [ ] Define common types (`StoreError`, IDs, etc.)

### Phase 2: SQLite Implementation
- [ ] Implement `SqliteDialogueStore`
- [ ] Refactor existing `DocumentStore` to implement trait
- [ ] Add integration tests with trait bounds

### Phase 3: DynamoDB Implementation
- [ ] Implement `DynamoDialogueStore`
- [ ] Add DynamoDB Local for integration tests
- [ ] Performance benchmarks

### Phase 4: Configuration & Factory
- [ ] Add storage config to `.blue/config.toml`
- [ ] Implement factory pattern
- [ ] Feature flags for optional DynamoDB dependency

## Scope Boundaries

**In scope:**
- Dialogue storage (RFC 0051)
- Document storage (existing)
- Link storage (relationships)

**Out of scope (for now):**
- File index (semantic search) - complex embeddings, defer
- Session state - may use different patterns
- Caching layer - separate concern

## Test Strategy

```rust
// Generic test that works with any backend
async fn test_dialogue_roundtrip<S: DialogueStore>(store: &S) {
    let dialogue = Dialogue { title: "Test".into(), .. };
    let id = store.create_dialogue(&dialogue).await?;
    let retrieved = store.get_dialogue(&id).await?;
    assert_eq!(retrieved.unwrap().title, "Test");
}

#[tokio::test]
async fn test_sqlite_roundtrip() {
    let store = SqliteDialogueStore::open_in_memory()?;
    test_dialogue_roundtrip(&store).await;
}

#[tokio::test]
#[ignore]  // Requires DynamoDB Local
async fn test_dynamo_roundtrip() {
    let store = DynamoDialogueStore::new_local()?;
    test_dialogue_roundtrip(&store).await;
}
```

## Dependencies

```toml
# Cargo.toml

[dependencies]
async-trait = "0.1"

[dependencies.aws-sdk-dynamodb]
version = "1.0"
optional = true

[features]
default = ["sqlite"]
sqlite = ["rusqlite"]
dynamodb = ["aws-sdk-dynamodb", "aws-config"]
```

## Risks

1. **Async migration** - Current code is sync; traits are async for DynamoDB compatibility
2. **Transaction semantics** - SQLite has ACID; DynamoDB has different guarantees
3. **Query flexibility** - Some SQLite queries won't map cleanly to DynamoDB

## Non-Goals

- Automatic schema migration between backends
- Real-time sync between SQLite and DynamoDB
- Supporting arbitrary SQL databases (Postgres, MySQL)

---

*"Same interface, different engine."*
