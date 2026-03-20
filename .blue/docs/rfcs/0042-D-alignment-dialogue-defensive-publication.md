# RFC 0042: Alignment Dialogue Defensive Publication

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-29 |
| **Dialogue** | [2026-01-29T2121Z-patentability-of-the-alignment-dialogue-game-system](../dialogues/2026-01-29T2121Z-patentability-of-the-alignment-dialogue-game-system.dialogue.recorded.md) |

---

## Summary

Establish formal prior art protection for the N+1 alignment dialogue architecture through defensive publication. This RFC documents the technical architecture explicitly as prior art, preventing competitor patents while preserving the collaborative ecosystem alignment that makes the system valuable.

**Recommendation: Defensive publication over patent prosecution** (12/12 expert consensus)

## Background

A 12-expert alignment dialogue deliberated the patentability of the N+1 alignment dialogue architecture described in ADR 0014. After 3 rounds and 509 total ALIGNMENT points, all experts unanimously converged on defensive publication as the superior strategy.

### Key Findings

1. **Technical Claims Identified:**
   - Parallel spawning mechanism eliminating first-mover bias through simultaneous context initialization
   - File-based protocol for round-scoped agent outputs (write-before-acknowledgment)
   - Convergence velocity detection across unbounded scoring dimensions
   - Session resumption without context pollution

2. **Patent Viability Assessment:**
   - Alice/Mayo § 101 risk: High but addressable via technical framing
   - Prior art density: Significant overlap with distributed systems (MapReduce, Raft, Paxos)
   - Novelty: Contested (LLM-specific constraints are new, file coordination is old)
   - Non-obviousness: Marginal (combination of known techniques)

3. **Strategic Analysis:**

| Factor | Patent | Defensive Pub |
|--------|--------|---------------|
| Cost | $15-30K prosecution | $0 (already achieved) |
| Timeline | 2-4 years | Immediate |
| Competitor blocking | Uncertain | Achieved |
| Future flexibility | Restricted | Unrestricted |
| Ecosystem alignment | Adversarial | Collaborative |
| Enforcement cost | $100K-1M+ | N/A |

## Technical Architecture (Prior Art Declaration)

The following technical architecture is hereby declared as prior art, released for public use, and explicitly not subject to patent protection:

### 1. N+1 Agent Architecture

A system for multi-agent deliberation comprising:
- **N expert agents**: Independent LLM sessions with isolated context windows
- **1 Judge agent**: Orchestrator that spawns, scores, and synthesizes
- **Parallel execution**: All N agents spawned simultaneously to eliminate first-mover bias

### 2. File-Based State Coordination Protocol

A method for coordinating stateless LLM sessions comprising:
- Each agent MUST write complete output to a dedicated file before acknowledgment
- Round-scoped directory structure: `round-N/{agent}.md`
- Judge reads all N files and merges without race conditions
- Enables session resumption and context window management

### 3. Convergence Detection Mechanism

An algorithmic method for determining deliberation completion comprising:
- Multi-dimensional scoring: Wisdom + Consistency + Truth + Relationships
- Unbounded dimensions: No upper limit, rewarding exceptional contributions
- Velocity calculation: Score delta between rounds
- Convergence criterion: Velocity approaches zero OR all tensions resolved

### 4. Perspective Integration Protocol

A structured format for agent contributions comprising:
- `[PERSPECTIVE Pnn: label]` — Novel viewpoints (2-4 sentences)
- `[TENSION Tn: description]` — Unresolved issues
- `[REFINEMENT/CONCESSION/RESOLVED]` — Engagement moves
- Perspective inventory tracking consensus emergence

## Rationale

### Why Defensive Publication > Patent

1. **GitHub ADR already establishes prior art** — ADR 0014 published with timestamps blocks competitor patents
2. **Distributed systems precedent** — File coordination patterns date to 1970s
3. **Cost/benefit unfavorable** — Prosecution costs exceed defensive value
4. **Enforcement impractical** — Software patents against well-funded competitors rarely succeed
5. **Philosophical alignment** — System designed for collaboration, not exclusion

### Resolved Tensions

All 13 tensions raised during deliberation were resolved:

| Tension | Resolution |
|---------|------------|
| T01-T11 | Moot under defensive publication strategy |
| T12: Prior art overlap | Split verdict; unanimous on strategy |
| T13: One-year bar | Already achieved via GitHub publication |

## Implementation

### Phase 1: Formalize Defensive Publication
- [x] Conduct expert deliberation on patentability
- [x] Document technical architecture explicitly as prior art
- [ ] Add explicit prior art declaration to ADR 0014
- [ ] Timestamp and hash this RFC for provenance

### Phase 2: Public Dissemination
- [ ] Publish technical whitepaper to arXiv or similar
- [ ] Submit to Defensive Patent License (DPL) registry
- [ ] Cross-reference in relevant academic literature

### Phase 3: Ecosystem Communication
- [ ] Blog post explaining architectural choices
- [ ] Open-source implementation documentation
- [ ] Community engagement on design decisions

## Test Plan

- [x] 12-expert alignment dialogue reached convergence (509 ALIGNMENT)
- [x] All tensions resolved
- [x] Unanimous recommendation achieved
- [ ] RFC reviewed and approved
- [ ] Prior art declaration added to ADR 0014

## References

- [ADR 0014: Alignment Dialogue Agents](../adrs/0014-alignment-dialogue-agents.accepted.md)
- [Dialogue Record](../dialogues/2026-01-29T2121Z-patentability-of-the-alignment-dialogue-game-system.dialogue.recorded.md)
- Alice Corp. v. CLS Bank International, 573 U.S. 208 (2014)
- 35 U.S.C. § 101, § 103

---

*"The architecture's value comes from collaboration, not exclusion. Defensive publication protects without restricting."*

— 💙 Judge + 🧁🧁🧁🧁🧁🧁🧁🧁🧁🧁🧁🧁
