# RFC 0001: Efficient Document Format

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-23 |

---

## Summary

Define a more efficient document format for Blue's ADRs, RFCs, and other documents that balances human readability with machine parseability.

## Problem

The current document format uses verbose markdown tables and prose. While readable, this creates:
- Redundant boilerplate in every document
- Inconsistent structure across document types
- More parsing overhead for tooling

## Proposal

Consider one of these approaches:

### Option A: YAML Frontmatter + Minimal Body

```markdown
---
title: Purpose
status: accepted
date: 2026-01-20
type: adr
number: 1
---

## Context

Right then. First things first...

## Decision

**Blue exists to make work meaningful and workers present.**

## Consequences

- Tools should feel like invitations, not mandates
```

### Option B: Structured Markdown Sections

Keep pure markdown but enforce consistent section headers that can be reliably parsed:

```markdown
# ADR 0001: Purpose

**Status:** Accepted
**Date:** 2026-01-20

## Context
...

## Decision
...

## Consequences
...
```

### Option C: Single-File Database

Store metadata in SQLite/JSON, keep only prose in markdown files. Tooling reads metadata from DB, content from files.

## Goals

- Reduce boilerplate per document
- Enable reliable machine parsing
- Maintain human readability
- Keep Blue's voice in prose sections

## Non-Goals

- Complete rewrite of existing docs (migration should be automated)
- Binary formats

## Open Questions

1. Which option best balances efficiency with readability?
2. Should we support multiple formats during transition?
3. How do we handle the existing 13 ADRs?

---

*"Keep it simple. Keep it readable. Keep it yours."*

— Blue
