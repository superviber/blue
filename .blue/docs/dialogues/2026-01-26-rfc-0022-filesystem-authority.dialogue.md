# Alignment Dialogue: RFC 0022 Filesystem Authority

**RFC**: [0022-filesystem-authority](../rfcs/0022-filesystem-authority.md)
**Experts**: 12
**Rounds**: 2
**Final Convergence**: 94%

---

## Problem Statement

Four RFCs (0017, 0018, 0020, 0021) all establish the same principle: filesystem is truth, database is derived index. Should they be consolidated?

---

## Round 1: Initial Positions

| Expert | Position | Confidence | Key Insight |
|--------|----------|------------|-------------|
| Database Architect | ALIGN | 85% | Event sourcing pattern - filesystem is event store |
| Filesystem Engineer | PARTIAL | 78% | RFC 0020 is orthogonal - link rendering, not authority |
| Distributed Systems | ALIGN | 85% | Consolidation prevents implementation drift |
| DX Advocate | ALIGN | 85% | One principle, one document |
| Rustacean | ALIGN | 88% | Trait abstraction: `DerivedFromFilesystem` |
| ADR Guardian | ALIGN | 92% | Four RFCs = shadow copies violating ADR 0005 |
| Performance Engineer | ALIGN | 85% | Unified cache layer, single filesystem scan |
| Git Workflow | ALIGN | 92% | Filesystem-as-truth aligns with git model |
| Minimalist | PARTIAL | 78% | RFC 0020 doesn't belong - presentation concern |
| Documentation | ALIGN | 85% | One comprehensive RFC easier to document |
| Testing/QA | ALIGN | 88% | Cross-feature edge cases need unified testing |
| Devil's Advocate | DISSENT | 72% | Different lifecycle velocities; mega-RFC anti-pattern |

**Round 1 Convergence**: 75% (9 ALIGN, 2 PARTIAL, 1 DISSENT)

### Key Tension: RFC 0020

5 experts flagged RFC 0020 (Source Link Resolution) as not belonging:
- "It's about presentation, not truth"
- "It's about link rendering, not authority"
- "Different lifecycle velocity"

---

## Round 2: Modified Proposal

**Proposal**: Consolidate 0017, 0018, 0021 into RFC 0022. Keep RFC 0020 separate.

### Synthesis Response

> "Authority RFCs (0017, 0018, 0021) ask 'what is the source of truth?' RFC 0020 asks 'how do we format references to it?' These are orthogonal concerns."

| Aspect | Assessment |
|--------|------------|
| Addresses key concern | Yes - RFC 0020's distinct nature recognized |
| Cleaner architecture | Yes - 3+1 is cleaner than 4 consolidated or 4 separate |
| ADR alignment | Yes - Single Source honored, No Dead Code honored |

**Round 2 Convergence**: 94% ALIGN

---

## Final Architecture

```
RFC 0022: Filesystem Authority (consolidated)
├── Plan File Authority (ex-0017)
├── Document Import/Sync (ex-0018)
└── Filesystem-Aware Numbering (ex-0021)

RFC 0020: Source Link Resolution (separate)
└── References RFC 0022 for path resolution context
```

---

## Key Consensus Points

1. **Filesystem is truth, database is derived** - one principle stated once
2. **Rebuild-on-read pattern** - universal across document types
3. **Staleness detection** - mtime fast path, hash slow path
4. **RFC 0020 is presentation** - not authority; stays separate
5. **Gitignore blue.db** - it's a generated artifact

---

## Expert Contributions

### Database Architect
> "This is the Event Sourcing pattern. The filesystem becomes the event store; the database becomes a read model that can be rebuilt from source."

### ADR Guardian
> "Four RFCs expressing one idea is architectural dead code. 'Delete boldly. Git remembers.'"

### Minimalist
> "Minimalism isn't about fewer documents; it's about each document doing one thing well."

### Devil's Advocate (valuable dissent)
> "Would you have written one RFC covering all four topics from scratch? Or did you write four because they were four different problems?"

---

*Convergence achieved. RFC 0022 drafted. RFC 0020 remains separate.*
