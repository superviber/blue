# RFC 0004: ADR Adherence

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-24 |
| **Source Spike** | adr-adherence |
| **ADRs** | 0004 (Evidence), 0007 (Integrity), 0008 (Honor) |

---

## Summary

No mechanism to surface relevant ADRs during work, track ADR citations, or verify adherence to testable architectural decisions.

## Philosophy

**Guide, don't block.** ADRs are beliefs, not bureaucracy. Blue should:
- Help you find relevant ADRs
- Make citing ADRs easy
- Verify testable ADRs optionally
- Never require ADR approval to proceed

## Proposal

### Layer 1: Awareness (Passive)

#### `blue_adr_list`

List all ADRs with summaries:

```
blue_adr_list
```

Returns:
```json
{
  "adrs": [
    { "number": 0, "title": "Never Give Up", "summary": "The only rule we need" },
    { "number": 4, "title": "Evidence", "summary": "Show, don't tell" },
    ...
  ]
}
```

#### `blue_adr_get`

Get full ADR content:

```
blue_adr_get number=4
```

Returns ADR markdown and metadata.

### Layer 2: Contextual Relevance (Active)

#### `blue_adr_relevant`

Given context, use AI to suggest relevant ADRs:

```
blue_adr_relevant context="testing strategy"
```

Returns:
```json
{
  "relevant": [
    {
      "number": 4,
      "title": "Evidence",
      "confidence": 0.95,
      "why": "Testing is the primary form of evidence that code works. This ADR's core principle 'show, don't tell' directly applies to test strategy decisions."
    },
    {
      "number": 7,
      "title": "Integrity",
      "confidence": 0.82,
      "why": "Tests verify structural wholeness - that the system holds together under various conditions."
    }
  ]
}
```

**AI-Powered Relevance:**

Keyword matching fails for philosophical ADRs. "Courage" won't match "deleting legacy code" even though ADR 0009 is highly relevant.

The AI evaluator:
1. Receives the full context (RFC title, problem, code diff, etc.)
2. Reads all ADR content (cached in prompt)
3. Determines semantic relevance with reasoning
4. Returns confidence scores and explanations

**Prompt Structure:**

```
You are evaluating which ADRs are relevant to this work.

Context: {user_context}

ADRs:
{all_adr_summaries}

For each ADR, determine:
1. Is it relevant? (yes/no)
2. Confidence (0.0-1.0)
3. Why is it relevant? (1-2 sentences)

Only return ADRs with confidence > 0.7.
```

**Model Selection:**
- Use fast/cheap model (Haiku) for relevance checks
- Results are suggestions, not authoritative
- User can override or ignore

**Graceful Degradation:**

| Condition | Behavior |
|-----------|----------|
| API key configured, API up | AI relevance (default) |
| API key configured, API down | Fallback to keywords + warning |
| No API key | Keywords only (no warning) |
| `--no-ai` flag | Keywords only (explicit) |

**Response Metadata:**

```json
{
  "method": "ai",        // or "keyword"
  "cached": false,
  "latency_ms": 287,
  "relevant": [...]
}
```

**Privacy:**
- Only context string sent to API (not code, not files)
- No PII should be in context string
- User controls what context to send

#### RFC ADR Suggestions

When creating an RFC, Blue suggests relevant ADRs based on title/problem:

```
blue_rfc_create title="testing-framework" ...

→ "Consider these ADRs: 0004 (Evidence), 0010 (No Dead Code)"
```

#### ADR Citations in Documents

RFCs can cite ADRs in frontmatter:

```markdown
| **ADRs** | 0004, 0007, 0010 |
```

Or inline:

```markdown
Per ADR 0004 (Evidence), we require test coverage > 80%.
```

### Layer 3: Lightweight Verification (Optional)

#### `blue_adr_audit`

Scan for potential ADR violations. Only for testable ADRs:

```
blue_adr_audit
```

Returns:
```json
{
  "findings": [
    {
      "adr": 10,
      "title": "No Dead Code",
      "type": "warning",
      "message": "3 unused exports in src/utils.rs",
      "locations": ["src/utils.rs:45", "src/utils.rs:67", "src/utils.rs:89"]
    },
    {
      "adr": 4,
      "title": "Evidence",
      "type": "info",
      "message": "Test coverage at 72% (threshold: 80%)"
    }
  ],
  "passed": [
    { "adr": 5, "title": "Single Source", "message": "No duplicate definitions found" }
  ]
}
```

**Testable ADRs:**

| ADR | Check |
|-----|-------|
| 0004 Evidence | Test coverage, assertion ratios |
| 0005 Single Source | Duplicate definitions, copy-paste detection |
| 0010 No Dead Code | Unused exports, unreachable branches |

**Non-testable ADRs** (human judgment):

| ADR | Guidance |
|-----|----------|
| 0001 Purpose | Does this serve meaning? |
| 0002 Presence | Are we actually here? |
| 0009 Courage | Are we acting rightly? |
| 0013 Overflow | Building from fullness? |

### Layer 4: Documentation Trail

#### ADR-Document Links

Store citations in `document_links` table:

```sql
INSERT INTO document_links (source_id, target_id, link_type)
VALUES (rfc_id, adr_doc_id, 'cites_adr');
```

#### Search by ADR

```
blue_search query="adr:0004"
```

Returns all documents citing ADR 0004.

#### ADR "Referenced By"

```
blue_adr_get number=4
```

Includes:
```json
{
  "referenced_by": [
    { "type": "rfc", "title": "testing-framework", "date": "2026-01-20" },
    { "type": "decision", "title": "require-integration-tests", "date": "2026-01-15" }
  ]
}
```

## ADR Metadata Enhancement

Add to each ADR:

```markdown
## Applies When

- Writing or modifying tests
- Reviewing pull requests
- Evaluating technical claims

## Anti-Patterns

- Claiming code works without tests
- Trusting documentation over running code
- Accepting "it works on my machine"
```

This gives the AI richer context for relevance matching. Anti-patterns are particularly useful - they help identify when work might be *violating* an ADR.

## Implementation

1. Add ADR document type and loader
2. Implement `blue_adr_list` and `blue_adr_get`
3. **Implement AI relevance evaluator:**
   - Load all ADRs into prompt context
   - Send context + ADRs to LLM (Haiku for speed/cost)
   - Parse structured response with confidence scores
   - Cache ADR summaries to minimize token usage
4. Implement `blue_adr_relevant` using AI evaluator
5. Add ADR citation parsing to RFC creation
6. Implement `blue_adr_audit` for testable ADRs
7. Add "referenced_by" to ADR responses
8. Extend `blue_search` for ADR queries

**AI Integration Notes:**

- Blue MCP server needs LLM access (API key in `.blue/config.yaml`)
- Use streaming for responsiveness
- Fallback to keyword matching if AI unavailable
- Cache relevance results per context hash (5 min TTL)

**Caching Strategy:**

```sql
CREATE TABLE adr_relevance_cache (
    context_hash TEXT PRIMARY KEY,
    adr_versions_hash TEXT,  -- Invalidate if ADRs change
    result_json TEXT,
    created_at TEXT,
    expires_at TEXT
);
```

**Testing AI Relevance:**

- Golden test cases with expected ADRs (fuzzy match)
- Confidence thresholds: 0004 should be > 0.8 for "testing"
- Mock AI responses in unit tests
- Integration tests hit real API (rate limited)

## Test Plan

- [ ] List all ADRs returns correct count and summaries
- [ ] Get specific ADR returns full content
- [ ] AI relevance: "testing" context suggests 0004 (Evidence)
- [ ] AI relevance: "deleting old code" suggests 0009 (Courage), 0010 (No Dead Code)
- [ ] AI relevance: confidence scores are reasonable (0.7-1.0 range)
- [ ] AI relevance: explanations are coherent
- [ ] Fallback: keyword matching works when AI unavailable
- [ ] RFC with `| **ADRs** | 0004 |` creates document link
- [ ] Search `adr:0004` finds citing documents
- [ ] Audit detects unused exports (ADR 0010)
- [ ] Audit reports test coverage (ADR 0004)
- [ ] Non-testable ADRs not included in audit findings
- [ ] Caching: repeated same context uses cached result
- [ ] Cache invalidation: ADR content change clears relevant cache
- [ ] `--no-ai` flag forces keyword matching
- [ ] Response includes method (ai/keyword), cached, latency
- [ ] Graceful degradation when API unavailable

## FAQ

**Q: Will this block my PRs?**
A: No. All ADR features are informational. Nothing blocks.

**Q: Do I have to cite ADRs in every RFC?**
A: No. Citations are optional but encouraged for significant decisions.

**Q: What if I disagree with an ADR?**
A: ADRs can be superseded. Create a new ADR documenting why.

**Q: How do I add a new ADR?**
A: `blue_adr_create` (future work) or manually add to `docs/adrs/`.

**Q: Why use AI for relevance instead of keywords?**
A: Keywords fail for philosophical ADRs. "Courage" won't match "deleting legacy code" but ADR 0009 is highly relevant. AI understands semantic meaning.

**Q: What if I don't have an API key configured?**
A: Falls back to keyword matching. Less accurate but still functional.

**Q: How much does the AI relevance check cost?**
A: Uses Haiku (~$0.00025 per check). Cached for 5 minutes per unique context.

---

*"The beliefs that guide us, made visible."*

— Blue
