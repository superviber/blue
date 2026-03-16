# UKFS Specification Review - Background

## Document Under Review

**Universal Knowledge Filesystem (UKFS) v0.1.0** - A hierarchical directory-based architecture for organizing arbitrary knowledge in a format optimized for autonomous agent navigation.

## Core Innovation

Unlike RAG (probabilistic embedding similarity) or Tool-Based Retrieval (query languages), UKFS enables **deterministic semantic path traversal** - agents reason about knowledge location rather than search for it.

## Current Specification

### Root Directory Structure
```
/{root}/
├── .spec/    # UKFS specification metadata
├── e/        # ENTITIES (nouns)
├── r/        # RELATIONS (edges)
├── t/        # TIMELINE (events)
├── s/        # STATE (current snapshots)
├── p/        # PROCEDURES (how-to)
└── q/        # QUERIES (cached Q&A)
```

### Entity Types (/e/)
- person, org, place, concept, project, asset, task, word

### Relation Convention
`{subject}+{object}.yaml` in `/r/{relation-type}/`

### Design Principles
1. Path as identifier (human-readable, greppable)
2. No UUIDs in paths (speakable, memorable)
3. Type as namespace (disambiguation through hierarchy)
4. English structure, universal content
5. Relations as first-class files
6. State separation (current vs timeless)

### Agent Navigation Algorithm (8 steps)
1. Parse query for entity references and intent
2. Identify relevant entity types
3. Resolve entity paths: /e/{type}/{slug}/
4. Find relations: /r/{relation-type}/*{entity}*.yaml
5. Check state if "current": /s/
6. Check timeline if temporal: /t/
7. Check procedures if "how to": /p/
8. Synthesize response

## Key Questions for Deliberation

1. **Naming Convention**: Single-character (e/, r/, t/, s/, p/, q/) vs full names (entities/, relations/...)
   - Trade-offs: brevity vs discoverability vs agent navigation efficiency

2. **Entity Type Coverage**: Are person, org, place, concept, project, asset, task, word sufficient?

3. **Relation Modeling**: Is {subject}+{object}.yaml adequate? What about:
   - N-ary relations (3+ participants)
   - Relation attributes
   - Bidirectional relations

4. **State vs Timeline**: Is /s/ vs /t/ separation clear? Edge cases?

5. **Query Caching (/q/)**: Staleness concerns? Invalidation strategy?

6. **Scalability**: Millions of entities? Sharding strategies?

7. **Agent Navigation**: Is the 8-step algorithm complete? Disambiguation? Fallbacks?

8. **Git Integration**: Merge conflicts? Concurrent edits?

9. **Security/Privacy**: Access control? Encryption?

10. **Missing Concepts**: What fundamental categories are absent?

## Success Criteria

The dialogue should produce concrete, actionable improvements to the UKFS specification that maintain its core philosophy (deterministic semantic navigation) while addressing practical concerns.
