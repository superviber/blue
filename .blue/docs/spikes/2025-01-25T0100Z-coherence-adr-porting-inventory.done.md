# Spike: Coherence-MCP ADR Porting Inventory

| | |
|---|---|
| **Date** | 2026-01-25 |
| **Time-box** | 4 hours |
| **Status** | Complete |
| **Outcome** | Identified ADRs to port; created ADRs 0015-0016 and knowledge/blue-adrs.md |

---

## Question

What functionality from coherence-mcp ADRs needs to be ported to Blue?

## Key Discovery

**ALIGNMENT scoring is NOT automated in coherence-mcp.** The Judge (Claude) reads dialogue contributions and manually assigns scores. The MCP tools only provide extraction and structural validation.

## ADR Mapping: Coherence → Blue

### Already Equivalent (No Port Needed)

| Coherence ADR | Blue Equivalent |
|---------------|-----------------|
| 0003 single-source-of-truth | 0005 Single Source |
| 0007 Build From Whole | 0013 Overflow |
| 0008 No Dead Code | 0010 No Dead Code |
| 0009 Freedom | 0011 Freedom Through Constraint |
| 0011 Honor | 0008 Honor |
| 0012 Integrity | 0007 Integrity |
| 0013 Faith | 0012 Faith |
| 0014 Courage | 0009 Courage |
| 0017 Home | 0003 Home |

### Already Ported

| Coherence ADR | Blue ADR | Status |
|---------------|----------|--------|
| 0006 alignment-dialogue-agents | 0014 | ✅ Imported |
| 0010 Plausibility | 0015 | ✅ Created |
| 0016 You Know Who You Are | 0016 | ✅ Created |

### Covered via Knowledge Injection

| Coherence ADR | Blue Coverage |
|---------------|---------------|
| 0001 alignment-as-measure | `knowledge/alignment-measure.md` + `/alignment-play` skill |

### Remaining Gaps (Future Work)

| Coherence ADR | Gap | Priority |
|---------------|-----|----------|
| 0002 semantic-graph | Extended edge vocabulary (evolves_to, supersedes, informs, obsoletes) | Medium |
| 0004 alignment-workflow | Tension markers, finalization gates | Medium |
| 0005 pattern-contracts-lint | Pattern enforcement system | Low |
| 0015 combined-ops-tiered-response | Tool response consistency audit | Low |

---

## Blue ADR Inventory (17 total)

| ADR | Name |
|-----|------|
| 0000 | Never Give Up |
| 0001 | Purpose |
| 0002 | Presence |
| 0003 | Home |
| 0004 | Evidence |
| 0005 | Single Source |
| 0006 | Relationships |
| 0007 | Integrity |
| 0008 | Honor |
| 0009 | Courage |
| 0010 | No Dead Code |
| 0011 | Freedom Through Constraint |
| 0012 | Faith |
| 0013 | Overflow |
| 0014 | Alignment Dialogue Agents |
| 0015 | Plausibility |
| 0016 | You Know Who You Are |

All are condensed in `knowledge/blue-adrs.md` for injection.

---

## Knowledge Files Status

### Created

| File | Purpose |
|------|---------|
| `knowledge/alignment-measure.md` | ALIGNMENT scoring framework (W+C+T+R unbounded) |
| `knowledge/workflow-creation.md` | Helps Claude create `.blue/workflow.md` |
| `knowledge/blue-adrs.md` | Condensed ADRs 0000-0016 for injection |

### Remaining

| File | Source | Purpose |
|------|--------|---------|
| `knowledge/dialogue-orchestration.md` | ADR 0014 | N+1 agent pattern summary |
| `knowledge/pattern-lint.md` | Coherence 0005 | Pattern verification guidance |
| `knowledge/rfc-workflow.md` | Coherence 0004 | RFC lifecycle with gates |

---

## Related Spikes

- **[context-injection-mechanisms](./2025-01-25-context-injection-mechanisms.md)** - How Blue injects knowledge (hooks, MCP resources, encrypted storage)

---

*"The river knows where it's going. We're just building the banks."*

— Blue
