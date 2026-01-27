# Spike: Audit Path Integration

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-24 |
| **Time Box** | 30 minutes |

---

## Question

Does Blue MCP need updates for audit document paths and integration?

---

## Findings

### Current State

1. **`blue_audit` tool exists** but it's a health checker, not document management:
   - Checks for stalled RFCs (in-progress without worktree)
   - Finds implemented RFCs without ADRs
   - Detects overdue reminders
   - Identifies expired staging locks

2. **No `DocType::Audit`** in `blue-core/src/store.rs`:
   ```rust
   pub enum DocType {
       Rfc, Spike, Adr, Decision, Prd, Postmortem, Runbook, Dialogue
   }
   ```

3. **No audit document paths** - `BlueHome` doesn't define an audits directory

4. **Fungal has audit documents** in `docs/audits/`:
   - `2026-01-17-repository-audit.md` - Full repo audit report
   - `2026-01-17-rfc-status-verification.md` - RFC status verification

### Gap Analysis

| Feature | Status |
|---------|--------|
| DocType::Audit | ❌ Missing |
| `.blue/docs/audits/` path | ❌ Missing |
| `blue_audit_create` tool | ❌ Missing |
| `blue_audit_list` tool | ❌ Missing |

### Recommendation

**Yes, Blue MCP needs updates** to support audit documents as a first-class document type:

1. Add `DocType::Audit` to store.rs
2. Add `audits_path` to `BlueHome`
3. Create `blue_audit_create` tool for generating audit reports
4. Rename current `blue_audit` to `blue_health_check` to avoid confusion

---

*"Two audits with the same name. One checks health, one documents findings. Let's clarify."*

— Blue
