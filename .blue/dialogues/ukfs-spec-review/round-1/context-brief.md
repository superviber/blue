# Round 1 Context Brief

## Round 0 Summary

12 experts analyzed the UKFS specification. Key findings:

### Emerging Consensus

1. **Naming Convention**: Symlink aliases providing both forms (e/ AND entities/) is gaining support
2. **Relation Modeling**: Reification for n-ary + canonical ordering for symmetric + `--` delimiter instead of `+`
3. **Scalability**: Hash-prefix sharding (e/person/a8/alice/) + secondary index files
4. **Event Entity Type**: Add /e/event/ as first-class type
5. **Schema Layer**: Add /.schema/ for type definitions

### Open Tensions Requiring Resolution

| ID | Tension | Key Question |
|----|---------|--------------|
| T01 | Single-char vs full names | Lock the naming decision |
| T04 | Binary relations insufficient | Confirm reification approach |
| T05 | Closed entity types | Define extensibility mechanism |
| T08 | Determinism vs semantic ambiguity | Clarify what "deterministic" means |
| T14 | No implementation evidence | Define proof-of-concept scope |

### Key Perspectives from Round 0

- **Muffin**: "Single-char roots are correct. Hash-prefix sharding for scale. Relations should be directories, not files."
- **Cupcake**: "Add Event entity type. Use reification for n-ary relations. Define extensibility via /_custom/."
- **Eclair**: "Provide symlink aliases for both forms. Require MANIFEST.md in each directory."
- **Brioche**: "Add relation index manifests per entity. Define state semantics explicitly."
- **Cannoli**: "Canonical lexicographic ordering for symmetric relations. Accept O(n) scan cost for attributes."
- **Beignet**: "Adopt faceted auxiliary layer (/f/). Implement authority control for synonyms."
- **Churro**: "Prove it first. Define anti-goals. Determinism claim is overstated."

### Round 1 Objectives

1. **Lock T01**: Decide naming convention (single-char canonical + symlink aliases?)
2. **Lock T04**: Confirm relation modeling (reification + `--` delimiter + canonical ordering?)
3. **Lock T05**: Define extensibility (/_custom/? /f/ facets? Both?)
4. **Address T08**: Reframe "deterministic" claim accurately
5. **Address T14**: Define minimum viable proof-of-concept

Experts should build on Round 0 insights and drive toward convergence on these decisions.
