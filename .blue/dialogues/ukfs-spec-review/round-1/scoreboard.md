# Round 1 Scoreboard

## Expert Contributions

| Expert | Role | Tier | W | C | T | R | Score | Key Contribution |
|--------|------|------|---|---|---|---|-------|------------------|
| Brioche | Systems Architect | Core | 14 | 12 | 10 | 9 | **45** | Epoch-scoped determinism, state-as-property |
| Beignet | Library Scientist | Wildcard | 12 | 10 | 9 | 8 | **39** | X-prefix extension, length-prefix notation |
| Muffin | Filesystem Architect | Core | 13 | 11 | 9 | 10 | **43** | Locked naming + relation hybrid model |
| Cannoli | Graph Theorist | Adjacent | 12 | 11 | 8 | 9 | **40** | Converged with Muffin on directory-per-relation |
| Eclair | DevEx Lead | Core | 11 | 10 | 8 | 8 | **37** | Dual-path naming + progressive disclosure |
| Strudel | Cognitive Scientist | Adjacent | 8 | 7 | 6 | 5 | **26** | Conditional converge pending doc policy |
| Tartlet | Git/VCS Expert | Adjacent | 15 | 11 | 12 | 10 | **48** | Concurrency model, migration conventions |
| Galette | Standards Expert | Adjacent | 16 | 13 | 14 | 11 | **54** | RFC 2119 adoption, conformance classes |
| Palmier | Implementation Pragmatist | Adjacent | 14 | 10 | 9 | 9 | **42** | 11-day validation plan, benchmark design |

**Round 1 Total: 374**
**Cumulative Total: 791 (R0: 417 + R1: 374)**

## Tensions Status

| ID | Tension | Status | Resolution |
|----|---------|--------|------------|
| T01 | Path brevity vs self-documentation | **RESOLVED** | Canonical short + symlink aliases |
| T02 | Filesystem vs graph semantics | **RESOLVED** | Binary=files, n-ary=directories |
| T03 | Determinism vs schema evolution | **RESOLVED** | Epoch-scoped determinism |
| T04 | Binary relations insufficient | **RESOLVED** | Directory-per-relation with symlinks |
| T05 | Closed entity types | **RESOLVED** | X-prefix extension mechanism |
| T06 | Relation symmetry ambiguity | **RESOLVED** | Canonical lexicographic ordering |
| T07 | Expert-novice asymmetry | **CONDITIONAL** | Pending doc/error policy commitment |
| T08 | Determinism vs semantic ambiguity | **RESOLVED** | "Deterministic at version" + mandatory qualifiers |
| T09 | State/entity boundary | **RESOLVED** | State-as-property rule |
| T10 | Relation query performance | **ACCEPTED** | O(n) scan accepted as tradeoff |
| T11 | Greppability vs confidentiality | OPEN | Deferred to security RFC |
| T12 | Traversal vs modeling fidelity | **ACCEPTED** | Modeling correctness > query speed |
| T13 | Notational hospitality | **RESOLVED** | Length-prefix overflow syntax |
| T14 | No implementation evidence | **RESOLVED** | 11-day validation plan accepted |

### New Tensions from Round 1

| ID | Tension | Raised By | Status |
|----|---------|-----------|--------|
| T15 | Concurrent write model undefined | Tartlet | OPEN |
| T16 | No .gitattributes for merge strategy | Tartlet | OPEN |
| T17 | Destructive operations unguarded | Tartlet | OPEN |
| T18 | No RFC 2119 normative language | Galette | OPEN |
| T19 | No conformance classes defined | Galette | OPEN |
| T20 | No error model | Galette | OPEN |

## Velocity Calculation

```
Open Tensions: 7 (T07 conditional, T11, T15-T20)
New Perspectives: 18
Velocity = 7 + 18 = 25 (down from 61)
```

## Convergence Signals

| Expert | Signal | Condition |
|--------|--------|-----------|
| Brioche | [MOVE:CONVERGE] | T03, T08, T09 |
| Beignet | [MOVE:CONVERGE] | T05, T13 |
| Muffin | [MOVE:CONVERGE] | T01, T02, delimiter, hash sharding |
| Cannoli | [MOVE:CONVERGE] | T04, T06, T12 |
| Eclair | [MOVE:CONVERGE] | T01, T07 |
| Strudel | [MOVE:CONDITIONAL-CONVERGE] | Needs doc policy |
| Tartlet | Partial | Requires T15-T17 resolution |
| Galette | Conditional | Requires RFC 2119 + conformance classes |
| Palmier | [RE:RESOLVE T14] | Validation plan accepted |

**Converge %: 5/9 unconditional (56%), 2/9 conditional (78% potential)**

## Locked Decisions (Consensus Achieved)

### 1. Naming Convention
- **Canonical**: Single-char (e/, r/, t/, s/, p/, q/)
- **Aliases**: Symlinks to full names (entities/, relations/, etc.)
- **Discovery**: `.aliases/` directory + MANIFEST.md required
- **Authorities**: Muffin, Eclair, Strudel (conditional)

### 2. Relation Storage
- **Binary relations**: Files with `--` delimiter (e.g., `alice--bob.rel`)
- **N-ary relations**: Directories with role-labeled symlinks
- **Symmetric relations**: Canonical hash ordering
- **Attributes**: `meta.yaml` within relation directory
- **Authorities**: Muffin, Cannoli

### 3. Entity Extensibility
- **Core types**: Closed set (person, org, place, concept, project, asset, task, word, event)
- **Extensions**: X-prefix registration (X01, X02, etc.)
- **Overflow notation**: Length-prefix for exhausted namespaces
- **Authorities**: Beignet, Cupcake (R0)

### 4. Determinism Model
- **Scope**: Deterministic within schema epoch
- **Ambiguity**: Mandatory semantic qualifiers
- **State boundary**: State-as-property (not entity)
- **Authorities**: Brioche

### 5. Validation Path
- **Scope**: 27 docs, 50 questions, UKFS vs RAG
- **Timeline**: 11 days
- **Metrics**: Precision@3, determinism, staleness resilience, token efficiency
- **Authority**: Palmier

## Round 2 Requirements for Full Convergence

To reach velocity = 0 and 100% convergence:

1. **T07**: Lock documentation policy (long form in docs, short in code)
2. **T15-T17**: Define concurrency model, .gitattributes, soft-delete
3. **T18-T20**: Adopt RFC 2119, define conformance classes, specify error model

These are **spec quality** issues, not design disagreements. Round 2 should focus on specification rigor rather than architectural decisions.
