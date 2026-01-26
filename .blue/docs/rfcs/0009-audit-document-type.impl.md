# RFC 0009: Audit Document Type

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-01-24 |
| **Source Spike** | audit-path-integration |

---

## Summary

Add Audit as a first-class document type in Blue. Rename the existing `blue_audit` health checker to `blue_health_check` to eliminate naming collision.

## Problem

Two distinct concepts share the name "audit":

1. **Health checks** - The current `blue_audit` tool scans for stalled RFCs, overdue reminders, expired locks
2. **Audit documents** - Formal reports documenting findings (security audits, repository audits, RFC verification)

This collision violates ADR 0005 (Single Source) and ADR 0007 (Integrity). Names should mean one thing.

### Evidence

Fungal-image-analysis has audit documents with no Blue integration:
```
docs/audits/
├── 2026-01-17-repository-audit.md
└── 2026-01-17-rfc-status-verification.md
```

These are valuable artifacts with no home in `.blue/docs/`.

## Proposal

### 1. Add DocType::Audit

```rust
// store.rs
pub enum DocType {
    Rfc,
    Spike,
    Adr,
    Decision,
    Prd,
    Postmortem,
    Runbook,
    Dialogue,
    Audit,  // NEW
}
```

### 2. Add audits path to BlueHome

```rust
// repo.rs
pub struct BlueHome {
    pub root: PathBuf,
    pub docs_path: PathBuf,
    pub db_path: PathBuf,
    pub worktrees_path: PathBuf,
    pub audits_path: PathBuf,  // NEW: .blue/docs/audits
}
```

### 3. Rename blue_audit → blue_health_check

| Old | New |
|-----|-----|
| `blue_audit` | `blue_health_check` |

The health check tool remains unchanged in functionality—just renamed for clarity.

### 4. Add Audit Document Tools

| Tool | Purpose |
|------|---------|
| `blue_audit_create` | Create a new audit document |
| `blue_audit_list` | List audit documents |
| `blue_audit_get` | Retrieve an audit by title |

### 5. Audit Document Structure

```markdown
# Audit: {Title}

| | |
|---|---|
| **Status** | In Progress / Complete |
| **Date** | YYYY-MM-DD |
| **Type** | repository / security / rfc-verification / custom |
| **Scope** | What was audited |

---

## Executive Summary

Brief findings overview.

## Findings

Detailed findings with severity ratings.

## Recommendations

Actionable next steps.

---

*Audited by Blue*
```

### 6. Audit Types

| Type | Purpose |
|------|---------|
| `repository` | Full codebase health assessment |
| `security` | Security-focused review |
| `rfc-verification` | Verify RFC statuses match reality |
| `adr-adherence` | Check code follows ADR decisions |
| `custom` | User-defined audit scope |

## Non-Goals

- Automated audit generation (that's a separate RFC)
- Integration with external audit tools
- Compliance framework mappings (SOC2, etc.)

## Test Plan

- [x] `DocType::Audit` added to store.rs
- [x] Audits stored in `.blue/docs/audits/` (uses docs_path + "audits/")
- [x] `blue_health_check` replaces `blue_audit`
- [x] `blue_audit_create` generates audit document
- [x] `blue_audit_list` returns audit documents
- [x] `blue_audit_get` retrieves audit by title
- [x] `blue_audit_complete` marks audit as complete
- [ ] Existing fungal audits portable to new structure (manual migration)

## Implementation Plan

- [x] Add `DocType::Audit` to store.rs with `as_str()` and `from_str()`
- [x] Create `Audit`, `AuditType`, `AuditFinding`, `AuditSeverity` in documents.rs
- [x] Add `Audit::to_markdown()` for document generation
- [x] Rename `blue_audit` → `blue_health_check` in server.rs
- [x] Create handlers/audit_doc.rs for audit document tools
- [x] Register new tools in server.rs (4 tools)
- [x] Add unit tests

## Migration

Existing `blue_audit` callers will get a deprecation notice pointing to `blue_health_check`. The old name will work for one release cycle.

---

*"A name collision is a lie waiting to confuse. We fix it now."*

— Blue
