# RFC 0028: Dialogue Format Contract

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | [dialogue-generation-linter-mismatch](../spikes/2026-01-26-dialogue-generation-linter-mismatch.md) |
| **Alignment Dialogue** | [dialogue-format-contract-rfc-design](../dialogues/2026-01-26-dialogue-format-contract-rfc-design.dialogue.md) |
| **Alignment Dialogue** | [file-based-subagent-output-and-dialogue-format-contract-rfc-design](../dialogues/2026-01-26-file-based-subagent-output-and-dialogue-format-contract-rfc-design.dialogue.md) |
| **Downstream** | [RFC 0029](0029-file-based-subagent-output.md) depends on this RFC |

---

## Summary

Four independent components parse or produce dialogue markdown using independent format assumptions — regex patterns, ad-hoc `line.contains()` checks, and hardcoded strings. This causes 6+ mismatches between what gets generated and what gets validated. This RFC introduces a shared format contract module in `blue-core` with a `DialogueLine` enum and render/parse pair that eliminates all regex from dialogue handling.

## Problem

The [source spike](../spikes/2026-01-26-dialogue-generation-linter-mismatch.md) identified six format mismatches:

1. **Agent header order** — generator writes `### {Name} {Emoji}`, linter regex expects either order
2. **Perspective ID width** — generator uses `P{:02}` (zero-padded), linter regex accepts `P\d+` (any width)
3. **Judge assessment section** — generator emits `## 💙 Judge:`, linter doesn't recognize it as a valid section
4. **Round numbering** — generator started at Round 1, protocol instructed Round 0
5. **Scoreboard bold totals** — generator wraps totals in `**`, linter regex doesn't require it
6. **No shared format contract** — root cause of all five above

**Root cause**: Three components (generator, linter, Judge protocol) encode format assumptions independently. A fourth component (`alignment.rs::parse_expert_response`) was identified during the alignment dialogue — it uses `line.contains("[PERSPECTIVE")` and `extract_marker()` with its own string-slicing logic.

### Four Consumers

| Consumer | Location | Current Approach |
|----------|----------|-----------------|
| Generator | `blue-mcp/src/handlers/dialogue.rs:806` | Hardcoded `format!()` strings |
| Linter | `blue-mcp/src/handlers/dialogue_lint.rs` | 16+ compiled regex patterns |
| Judge Protocol | `blue-mcp/src/handlers/dialogue.rs:887` | Prose template with format assumptions |
| Alignment Parser | `blue-core/src/alignment.rs:927` | `line.contains()` + `extract_marker()` |

## Design

### Constraint: No Regex

The user constraint is explicit: **no regex in the solution**. All 16+ regex patterns in `dialogue_lint.rs` are replaced by structural parsing using `starts_with`, `split`, `trim`, and `parse`. This is not a limitation — regex was the wrong tool. Markdown lines have structural regularity (headings start with `#`, tables start with `|`, markers start with `[`) that string methods handle cleanly.

### Architecture: `blue-core::dialogue_format` Module

The format contract lives in `blue-core`, not `blue-mcp`. Rationale:

- `alignment.rs::parse_expert_response` (a consumer) already lives in `blue-core`
- The dependency arrow is `blue-mcp → blue-core`, never reversed
- `AlignmentDialogue` struct (the dialogue state model) already lives in `blue-core::alignment`
- Placing format types alongside the state model is natural — schema next to data

### Core Type: `DialogueLine` Enum

Every line in a dialogue document classifies into exactly one of 8 variants:

```rust
/// A classified line from a dialogue markdown document.
pub enum DialogueLine {
    /// `# Title`
    Heading1(String),
    /// `**Key**: Value` metadata fields
    Metadata { key: String, value: String },
    /// `## Section Name` (e.g., "Expert Panel", "Alignment Scoreboard")
    SectionHeading(String),
    /// `## Round N: Label`
    RoundHeading { number: u32, label: String },
    /// `### Agent Name Emoji`
    AgentHeading { name: String, emoji: String },
    /// `| cell | cell | cell |`
    TableRow(Vec<String>),
    /// `[MARKER_TYPE ID: description]`
    MarkerLine { marker_type: MarkerType, id: String, description: String },
    /// Everything else — prose, blank lines, code blocks
    Content(String),
}

pub enum MarkerType {
    Perspective,
    Tension,
    Refinement,
    Concession,
    Resolved,
}
```

Classification uses only `starts_with`, `split`, `trim`, and `parse`:

```rust
impl DialogueLine {
    pub fn classify(line: &str) -> Self {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") && !trimmed.starts_with("## ") {
            Self::Heading1(trimmed[2..].trim().to_string())
        } else if trimmed.starts_with("## Round ") {
            // parse "## Round N: Label"
            // split on ':', parse number from first part
            ...
        } else if trimmed.starts_with("## ") {
            Self::SectionHeading(trimmed[3..].trim().to_string())
        } else if trimmed.starts_with("### ") {
            // parse "### Name Emoji" — name is all words before the emoji
            ...
        } else if trimmed.starts_with("| ") {
            // split by '|', trim cells
            ...
        } else if trimmed.starts_with("[PERSPECTIVE") || trimmed.starts_with("[TENSION")
            || trimmed.starts_with("[REFINEMENT") || trimmed.starts_with("[CONCESSION")
            || trimmed.starts_with("[RESOLVED") {
            // extract marker type, ID, and description
            ...
        } else if trimmed.starts_with("**") && trimmed.contains("**:") {
            // Metadata field
            ...
        } else {
            Self::Content(trimmed.to_string())
        }
    }
}
```

### Interface: `DialogueFormat`

Four methods serve four consumers:

```rust
pub struct DialogueFormat;

impl DialogueFormat {
    /// Generator calls this to produce dialogue markdown.
    pub fn render(dialogue: &AlignmentDialogue) -> String { ... }

    /// Linter calls this to parse and validate a dialogue file.
    /// Returns structured errors instead of boolean checks.
    pub fn parse(markdown: &str) -> Result<ParsedDialogue, Vec<LintError>> { ... }

    /// Alignment parser calls this to extract markers from agent output.
    /// Replaces `parse_expert_response`'s ad-hoc `extract_marker()`.
    pub fn parse_markers(agent_output: &str) -> Vec<Marker> { ... }

    /// Judge protocol embeds this as format instructions for agents.
    /// Generated from the same types — agents read the spec, not code.
    pub fn specification_markdown() -> String { ... }
}
```

The `Marker` type replaces the current stringly-typed marker extraction:

```rust
pub enum Marker {
    Perspective { id: String, description: String },
    Tension { id: String, description: String },
    Refinement(String),
    Concession(String),
    Resolved(String),
}
```

### Tolerance Policy

**Strict where structure matters:**
- `## Round ` — capital R, space required
- `### {agent_name}` — must match a name from the expert panel
- `| {cell} |` — pipe-delimited, column count must match header
- `[PERSPECTIVE P` — capital P, ID required before colon
- Perspective IDs: accept `P1` or `P01`, normalize to `P01` on parse

**Lenient where voice matters:**
- Marker descriptions: any text after the colon
- Content blocks: any markdown
- Whitespace: leading/trailing trimmed, multiple spaces collapsed
- Colon spacing in markers: `P01:desc` and `P01: desc` both parse

### Migration

Phase 1 — **Compat mode** (default for one release cycle):
- New struct-based parser runs alongside existing regex linter
- Warnings emitted when formats diverge
- `fix_hint` strings updated to reference contract types

Phase 2 — **Strict mode**:
- Remove all regex from `dialogue_lint.rs`
- Replace `parse_dialogue()` with `DialogueFormat::parse()`
- Replace `check_markers_parseable()` (currently regex-scans content twice) with single parse call

Phase 3 — **Fourth parser migration**:
- Replace `alignment.rs::extract_marker()` with `DialogueFormat::parse_markers()`
- Replace `parse_expert_response`'s `line.contains()` checks with `DialogueLine::classify()`
- Delete `extract_marker()` function

### ADR Alignment

- **ADR 5 (Single Source)**: One format contract, four consumers. Markdown is the single source of document state. The struct is the schema (constraint definition), not a second copy of data.
- **ADR 10 (No Dead Code)**: Migration plan deletes `extract_marker()`, 16+ regex patterns, and the duplicated `parse_dialogue` logic.
- **ADR 11 (Freedom Through Constraint)**: The typed enum constrains what's valid while giving agents freedom in content and descriptions.

## Phases

### Phase 1: Contract Module

- Create `blue-core/src/dialogue_format.rs`
- Define `DialogueLine` enum with 8 variants
- Implement `DialogueLine::classify()` using string methods only
- Define `MarkerType` and `Marker` enums
- Implement `DialogueFormat::parse_markers()` — replaces `extract_marker()`
- Unit tests: classify every line type, round-trip property tests

### Phase 2: Generator Migration

- Implement `DialogueFormat::render()`
- Replace hardcoded `format!()` strings in `dialogue.rs:806+` with render calls
- Implement `DialogueFormat::specification_markdown()`
- Update `build_judge_protocol` to embed specification
- Integration tests: render then parse round-trips to same structure

### Phase 3: Linter Migration

- Implement `DialogueFormat::parse()` returning `Result<ParsedDialogue, Vec<LintError>>`
- Run in compat mode: both regex and struct parser, compare results
- Replace `parse_dialogue()` in `dialogue_lint.rs` with `DialogueFormat::parse()`
- Remove all `Regex::new()` calls from dialogue lint
- Lint tests: validate all existing dialogue files pass

### Phase 4: Alignment Parser Migration

- Replace `parse_expert_response`'s `line.contains()` checks with `DialogueLine::classify()`
- Replace `extract_marker()` with `DialogueFormat::parse_markers()`
- Delete `extract_marker()` function from `alignment.rs`
- Alignment tests: parse existing expert responses, verify identical output

## Test Plan

- [ ] `DialogueLine::classify()` correctly classifies all 8 line types
- [ ] `DialogueLine::classify()` handles whitespace tolerance (extra spaces, tabs)
- [ ] `DialogueFormat::render()` produces valid markdown that `parse()` accepts
- [ ] `DialogueFormat::parse()` correctly parses all existing dialogue files in `.blue/docs/dialogues/`
- [ ] `DialogueFormat::parse_markers()` produces identical output to current `extract_marker()` for all test cases
- [ ] Zero regex patterns remain in `dialogue_lint.rs` after Phase 3
- [ ] `extract_marker()` deleted after Phase 4
- [ ] Round-trip property: `parse(render(dialogue))` recovers the original structure
- [ ] Compat mode: struct parser and regex parser agree on all existing dialogues

---

*"Right then. Let's get to it."*

— Blue
