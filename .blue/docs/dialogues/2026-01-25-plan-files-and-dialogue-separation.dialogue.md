# Dialogue 0001: Plan Files And Dialogue Separation

| | |
|---|---|
| **Date** | 2026-01-25 19:27 |
| **Status** | Converged |
| **Experts** | 12 |
| **Rounds** | 2 |

---

## Summary

12-expert alignment dialogue on:
- **Proposal A**: Adopting `.plan.md` companion files for RFC task tracking
- **Proposal B**: Separating dialogue content from scoreboard/perspectives/tensions

## Alignment Scoreboard

### Proposal A: Plan Files

| Expert | R1 | R2 | Final |
|--------|----|----|-------|
| Systems Architect | SUPPORT | SUPPORT | SUPPORT |
| Token Economist | SUPPORT | SUPPORT | SUPPORT |
| Data Integrity | OPPOSE | **SUPPORT** | SUPPORT |
| Minimalist | OPPOSE | **SUPPORT** | SUPPORT |
| Documentation | OPPOSE | **SUPPORT** | SUPPORT |
| API Designer | OPPOSE | **SUPPORT** | SUPPORT |
| Developer Experience | SUPPORT | SUPPORT | SUPPORT |
| Git Workflow | SUPPORT | SUPPORT | SUPPORT |
| Performance Engineer | SUPPORT | SUPPORT | SUPPORT |
| Workflow Orchestrator | SUPPORT | SUPPORT | SUPPORT |
| Migration Specialist | SUPPORT | SUPPORT | SUPPORT |
| Observability Engineer | SUPPORT | SUPPORT | SUPPORT |

**Convergence: 12/12 = 100%**

### Proposal B: Dialogue Separation

| Expert | R1 | R2 | Final |
|--------|----|----|-------|
| Systems Architect | SUPPORT | SUPPORT | SUPPORT |
| Token Economist | SUPPORT | **OPPOSE** | OPPOSE |
| Data Integrity | NEUTRAL | NEUTRAL | NEUTRAL |
| Minimalist | OPPOSE | OPPOSE | OPPOSE |
| Documentation | NEUTRAL | **SUPPORT** | SUPPORT |
| API Designer | OPPOSE | **SUPPORT** | SUPPORT |
| Developer Experience | OPPOSE | **SUPPORT** | SUPPORT |
| Git Workflow | SUPPORT | SUPPORT | SUPPORT |
| Performance Engineer | SUPPORT | **NEUTRAL** | NEUTRAL |
| Workflow Orchestrator | SUPPORT | SUPPORT | SUPPORT |
| Migration Specialist | SUPPORT | SUPPORT | SUPPORT |
| Observability Engineer | SUPPORT | SUPPORT | SUPPORT |

**Convergence: 8 SUPPORT / 2 NEUTRAL / 2 OPPOSE = 67%**

---

## Tensions Identified

| ID | Tension | Status |
|----|---------|--------|
| T1 | Single Source of Truth - plan.md vs SQLite authority | Resolved R2 |
| T2 | Complexity vs Efficiency - file count vs token savings | Resolved R2 |
| T3 | Dialogue Integrity - scoreboard as metadata vs as content | Partially Resolved |

---

## Rounds

| Round | Topic | Outcome |
|-------|-------|---------|
| 1 | Initial positions | A: 67% B: 50% |
| 2 | Synthesis after tensions | A: 100% B: 67% |

### Round 1 Summary

**Proposal A (Plan Files):** 8/12 SUPPORT (67%)
- Supporters cited: token efficiency (60-80% reduction), cleaner git diffs, surgical commits
- Opponents cited: file count doubling, drift risk, fragmented knowledge

**Proposal B (Dialogue Separation):** 6/12 SUPPORT (50%)
- Supporters cited: 50KB waste loading full dialogue for 3 exchanges
- Opponents cited: scoreboard IS the dialogue, not metadata about it

### Round 2 Synthesis

**Key Shifts:**

1. **Data Integrity** (OPPOSE → SUPPORT A): Inversion resolves drift. If `.plan.md` is authoritative and SQLite is rebuild-on-read, drift is transient, not persistent.

2. **Minimalist** (OPPOSE → SUPPORT A): Performance evidence compelling. Decoupling plan state from content solves real I/O problems.

3. **Documentation** (OPPOSE → SUPPORT A): Ephemeral framing resolves cohesion concern. Plans are scaffolding, not documentation.

4. **Token Economist** (SUPPORT → OPPOSE B): File separation not needed. Selective extraction within single file achieves same token efficiency without fragmenting narrative.

5. **Performance Engineer** (SUPPORT → NEUTRAL B): File separation guarantees selective loading; section extraction requires implementation discipline.

---

## Converged Recommendations

### Proposal A: Plan Files - APPROVED

**Architecture:**
- `.plan.md` becomes **authoritative source** for task state
- SQLite becomes **derived index** (rebuilt from plan on read)
- Existing API (`blue_rfc_plan`, `blue_rfc_task_complete`) unchanged
- Atomic updates via status gate: `updating-plan` → write → clear

**Guardrails (from Git Workflow Expert):**
- Status gating: Plans only for `accepted` or `in-progress` RFCs
- Maximum 3 companion files per RFC (plan, test-plan, architecture notes)
- Each file serves single responsibility

### Proposal B: Dialogue Separation - ALTERNATIVE APPROACH

**Original proposal (file separation) did not converge.**

**Emerging alternative (67% implicit support):**
- Keep dialogue as **single file** (preserves narrative cohesion)
- Use **selective extraction** via header markers or offset-based reading
- Generated summaries serve as "metrics endpoint" for quick observability
- Tools load only current round + scoreboard section, not full history

**Key insight from Token Economist:**
> "File separation breaks the narrative artifact. Selective extraction leaves the artifact whole while optimizing access patterns."

---

## Lessons Learned

- **Inversion resolves integrity concerns**: Making plan files authoritative (not derived) eliminates drift risk entirely
- **Goal vs Method**: Experts agreed on token efficiency goal but split on method (file separation vs selective extraction)
- **Ephemeral framing**: Distinguishing "operational state" from "documentation" resolved documentation cohesion objections
- **Scaffolding metaphor**: Plans are like scaffolding - essential during construction, removed after completion

---

*"Right then. Let's build something good."*

— Blue
