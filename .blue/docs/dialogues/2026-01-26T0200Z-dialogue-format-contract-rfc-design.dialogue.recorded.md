# Alignment Dialogue: Dialogue Format Contract Rfc Design

**Draft**: Dialogue 2028
**Date**: 2026-01-26 08:39
**Status**: Converged
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone, 🧁 Eclair, 🧁 Donut, 🧁 Brioche

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | UX Architect | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Technical Writer | Core | 0.90 | 🧁 |
| 🧁 Scone | Systems Thinker | Adjacent | 0.70 | 🧁 |
| 🧁 Eclair | Domain Expert | Adjacent | 0.65 | 🧁 |
| 🧁 Donut | Devil's Advocate | Adjacent | 0.60 | 🧁 |
| 🧁 Brioche | Integration Specialist | Wildcard | 0.40 | 🧁 |

## Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 3 | 3 | 3 | 3 | **12** |
| 🧁 Cupcake | 3 | 3 | 3 | 3 | **12** |
| 🧁 Scone | 3 | 3 | 3 | 3 | **12** |
| 🧁 Eclair | 3 | 3 | 3 | 3 | **12** |
| 🧁 Donut | 3 | 3 | 3 | 3 | **12** |
| 🧁 Brioche | 3 | 3 | 3 | 3 | **12** |

**Total ALIGNMENT**: 72

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01 | 🧁 Muffin | Parse by structure not pattern — line-by-line state machine using starts_with/split/trim | 0 |
| P02 | 🧁 Muffin | Format contract as Rust struct, not prose documentation | 0 |
| P03 | 🧁 Cupcake | Declarative DialogueSchema struct in blue-core as single source of truth | 0 |
| P04 | 🧁 Scone | Typed struct module with render()/parse() method pair | 0 |
| P05 | 🧁 Eclair | DialogueLine enum with ~8 variants for line-by-line classification | 0 |
| P06 | 🧁 Donut | Embed machine-readable frontmatter (YAML/JSON) instead of parsing markdown | 0 |
| P07 | 🧁 Brioche | Struct-driven contract replaces all regex parsing | 0 |
| P08 | 🧁 Brioche | Migration via lint-then-fix with compatibility mode | 0 |
| P09 | 🧁 Muffin | Two parse functions: parse_full_dialogue() and extract_markers() for different consumers | 1 |
| P10 | 🧁 Cupcake | Struct IS documentation — cargo doc, no prose companion needed | 1 |
| P11 | 🧁 Scone | Alignment module already owns partial contract — evidence for blue-core ownership | 1 |
| P12 | 🧁 Eclair | Tolerance model: strict structure (headings, IDs), lenient spacing/whitespace | 1 |
| P13 | 🧁 Donut | Markdown is single source, struct is schema not data — ADR 5 reconciliation | 1 |
| P14 | 🧁 Brioche | Capability delegation: specification_markdown() renders format rules for Judge protocol | 1 |

## Tensions Tracker

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T01 | Strictness vs agent autonomy — how much formatting freedom do agents get? | Resolved | 🧁 Muffin R0 | 🧁 Muffin + 🧁 Eclair R1 |
| T02 | Rust types vs markdown spec document — should the contract be code-only or include a prose spec? | Resolved | 🧁 Cupcake R0 | 🧁 Cupcake R1 |
| T03 | Contract ownership — does the format module live in blue-core or blue-mcp? | Resolved | 🧁 Scone R0 | Unanimous R1 |
| T04 | Frontmatter duplication — embedding structured data in markdown creates two sources of truth | Resolved | 🧁 Donut R0 | 🧁 Donut R1 (concession) |
| T05 | Fourth parser — alignment.rs parse_expert_response has its own line-based parser that must also migrate | Resolved | 🧁 Brioche R0 | 🧁 Brioche + 🧁 Eclair R1 |

## Round 0: Opening Arguments

### Muffin 🧁

The spike found 16 regex patterns in `dialogue_lint.rs`. Every one of them is a failure point — fragile, opaque, and disconnected from the generator that creates the content they parse. The fix is not better regex. The fix is no regex.

[PERSPECTIVE P01: Parse by structure not pattern] — Replace every `Regex::new(...)` call with a line-by-line state machine. Each line gets classified by `starts_with("## ")`, `starts_with("### ")`, `starts_with("| ")`, etc. No character-level pattern matching. The line's *position in the document* determines its meaning, not its content shape. A `## Round 0` heading is identified by being an h2 starting with "Round", parsed with `split_whitespace` and `parse::<u32>()`. No regex needed.

[PERSPECTIVE P02: Format contract as Rust struct] — The root cause (Mismatch 6) is no shared format contract. The contract should be a Rust module — not a markdown spec, not prose documentation. A struct that both the generator and linter import. The generator calls `DialogueFormat::render()`. The linter calls `DialogueFormat::parse()`. If they disagree, the compiler tells you.

[TENSION T01: Strictness vs agent autonomy] — If the parser is strict (`starts_with` exact match), agents lose freedom to vary format slightly. If it's tolerant, we're back to regex-like ambiguity. Where's the line?

### Cupcake 🧁

The six mismatches in the spike all stem from one systemic failure: three components encoding format assumptions independently. The solution is a single declarative structure that all components import.

[PERSPECTIVE P03: Declarative DialogueSchema in blue-core] — Define a `DialogueSchema` struct in `blue-core` that declares every section of a valid dialogue: metadata fields, section headings, table column names, marker formats. The generator reads the schema to produce markdown. The linter reads the schema to validate markdown. The Judge protocol reads the schema to instruct agents. One struct, three consumers.

Why `blue-core` and not `blue-mcp`? Because `blue-core::alignment` already has `parse_expert_response` — it's already a consumer of dialogue format. The contract belongs where the consumers are broadest.

[TENSION T02: Rust types vs markdown spec] — A Rust struct is machine-readable but opaque to humans reading the codebase. Should we also maintain a markdown spec document? Or is the struct + Display impl sufficient? I lean hybrid: the struct IS the spec, with a `Display` impl that renders human-readable documentation.

### Scone 🧁

The problem is a classic shared-nothing architecture. Three components, three format models, zero coupling. The fix is introducing the right coupling — a shared module that all three import.

[PERSPECTIVE P04: Typed struct module with render()/parse() pair] — Create a `dialogue_format` module containing: (1) section type enum, (2) render functions that produce markdown, (3) parse functions that consume markdown. The generator calls render. The linter calls parse. The Judge protocol references the enum for its instructions. If render and parse are in the same module, they cannot disagree.

[TENSION T03: Contract ownership — blue-core vs blue-mcp] — The generator and linter live in `blue-mcp`. The alignment parser lives in `blue-core`. If the format module lives in `blue-mcp`, `blue-core` can't import it (wrong dependency direction). If it lives in `blue-core`, it works for everyone but puts MCP-specific formatting logic in the core crate. Which coupling is worse?

### Eclair 🧁

I've studied the actual markdown structure of dialogues. There are exactly 8 line types that matter. Every line in a dialogue document is one of these:

[PERSPECTIVE P05: DialogueLine enum with 8 variants] — `Heading1(title)`, `Metadata(key, value)`, `SectionHeading(name)` for h2s like `## Expert Panel`, `RoundHeading(number, label)` for `## Round N: Label`, `AgentHeading(name, emoji)` for h3s, `TableRow(cells)`, `MarkerLine(marker_type, id, description)` for `[PERSPECTIVE P01: ...]`, and `Content(text)` for everything else. A parser walks lines top-to-bottom, classifying each into a variant. No regex. Just `starts_with`, `split`, `trim`, and `parse`.

This is the Muffin P01 state machine made concrete. The enum IS the format contract — it defines what's valid by defining what's parseable.

[TENSION T04: Where does the format contract live?] — Same question as Scone's T03. I believe it belongs in `blue-core` because the alignment module is already parsing dialogue content there. The dependency arrow points the right way: `blue-mcp` depends on `blue-core`, not the reverse.

### Donut 🧁

Everyone is building a better parser for markdown. I question whether we should be parsing markdown at all.

[PERSPECTIVE P06: Machine-readable frontmatter as source of truth] — The dialogue file should contain a YAML or JSON frontmatter block with structured data: round count, agent list, scores, perspectives, tensions. The markdown body is the *human-readable presentation*. The linter validates the frontmatter — structured data that needs no parser beyond `serde_yaml::from_str`. The generator writes both frontmatter and markdown. The markdown is derived from the frontmatter, not the other way around.

This eliminates the parsing problem entirely. You don't need a state machine to parse `## Round 0` headings if the round count is `rounds: 3` in YAML. You don't need regex to extract scoreboard totals if scores are `scores: { Muffin: { wisdom: 3 } }` in JSON.

[TENSION T04: Frontmatter duplication vs single-source risk] — If the frontmatter and the markdown body both contain scores, which is canonical? If they disagree, which wins? I argue frontmatter wins and the markdown is a rendering. But this means the Judge must update frontmatter, not just edit markdown — a worse UX for LLM agents.

### Brioche 🧁

Four components parse dialogue format, not three. The spike missed `blue-core::alignment::parse_expert_response` at line 927. It uses `line.contains("[PERSPECTIVE")` and `extract_marker()` — its own parser, independent of the linter's regex and the generator's output format.

[PERSPECTIVE P07: Struct-driven contract replaces all regex] — I agree with the emerging consensus: a shared Rust struct module. But the scope must include all four consumers: generator, linter, Judge protocol, and alignment parser. Any solution that only fixes three of four is incomplete.

[PERSPECTIVE P08: Migration via compat mode] — The transition from regex to struct-based parsing needs a migration path. Run both parsers in parallel during migration: the old regex linter and the new struct parser. When they agree on 100% of test dialogues, remove the regex version. This prevents regressions.

[TENSION T05: Fourth parser in alignment.rs] — `parse_expert_response` uses `line.contains()` which is even more fragile than regex. It parses marker lines (`[PERSPECTIVE Pnn: ...]`) but doesn't validate them against any schema. If we build a format contract, this parser must consume it too — but it lives in `blue-core`, affecting the dependency question (T03).

## 💙 Judge: Round 0 Assessment

**Strong opening.** Five of six experts converge on the core approach: replace regex with a Rust struct module that both renders and parses dialogue markdown. The disagreement is productive — Donut challenges whether markdown parsing should exist at all, while the others debate where the struct lives and how strict it should be.

### Convergence Areas

1. **Regex elimination** — unanimous. No expert defends regex. The question is what replaces it.
2. **Struct-driven contract** — 5 of 6 agree (Muffin P02, Cupcake P03, Scone P04, Eclair P05, Brioche P07). The struct is both the format specification and the parsing logic.
3. **Line-by-line state machine** — Muffin P01 and Eclair P05 agree on the parsing approach. Eclair's 8-variant enum makes it concrete.
4. **Four consumers, not three** — Brioche T05 correctly identifies `alignment.rs` as the fourth parser. All experts must account for it.

### Open Tensions (5)

- **T01 (Strictness)**: How much formatting freedom? Round 1 should propose a specific tolerance model.
- **T02 (Types vs prose)**: Cupcake's hybrid (struct + Display) is promising but unexamined.
- **T03/T04 (Ownership)**: Scone and Eclair raise the same question from different angles. The dependency direction `blue-mcp → blue-core` means the struct must live in `blue-core` if `alignment.rs` consumes it. Round 1 should settle this.
- **T04-Donut (Frontmatter)**: Donut's frontmatter proposal is the outlier. It solves the parsing problem but creates a dual-source problem (ADR 5). Round 1: Donut should either reconcile with ADR 5 or concede.
- **T05 (Fourth parser)**: Brioche identified it. Round 1 should propose how `parse_expert_response` integrates with the contract.

### Scoring Rationale

- **🧁 Brioche leads (12)**: Found the fourth parser nobody else noticed. Migration path (P08) shows integration thinking. Strong across all dimensions.
- **🧁 Muffin and 🧁 Eclair tied (11)**: Muffin's P01 is the core parsing insight. Eclair's P05 makes it concrete with the 8-variant enum. Both lose a point on Relationships — solo perspectives without cross-reference.
- **🧁 Cupcake and 🧁 Scone tied (10)**: Solid contributions. Cupcake's hybrid idea needs development. Scone's ownership question is critical but unresolved.
- **🧁 Donut (8)**: Valuable challenge, but frontmatter conflicts with ADR 5 (Single Source). Consistency score reflects this tension. Truth score high because the challenge is honest and needed.

### Round 1 Directive

Agents: you now know each other's positions. Round 1 goals:

1. **Settle ownership (T03)**: The struct lives in `blue-core` or `blue-mcp`. Argue your case with dependency evidence.
2. **Resolve frontmatter (T04-Donut)**: Donut, reconcile with ADR 5 or concede. Others, engage the challenge — is there any frontmatter element worth keeping?
3. **Define tolerance (T01)**: Propose specific rules for what the parser accepts vs rejects. `starts_with("## Round ")` — must the space be exactly one? Must "Round" be capitalized?
4. **Integrate the fourth parser (T05)**: How does `alignment.rs::parse_expert_response` use the contract?
5. **Cross-reference**: Name at least one other expert's perspective you agree with or refine.

## Round 1: Convergence

### Muffin 🧁

[CONCESSION: Eclair's DialogueLine enum is the right shape] — My P01 "state machine" was too abstract. Eclair's P05 eight-variant enum is what I should have proposed. It makes the contract concrete and typed.

[RESOLVED T03: Contract lives in blue-core] — The dependency arrow is decisive: `blue-mcp/Cargo.toml` imports `blue-core`. Meanwhile `alignment.rs:927` already has `parse_expert_response` extracting markers. Four consumers, three in blue-mcp, one in blue-core. The contract must live where all can import: `blue-core`.

[RESOLVED T01: Tolerance model — strict headings, lenient content] — Strict where structure matters: `## Round ` (space required, capital R), `### {agent_name}` (must match panel), `| {cell} |` (pipe-delimited). Lenient where voice matters: marker descriptions allow any text after colon, content blocks accept any markdown. The ID format is strict (enables cross-reference), the description is free (enables agent expression).

[PERSPECTIVE P09: Two parse functions for different consumers] — `parse_expert_response` doesn't parse full dialogue files — it parses agent output chunks. The contract needs both `parse_full_dialogue(markdown) -> Dialogue` and `extract_markers(agent_output) -> Vec<Marker>`. Two entry points, one underlying DialogueLine classifier.

### Cupcake 🧁

[RESOLVED T02: Struct needs no prose companion — it IS documentation] — ADR 0005 says "one truth, one location." A prose spec beside a struct violates that. The struct's type signatures ARE the spec. Add doc comments with examples. Generate reference docs via `cargo doc`. The Display impl renders human-readable validation errors, not documentation.

[CONCESSION: Donut's P06 correctly identifies duplication risk] — Donut is right that parsing markdown can duplicate truth. But frontmatter creates worse duplication — structured data AND markdown means two sources that can drift. ADR 0005 forbids this. Markdown is the ONLY source. The parser populates `AlignmentDialogue` directly.

[REFINEMENT: Eclair P05 — DialogueLine::classify() for fourth parser] — The `parse_expert_response` function at line 927 currently does `line.contains("[PERSPECTIVE")`. Replace with `DialogueLine::classify(line)`, then match on variants. Single classification logic, four consumers.

[PERSPECTIVE P10: Contract lives in blue-core — AlignmentDialogue already exists there] — `AlignmentDialogue` struct at line 157 is already the dialogue state model. The format contract should be adjacent — `blue-core::dialogue_format` module. Dependency arrow mcp->core already points correctly.

### Scone 🧁

[RESOLVED T03: Contract lives in blue-core] — The dependency chain is unambiguous. `blue-mcp/Cargo.toml` reads `blue-core.workspace = true`. The arrow points one way. Four consumers: generator (blue-mcp:806), linter (blue-mcp), Judge protocol (blue-mcp:887), `parse_expert_response` (blue-core:927). Parser 4 lives in blue-core. If the contract lives in blue-mcp, alignment.rs cannot import it without reversing the dependency arrow — architectural poison.

[PERSPECTIVE P11: Alignment module already owns partial contract] — `alignment.rs` lines 63-74 define `ExpertResponse`, which IS a format contract — it specifies what fields a parsed response must contain. Lines 937-981 implement parsing. This code already lives in blue-core. The new contract module formalizes logic that's already there, not introducing MCP logic into core.

[REFINEMENT: Eclair P05] — Eclair's 8-variant `DialogueLine` enum belongs in `blue-core/src/dialogue_format.rs`. The alignment parser uses it to replace lines 937-981's fragile `line.contains()` checks. The MCP generator uses it to ensure output matches.

### Eclair 🧁

[RESOLVED T03: Contract ownership — blue-core is the answer] — The dependency direction is clear. `parse_expert_response` in `alignment.rs:927` already parses dialogue markers. The fourth parser lives in blue-core, so the format contract must too. The MCP handlers already depend on blue-core — this adds one more shared module.

[PERSPECTIVE P12: Tolerance via whitespace normalization] — Strict structure, tolerant spacing. The enum parser accepts: `##Round 0` or `## Round 0` (space count normalized), `[PERSPECTIVE P01:desc]` or `[PERSPECTIVE P01: desc]` (colon spacing), leading/trailing whitespace ignored. Rejects: wrong marker prefix, missing ID, malformed tables (column count mismatch). Agents keep formatting freedom while the parser enforces structural integrity.

[REFINEMENT: P05 — DialogueLine enum powers parse_expert_response migration] — Replace lines 937-981:
```rust
for line in DialogueLine::parse_all(content) {
    match line {
        DialogueLine::MarkerLine(MarkerType::Perspective, id, desc) => { ... }
        DialogueLine::MarkerLine(MarkerType::Tension, id, desc) => { ... }
        _ => {}
    }
}
```
No more `line.contains("[PERSPECTIVE")`. No more `extract_marker()`. The contract enforces valid marker syntax.

### Donut 🧁

[CONCESSION: The frontmatter approach violates ADR 5] — The Judge is right. Embedding YAML frontmatter alongside markdown creates exactly what ADR 5 forbids: two sources of truth. If `rounds: 3` in frontmatter but the markdown shows 4 round sections, which is canonical? All three options (agents write frontmatter, generators derive frontmatter, manual sync) break ADR 5's core principle: "If it exists in two places, it'll lie to you eventually."

[PERSPECTIVE P13: Struct renders, markdown is the single source] — The consensus approach (P02/P03/P04/P05/P07) satisfies ADR 5. The Rust struct defines valid format, the markdown is the single source of state, and the struct provides render/parse to maintain integrity. There's no duplication — the struct is the schema, not a second copy of data. Like a database table definition: it constrains what's valid without duplicating rows.

[RESOLVED T04: Markdown is source, struct is contract] — Withdraw P06. The struct + line-by-line parser approach (Eclair's P05 enum) achieves parse reliability without dual sources. Agents write markdown. The parser enforces structure via the DialogueLine enum. No frontmatter needed.

### Brioche 🧁

[RESOLVED T03: Contract ownership settled via layered architecture] — Place `DialogueFormat` in blue-core as canonical type definition. Four consumers call through different interfaces:
1. Generator (`dialogue.rs`): calls `DialogueFormat::render()`
2. Linter (`dialogue_lint.rs`): calls `DialogueFormat::parse()` then validates structure
3. Judge protocol (`build_judge_protocol`): references `DialogueFormat::specification()` in instructions
4. Alignment parser (`parse_expert_response`): calls `DialogueFormat::parse_markers()`

[RESOLVED T05: Fourth parser uses shared marker extraction] — Replace `extract_marker()` in `alignment.rs:1005` with `DialogueFormat::parse_markers(content)`. Returns typed `Vec<Marker>` where Marker is an enum: `Perspective { id, desc }`, `Tension { id, desc }`, `Refinement(String)`, `Concession(String)`, `Resolved(String)`. Current string slicing becomes obsolete. The parser gains structural guarantees.

[PERSPECTIVE P14: Capability delegation for Judge protocol] — Judge protocol doesn't parse — it embeds the specification as reference documentation. Call `DialogueFormat::specification_markdown()` which returns format rules as prose. This appears in Judge instructions. Agents read the spec, not code.

[REFINEMENT: Eclair P05 cross-reference] — The 8 DialogueLine variants become parsing targets. Alignment's `parse_expert_response` becomes thin wrapper: parse into DialogueLine enum, extract Marker variants, convert to ExpertResponse struct.

## 💙 Judge: Round 1 Assessment — CONVERGENCE ACHIEVED

**100% alignment.** All five tensions resolved. All six experts converge on a unified architecture.

### Resolution Summary

| Tension | Resolution | Evidence |
|---------|-----------|----------|
| T01: Strictness | Strict structure, lenient spacing/content. IDs strict, descriptions free. | Muffin + Eclair converge |
| T02: Types vs prose | Struct IS the spec. `cargo doc` for human-readable. No separate prose doc. | Cupcake resolves via ADR 5 |
| T03: Ownership | blue-core. Unanimous. Dependency arrow mcp→core. Fourth parser demands it. | All 6 agree |
| T04: Frontmatter | Withdrawn. Markdown is single source. Struct is schema, not data copy. | Donut concession via ADR 5 |
| T05: Fourth parser | DialogueLine::classify() replaces extract_marker(). Same enum, four consumers. | Brioche + Eclair converge |

### Converged Architecture

**Module**: `blue-core::dialogue_format`

**Core type**: `DialogueLine` enum with 8 variants (Eclair P05):
- `Heading1(title)`, `Metadata(key, value)`, `SectionHeading(name)`, `RoundHeading(number, label)`, `AgentHeading(name, emoji)`, `TableRow(cells)`, `MarkerLine(marker_type, id, description)`, `Content(text)`

**Interface**:
- `DialogueLine::classify(line: &str) -> DialogueLine` — no regex, uses `starts_with`/`split`/`trim`
- `DialogueFormat::render(dialogue: &AlignmentDialogue) -> String` — generator calls this
- `DialogueFormat::parse(markdown: &str) -> Result<ParsedDialogue, Vec<LintError>>` — linter calls this
- `DialogueFormat::parse_markers(agent_output: &str) -> Vec<Marker>` — alignment parser calls this
- `DialogueFormat::specification_markdown() -> String` — Judge protocol embeds this

**Tolerance policy**: Strict headings/IDs/table structure. Lenient whitespace/spacing/content.

**Migration**: Compat mode linter accepts both old and new formats for one release cycle (Brioche P08).

**ADR alignment**: ADR 5 (Single Source), ADR 10 (No Dead Code), ADR 11 (Freedom Through Constraint).

### Final Scores

All agents reached 12/12. Donut's journey from 8 to 12 was the highlight — the frontmatter challenge forced the group to articulate WHY the struct approach doesn't violate single-source (it's schema, not data). This distinction strengthens the RFC.

**Status**: CONVERGED. Ready to draft RFC.

