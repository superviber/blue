# ADR 0018: DynamoDB-Portable Schema Design

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-02-02 |

---

## Context

Blue currently uses SQLite for local storage. Future deployment scenarios require cloud-native storage (DynamoDB) for:
- Multi-region availability
- Serverless scaling
- Concurrent access from distributed services

## Decision

**All new schemas must be designed for DynamoDB portability.**

### Principles

1. **Single-table design** - One table per domain, composite sort keys encode hierarchy
2. **Partition key scoping** - All hot-path queries scoped to single partition key
3. **No cross-partition JOINs** - Denormalize or use application-level joins for cold paths
4. **Single writer per partition** - Avoid concurrent write contention within a partition
5. **Composite sort keys** - Encode type and hierarchy in sort key: `TYPE#subkey1#subkey2`

### Pattern

```
Table: blue_{domain}
───────────────────────────────────────────────
PK (partition key)  │ SK (sort key)           │ attributes...
───────────────────────────────────────────────
{parent_id}         │ META                    │ metadata
{parent_id}         │ CHILD#subkey            │ child record
{parent_id}         │ EVENT#timestamp         │ audit event
```

### SQLite Implementation

For local development, the single-table pattern maps to SQLite:

```sql
CREATE TABLE blue_{domain} (
  pk TEXT NOT NULL,
  sk TEXT NOT NULL,
  data JSON NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (pk, sk)
);
```

Or use separate tables with foreign keys (more idiomatic SQL) as long as:
- All queries can be scoped to a single `pk` value
- No JOINs required in hot paths

## Consequences

**Positive:**
- Seamless migration path to DynamoDB
- Predictable query patterns
- Natural sharding by partition key

**Negative:**
- Some denormalization required
- Less flexible ad-hoc queries
- Slightly more complex local schema

## Examples

### Dialogue Domain (RFC 0051)

```
PK: dialogue_id
SK: TYPE#subkey

nvidia-dec | META                  → {title, status, created_at}
nvidia-dec | EXPERT#muffin         → {role, tier, score}
nvidia-dec | PERSP#0#muffin#1      → {label, content, status}
nvidia-dec | TENSION#T01           → {description, status}
nvidia-dec | TEVENT#T01#1706900000 → {event_type, actor, reason}
```

### Document Domain (existing)

```
PK: realm_id
SK: TYPE#subkey

letemcook  | META                  → {name, path}
letemcook  | DOC#rfc#0051          → {title, status, content}
letemcook  | LINK#0051#0050        → {link_type}
```

---

*"Design for the cloud, develop on the laptop."*
