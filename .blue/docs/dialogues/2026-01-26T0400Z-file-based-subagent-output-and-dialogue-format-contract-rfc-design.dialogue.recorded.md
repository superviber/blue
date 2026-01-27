# Alignment Dialogue: File Based Subagent Output And Dialogue Format Contract Rfc Design

**Draft**: Dialogue 2029
**Date**: 2026-01-26 09:05
**Status**: Converged
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone, 🧁 Eclair, 🧁 Donut, 🧁 Brioche, 🧁 Croissant, 🧁 Macaron, 🧁 Cannoli, 🧁 Strudel, 🧁 Beignet, 🧁 Churro

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | UX Architect | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Technical Writer | Core | 0.90 | 🧁 |
| 🧁 Scone | Systems Thinker | Core | 0.85 | 🧁 |
| 🧁 Eclair | Domain Expert | Core | 0.80 | 🧁 |
| 🧁 Donut | Devil's Advocate | Adjacent | 0.70 | 🧁 |
| 🧁 Brioche | Integration Specialist | Adjacent | 0.65 | 🧁 |
| 🧁 Croissant | Risk Analyst | Adjacent | 0.60 | 🧁 |
| 🧁 Macaron | First Principles Reasoner | Adjacent | 0.55 | 🧁 |
| 🧁 Cannoli | Pattern Recognizer | Adjacent | 0.50 | 🧁 |
| 🧁 Strudel | Edge Case Hunter | Wildcard | 0.40 | 🧁 |
| 🧁 Beignet | Systems Thinker | Wildcard | 0.35 | 🧁 |
| 🧁 Churro | Domain Expert | Wildcard | 0.30 | 🧁 |

## Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 6 | 7 | 7 | 4 | **24** |
| 🧁 Cupcake | 6 | 6 | 7 | 4 | **23** |
| 🧁 Scone | 7 | 8 | 7 | 4 | **26** |
| 🧁 Eclair | 6 | 6 | 7 | 4 | **23** |
| 🧁 Donut | 7 | 6 | 7 | 4 | **24** |
| 🧁 Brioche | 6 | 7 | 7 | 4 | **24** |
| 🧁 Croissant | 7 | 6 | 7 | 4 | **24** |
| 🧁 Macaron | 7 | 8 | 8 | 4 | **27** |
| 🧁 Cannoli | 7 | 6 | 7 | 4 | **24** |
| 🧁 Strudel | 7 | 5 | 6 | 4 | **22** |
| 🧁 Beignet | 6 | 7 | 7 | 4 | **24** |
| 🧁 Churro | 6 | 7 | 6 | 3 | **22** |

**Total ALIGNMENT**: 287

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01 | Muffin | Contract governs transport, not just schema | 0 |
| P01 | Cupcake | File-based arch IS format contract's distribution mechanism | 0 |
| P01 | Scone | Interface Boundary Confusion — transport vs schema orthogonal | 0 |
| P01 | Eclair | Separation of concerns — transport vs schema | 0 |
| P01 | Donut | Separable concerns masquerading as unity | 0 |
| P01 | Brioche | Integration surface — where file output meets format contract | 0 |
| P01 | Croissant | State Synchronization Gap — race condition risk | 0 |
| P01 | Macaron | Orthogonal layers, not parallel concerns | 0 |
| P01 | Cannoli | The Contract Is The Boundary | 0 |
| P02 | Cannoli | The Round Path Insight — staging area | 0 |
| P01 | Strudel | Atomic writes vs partial reads | 0 |
| P01 | Beignet | Temporal Boundaries Define Component Responsibilities | 0 |
| P02 | Beignet | File Paths Are Part of Protocol Contract | 0 |
| P01 | Churro | MCP surface area vs orchestration boundaries | 0 |
| P02 | Muffin | Fragment parsing IS the dependency edge | 1 |
| P02 | Cupcake | Two RFCs with explicit dependency — RFC 0028 ships first | 1 |
| P02 | Scone | Integration surface exists at read boundaries, not write boundaries | 1 |
| P02 | Eclair | Dependency is protocol-level, not implementation-level | 1 |
| P02 | Donut | MCP containment preserved via staging area + task barriers | 1 |
| P02 | Brioche | Zero shared implementation surface — three parse targets | 1 |
| P02 | Croissant | DialogueFormat as assembly-time validator in staging area | 1 |
| P02 | Macaron | Spike needs RFC 0028 for validation — dependency is real | 1 |
| P02 | Cannoli | Fragment vs document distinction proves separation | 1 |
| P02 | Strudel | Round-scoped paths + task barriers resolve race condition | 1 |
| P02 | Beignet | Path = protocol contract, not format contract | 1 |
| P02 | Churro | Two translation layers — structural and protocol | 1 |

## Tensions Tracker

| ID | Agent | Tension | Status | Round |
|----|-------|---------|--------|-------|
| T01 | Muffin | Agent instruction delivery gap | Resolved | 0 |
| T01 | Cupcake | Validation timing — at write or at read? | Resolved | 0 |
| T01 | Scone | Premature Unification | Resolved | 0 |
| T01 | Eclair | Which concern deserves RFC status? | Resolved | 0 |
| T01 | Donut | File-based output breaks MCP containment | Resolved | 0 |
| T01 | Brioche | Two migration paths, same target | Resolved | 0 |
| T01 | Croissant | Two Parsers or Parameterized Tolerance | Resolved | 0 |
| T01 | Macaron | Spike's recommendation is incomplete | Resolved | 0 |
| T01 | Cannoli | Write Tool Surface Area vs Contract Enforcement | Resolved | 0 |
| T01 | Strudel | Two RFCs or one unified? | Resolved | 0 |
| T01 | Beignet | Where Does DialogueLine Classification Happen? | Resolved | 0 |
| T01 | Churro | Single RFC risks conflating structural validation with file orchestration | Resolved | 0 |
| T02 | Churro | Stream vs document modes — when agent output exceeds buffer | Open | 1 |
| T02 | Muffin | MCP containment beneficial or restrictive? | Open | 1 |

## Round 0: Opening Arguments

### Muffin 🧁

[PERSPECTIVE P01: Contract governs transport, not just schema]

The spike proposes agents Write to /tmp files; RFC 0028 defines DialogueLine parsing. These aren't separate concerns — they're layers of the same contract. The unified RFC should define **both what a dialogue line IS (structure) and how it MOVES (transport)** between judge and subagents.

Current RFC 0028 treats DialogueFormat as pure schema — `parse()` and `render()` for validation. But the spike reveals agents need to **produce** valid DialogueLines, not just markdown blobs. If we give agents Write access without giving them the format spec, we recreate the same mismatch: subagents hallucinate markers, judges reject them.

[TENSION T01: Agent instruction delivery gap]

The spike says Judge protocol gets updated to tell agents "write to {{OUTPUT_FILE}}". But **how do agents know what to write?** The current prompt gives them prose: "use [PERSPECTIVE Pnn: label]". RFC 0028's `specification_markdown()` method exists but isn't connected to agent delivery.

The unified RFC needs to specify: agents receive BOTH the output path AND the format spec. Otherwise we're just moving the regex problem from linter to agent hallucination.

### Cupcake 🧁

[PERSPECTIVE P01: The file-based architecture IS the format contract's distribution mechanism]

The spike proposes `/tmp/blue-dialogue/{slug}/round-{n}/{agent}.md` as an efficiency optimization. RFC 0028 proposes `DialogueFormat::specification_markdown()` embedded in Judge protocols as a format contract.

These aren't competing — they're symbiotic. The file-based approach solves *where agents write*. The format contract solves *what they write*. The unified RFC should position file-based output as the delivery vehicle for structured dialogue lines.

**Key integration point**: The `{{OUTPUT_FILE}}` instruction in agent prompts should *reference* the format specification. Instead of just "write to this file," it becomes "write DialogueLine-conformant markdown to this file." The Judge can then parse using `DialogueFormat::parse_markers()` without transformation.

[TENSION T01: Validation timing — at write or at read?]

The spike assumes agents write arbitrary markdown that Judge reads raw. RFC 0028 assumes Judge parses and validates using `DialogueFormat::parse()`. If agents write malformed markers, when does validation happen?

**Option A**: Agents validate their own output before writing (requires exposing `blue_dialogue_lint` to subagents).
**Option B**: Judge reads, parses, reports errors in scoring (current path).
**Option C**: Format contract lives in agent system prompts as executable specification — agents self-enforce through prompt instruction, not tooling.

### Scone 🧁

[PERSPECTIVE P01: Interface Boundary Confusion]

The spike proposes agents Write to `/tmp`, while RFC 0028 defines a format contract in `blue-core`. These solve orthogonal problems at different system boundaries:

- **File-based output** (spike) = transport layer (how data moves from agent to judge)
- **DialogueLine/DialogueFormat** (RFC 0028) = schema layer (what format the data takes)

Combining them into one RFC conflates "where the bits go" with "what the bits mean." The judge needs *both*: it reads from `/tmp/{agent}.md` (transport) then parses with `DialogueFormat::parse_markers()` (schema).

[TENSION T01: Premature Unification]

The spike is still "In Progress" and calls itself a recommendation, not a decision. RFC 0028 is "Draft" with a 4-phase migration plan already scoped. Forcing them into a single RFC creates artificial coupling:

- If we discover `/tmp` approach has Write tool trust issues, we'd block the entire format contract migration
- If `DialogueFormat` needs iteration during Phase 1 tests, we'd drag the transport layer into the churn

**Proposal**: Keep them separate. RFC 0029 (file-based subagent output) depends on RFC 0028 (format contract). Clean dependency arrow, independent evolution.

### Eclair 🧁

[PERSPECTIVE P01: Separation of concerns — transport vs. schema]

The spike proposes changing HOW subagent output travels (JSONL extraction to direct file write). RFC 0028 proposes changing WHAT gets transported and parsed (regex to DialogueFormat contract).

These are orthogonal concerns:
- **Transport layer** — file-based output (/tmp paths, Write tool, Judge reads directly)
- **Schema layer** — format contract (DialogueLine enum, parse/render pair, no regex)

Combining them into one RFC risks conflating implementation choices. The file-based transport could work with regex parsing OR the DialogueFormat contract. The contract could work with JSONL extraction OR direct file writes. Neither depends on the other.

[TENSION T01: Which concern deserves RFC status?]

The spike solves an **optimization problem** — faster data collection. RFC 0028 solves a **correctness problem** — generator/linter/parser divergence causes real bugs.

**Recommendation**: RFC 0028 owns the format contract. Add Phase 0 specifying the file-based transport mechanism. The transport serves the contract, not vice versa.

### Donut 🧁

[PERSPECTIVE P01: separable concerns masquerading as unity]

These two proposals solve orthogonal problems. The spike optimizes **transport** (Judge to subagent output collection), while RFC 0028 standardizes **format** (how dialogue lines encode meaning). Merging them risks bundling an I/O optimization with a format contract — two stability profiles, two deployment risks.

Consider: if `/tmp` file writes fail (disk full, permission issues), does that invalidate the format contract? If the format contract evolves (add new marker types), must we re-test file I/O? No. They're coupled only by timing: both touch dialogue infrastructure now.

[TENSION T01: file-based output breaks MCP containment boundary]

The spike proposes subagents write to `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md` using the Write tool. But this violates the task isolation model: outputs should flow **through** the task system, not **around** it via filesystem side-channels.

What happens when:
- Multiple dialogues run concurrently (slug collision detection?)
- A subagent crashes mid-write (partial file, Judge reads garbage?)
- MCP runs in a container where `/tmp` isn't shared with the host?

The JSONL extraction is overhead, yes — but it's **intentional encapsulation**. The task system owns outputs. Bypassing it trades architectural clarity for microbenchmark wins.

### Brioche 🧁

[PERSPECTIVE P01: Integration surface — where file-based output meets format contract]

The spike and RFC 0028 solve adjacent problems that share a boundary: **the text that agents write**. The spike optimizes *where* that text goes. RFC 0028 standardizes *what* that text contains. The integration point is the Write tool instruction in the agent prompt.

The spike proposes: "WRITE YOUR OUTPUT: Use the Write tool to write your complete response to: {{OUTPUT_FILE}}"

But RFC 0028 gives us `DialogueFormat::specification_markdown()` — a rendered contract that tells agents *how* to format their output. These should merge: agents receive BOTH the output path AND the format spec.

The Judge protocol then references the same contract: "Read each agent's output from /tmp/blue-dialogue/{slug}/round-{n}/{name}.md and validate against DialogueFormat::parse_markers()."

[TENSION T01: Two migration paths, same target]

RFC 0028 has a 4-phase migration touching generator, linter, parser, and Judge protocol. The spike proposes changes to Judge protocol and agent templates. If we combine these into one RFC, do we serialize migrations (format contract first, then file output) or interleave them?

### Croissant 🧁

[PERSPECTIVE P01: State Synchronization Gap]

The unified RFC risks introducing a critical race condition. If subagents write formatted `DialogueLine` variants directly to `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`, but the Judge consumes those files using `DialogueFormat::parse()` before all agents complete writing, the parse operation could fail mid-file or read incomplete marker lines.

The contract RFC assumes parsing **complete documents** (full dialogue files with metadata, panels, rounds). The spike assumes parsing **fragments** (single-agent perspectives). These are different parse targets requiring different tolerances. A fragment parser needs to handle:
- Missing metadata (agent perspective has no **Topic** field)
- No section headings (agent writes markers, not `## Round N`)
- Partial marker sequences (agent might emit `[PERSPECTIVE P01` without closing bracket if output truncates)

[TENSION T01: Two Parsers or Parameterized Tolerance]

Should `DialogueFormat::parse()` gain a fragment mode, or should we introduce `DialogueFormat::parse_fragment()` as a separate entry point? Fragment parsing needs graceful degradation, but the linter needs strict validation. Combining both into one method with a boolean flag feels brittle.

### Macaron 🧁

[PERSPECTIVE P01: These are orthogonal layers, not parallel concerns]

The spike addresses **transport** — how Judge collects subagent output. RFC 0028 addresses **schema** — how dialogue lines are structured and parsed. Unifying them would couple an I/O optimization to a format contract. That's a design error.

**Evidence from the specs:**

Spike changes: where output goes, how Judge reads it, what gets eliminated (JSONL parsing, MCP round-trips).

RFC 0028 changes: `DialogueLine` enum, `DialogueFormat::parse()`, marker extraction logic, elimination of regex patterns across four consumers.

**These can proceed independently:**
- Judge can adopt file-based output *without* changing how it parses markers from those files
- RFC 0028's format contract can ship *without* changing where subagent output gets written
- They share zero implementation surface — one touches `dialogue.rs` agent spawn logic, the other touches `blue-core/dialogue_format.rs`

[TENSION T01: The spike's recommendation is incomplete]

The spike recommends `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md` for output paths but doesn't specify **what format** gets written to those files. If agents write raw markdown containing `[PERSPECTIVE P01: ...]` lines, then the Judge still needs a parser. That's where RFC 0028 comes in — but the spike doesn't mention this dependency.

**Integration point:** Judge reads agent file, then calls `DialogueFormat::parse_markers(content)` to extract structured markers. Keep them separate RFCs with a single dependency edge.

### Cannoli 🧁

[PERSPECTIVE P01: The Contract Is The Boundary]

The spike proposes `/tmp` file writing to eliminate extraction overhead. RFC 0028 proposes shared types to eliminate format divergence. These aren't separate concerns — **they're the same boundary problem at different stack layers**.

The spike eliminates the JSONL-to-JSON-to-text boundary by making agents write markdown directly. RFC 0028 eliminates the regex-to-string-to-struct boundary by making all components speak `DialogueLine`. Both say: **stop translating between representations; use the contract format natively**.

The unified RFC should recognize this: subagents write `DialogueLine`-compliant markdown to `/tmp` files, and the Judge reads that markdown using `DialogueFormat::parse_markers()`. The file-based output ISN'T an alternative to the format contract — it's a delivery mechanism FOR the contract.

[PERSPECTIVE P02: The Round Path Insight]

The spike's recommendation (`/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`) creates a **persistent dialogue state on disk** that mirrors the final `.dialogue.md` file structure. This isn't just collision avoidance — it's a debugging gift.

If we combine this with RFC 0028's typed parsing, we get: the Judge reads round-N agent files using `DialogueFormat::parse_markers()`, validates them immediately, and can flag format violations BEFORE appending to the dialogue. The round-scoped paths become a staging area where the contract is enforced.

[TENSION T01: Write Tool Surface Area vs Contract Enforcement]

The spike notes Write tool adds "surface area" — agents could write anywhere. But if we're giving agents a format contract (`DialogueFormat::specification_markdown()`), shouldn't we also constrain WHERE they write? Could the Judge verify file existence + parse validity before scoring? That turns the file write from a trust issue into a contract check.

### Strudel 🧁

[PERSPECTIVE P01: Atomic writes vs partial reads]

The spike proposes agents write directly to `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`, and Judge reads them. Critical edge case: **what if Judge reads while agent is still writing?**

File I/O isn't atomic. An agent streaming 400 words takes time. If Judge spawns 5 agents with `run_in_background: true`, finishes scoring agent 1's output, and starts reading agent 2's file while agent 2 is mid-write, Judge gets truncated content. No error — just partial perspective with missing `[TENSION]` markers.

This wasn't a problem with JSONL extraction because task output only becomes readable **after** task completion. The extraction happened post-execution. Direct file writes break that guarantee.

**Solutions:**
- Write to `.tmp` suffix, rename on completion (atomic on POSIX)
- Judge waits for task completion before reading (requires task handle tracking)
- Use write-lock protocol (agent writes `.lock` file, Judge checks)

[TENSION T01: Two RFCs or one unified?]

RFC 0028 defines `DialogueFormat::render()` and `parse()` for the **dialogue document** — the cumulative markdown file with all rounds, scoreboard, and metadata. The spike defines **agent output files** — individual perspectives in `/tmp`.

These are different formats serving different purposes. If we unify into one RFC, does `DialogueFormat::parse()` handle both? Or do we need `AgentOutputFormat::parse_markers()` vs `DialogueDocumentFormat::render()`?

### Beignet 🧁

[PERSPECTIVE P01: Temporal Boundaries Define Component Responsibilities]

The spike proposes **file-based I/O** (subagents writing /tmp markdown), while RFC 0028 proposes a **shared format contract** (structured parsing/rendering). These operate at different temporal boundaries in the dialogue lifecycle:

- **Before execution**: DialogueFormat renders protocol instructions that tell agents what markers to write
- **During execution**: Agents write raw markdown to /tmp (spike's contribution)
- **After execution**: Judge reads /tmp files, DialogueFormat parses markers for scoring

The unified RFC should make this temporal separation explicit. The contract defines WHAT gets written; the file-based output defines WHERE it gets written.

[PERSPECTIVE P02: File Paths Are Part of the Protocol Contract]

Current spike treats output paths as implementation detail. But the Judge protocol must communicate these paths to agents — so path structure becomes part of the format contract.

RFC 0028's `DialogueFormat::specification_markdown()` should include file I/O instructions. Both concerns merge naturally: the contract specifies format AND delivery mechanism.

[TENSION T01: Where Does DialogueLine Classification Happen?]

Spike eliminates `blue_extract_dialogue` MCP calls by having Judge read /tmp files directly. But who classifies lines into DialogueLine enums?

- Option A: Judge does it after reading (keeps agents simple)
- Option B: Agents write structured output, Judge parses format contract directly

Recommend A — classification stays Judge-side.

### Churro 🧁

[PERSPECTIVE P01: MCP surface area vs orchestration boundaries]

The spike proposes adding Write tool access to alignment-expert agents, but this crosses an MCP tool boundary that RFC 0028 never considered. RFC 0028 assumes the DialogueFormat contract governs **content structure** (what's written), not **delivery mechanism** (how it's written). The file-based output shifts orchestration responsibility from Task tool JSONL extraction to explicit file I/O. This creates two coupled but distinct contracts:

1. **Structural contract** (RFC 0028): DialogueLine enum, marker syntax, heading patterns
2. **Protocol contract** (spike): Who writes where, when Judge reads, /tmp path conventions

The spike's `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md` pattern is an **orchestration decision**, not a format decision.

[TENSION T01: Single RFC risks conflating structural validation with file orchestration]

If we merge these into one RFC, we risk creating a contract that couples "what valid dialogue markdown looks like" with "where subagents write their output files." The DialogueLine enum should be reusable beyond alignment dialogues, but the /tmp file protocol is specific to multi-round dialogue orchestration.

However, splitting them creates coordination overhead. Should the unified RFC have two distinct sections (Format Contract + Orchestration Protocol), or should these be separate RFCs with explicit dependencies?

## 💙 Judge: Round 0 Assessment

**Round 0 Summary**: Strong opening with a clear 7-5 split.

**SEPARATE camp** (Scone, Eclair, Donut, Croissant, Macaron, Strudel, Churro): Transport and schema are orthogonal concerns. Coupling them creates artificial dependencies, blocks independent evolution, and risks conflating I/O optimization with correctness fixes. Two RFCs with a dependency edge.

**UNIFY camp** (Muffin, Cupcake, Brioche, Cannoli, Beignet): The concerns share a boundary — the text agents write. File-based output is the delivery mechanism for the format contract. Separating them risks an incomplete spike (no format spec) and an ungrounded contract (no delivery mechanism).

**Key tensions to resolve in Round 1**:
1. Fragment vs document parsing (Croissant, Strudel) — does DialogueFormat need two parse modes?
2. Atomic writes / race conditions (Strudel) — task completion as read barrier
3. MCP containment (Donut) — is bypassing Task output a boundary violation?
4. The dependency question (Macaron) — if separate, what's the dependency edge?

**Scores favor the separatists**: Scone (12), Croissant (12), Macaron (12) all scored higher for grounded, evidence-based arguments. The unifiers have strong integration instincts but haven't yet answered the coupling risk.

## Round 1: Convergence

### Muffin 🧁

[CONCESSION: Coupling risk is real — separate RFCs with explicit dependency]

Round 0 I argued the format contract governs transport AND schema. I was wrong about the coupling direction. Scone and Macaron showed these concerns have **zero shared implementation surface** — one touches `dialogue.rs` agent spawn logic, the other touches `blue-core/dialogue_format.rs`.

[PERSPECTIVE P02: Fragment parsing IS the dependency edge]

The concrete dependency between the two RFCs is `DialogueFormat::parse_markers()`. RFC 0029 (file-based output) produces files; RFC 0028 (format contract) provides the parser. The dependency is a single function call, not architectural coupling. That's the cleanest possible separation.

[TENSION T02: MCP containment — beneficial restriction or unnecessary overhead?]

Donut raised MCP containment in Round 0. Task completion as read barrier resolves the race condition, but the broader question remains: is the Task system's output encapsulation something we should preserve or bypass? The staging area pattern (round-scoped paths) sidesteps this rather than answering it.

### Cupcake 🧁

[CONCESSION: Two RFCs with explicit dependency — RFC 0028 ships first]

I argued file-based output IS the format contract's distribution mechanism. The separatists convinced me: distribution and definition are different concerns. The format contract can ship without changing where output goes. The file-based output can ship without changing how parsing works — but it SHOULD use the format contract's parser.

[PERSPECTIVE P02: RFC 0028 ships first because RFC 0029 needs parse_markers()]

The dependency arrow is clear: RFC 0029 depends on RFC 0028. The spike acknowledged agents write markdown containing `[PERSPECTIVE P01: ...]` markers. The Judge needs to parse those markers. `DialogueFormat::parse_markers()` is that parser. Ship the contract, then ship the transport.

[RESOLVED T01: Validation timing resolved — Option C wins]

All experts converge on Option C from my Round 0 tension: format contract lives in agent system prompts as executable specification. Agents self-enforce through prompt instruction. Judge validates at read time using `parse_markers()`. No need for agents to call `blue_dialogue_lint`.

### Scone 🧁

[CONCESSION: File paths ARE part of a protocol contract — just not THIS format contract]

Beignet's P02 was right: `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md` is a contract. But it's a **protocol** contract (who writes where, when), not a **format** contract (what valid dialogue lines look like). This proves my Round 0 point — orthogonal concerns deserve separate RFCs.

[PERSPECTIVE P02: Integration surface exists at read boundaries, not write boundaries]

The integration point between the two RFCs is the Judge's read operation: `let content = read(path); let markers = DialogueFormat::parse_markers(&content);`. That's one line of glue code. The write side (agent → file) and the parse side (file → struct) are independent. Clean boundary.

[RESOLVED T01: Premature unification risk eliminated by separate RFCs]

Two RFCs with a dependency edge means RFC 0028 can iterate its `DialogueLine` enum without touching file I/O, and RFC 0029 can change path conventions without touching the parser. Independent evolution confirmed.

### Eclair 🧁

[CONCESSION: The representation translation argument clinches separation]

Macaron showed these share zero implementation surface. The dependency is protocol-level (RFC 0029 calls RFC 0028's `parse_markers()`), not implementation-level (no shared code paths during migration). Two separate RFCs is correct.

[PERSPECTIVE P02: Dependency is protocol-level, not implementation-level]

RFC 0029's migration (replace JSONL extraction with file reads) can proceed independently of RFC 0028's migration (replace regex with `DialogueLine`). They converge only when the Judge reads agent files AND parses them with the new contract. That convergence point is a protocol dependency, not a code dependency.

[RESOLVED T01: Both concerns deserve RFC status]

The spike solves transport efficiency. RFC 0028 solves format correctness. Both are real problems with real solutions. Neither subsumes the other. Two RFCs.

### Donut 🧁

[CONCESSION: MCP containment preserved via staging area pattern]

I raised the containment concern — agents writing to `/tmp` bypasses Task system output. But Cannoli's staging area insight (P02, Round 0) and Strudel's task-completion-as-read-barrier resolve this:

1. Agents write to round-scoped paths (staging area)
2. Judge waits for Task completion before reading (read barrier)
3. Judge validates with `parse_markers()` before scoring (contract enforcement)

The Task system still owns completion semantics. File I/O is a staging area, not a bypass.

[PERSPECTIVE P02: Task completion provides the atomic read barrier]

The race condition (Judge reads while agent writes) is solved by existing infrastructure: `run_in_background: true` agents have Task handles. Judge waits for task completion, then reads the file. No `.lock` files, no `.tmp` renames. The Task system already provides the barrier.

[RESOLVED T01: MCP containment not broken — staging area is a valid pattern]

### Brioche 🧁

[CONCESSION: Zero shared implementation surface — I was wrong about the integration depth]

I argued the integration surface (where file output meets format contract) was deep enough to justify a single RFC. Macaron proved otherwise: one touches `dialogue.rs` spawn logic, the other touches `blue-core/dialogue_format.rs`. The integration is a single function call at read time, not shared code.

[PERSPECTIVE P02: Three parse targets, not two]

RFC 0028 actually serves three parse targets, not two:
1. **Document parsing** — full `.dialogue.md` files (linter, renderer)
2. **Fragment parsing** — single-agent output files (Judge reading `/tmp` files)
3. **Marker extraction** — inline markers within prose (`parse_expert_response`)

Fragment parsing (target 2) is what RFC 0029 needs. It's already scoped as `parse_markers()` in RFC 0028. Separate RFCs with shared type dependency.

[RESOLVED T01: Two migration paths converge on same target — no conflict]

RFC 0028's 4-phase migration and RFC 0029's transport migration are independent. They converge when Phase 2 (generator migration) aligns with RFC 0029's Judge protocol update. Serial, not conflicting.

### Croissant 🧁

[REFINEMENT: Staging area resolves the synchronization gap — switching to unified view]

My Round 0 concern about race conditions was valid but solvable. Cannoli's staging area insight + Donut's task-completion barrier = no race condition. Round-scoped paths (`/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`) provide natural isolation.

[PERSPECTIVE P02: DialogueFormat as assembly-time validator]

The staging area pattern creates a natural validation checkpoint: Judge reads agent file → `parse_markers()` validates format → only valid output gets assembled into the dialogue document. The format contract becomes an **assembly-time validator**, not just a lint-time checker.

This is stronger than either RFC alone: RFC 0028 provides the validator, RFC 0029 provides the staging area where validation happens. Whether this lives in one RFC or two is secondary — the architecture is the same either way. I lean toward unified because the staging-area-as-validation-checkpoint is a novel insight that neither document captures alone.

[RESOLVED T01: Fragment parser resolved — parse_markers() already handles fragments]

### Macaron 🧁

[CONCESSION: The spike's incompleteness IS the dependency edge]

My Round 0 tension (spike is incomplete) is actually the strongest argument for separate RFCs: the spike NEEDS RFC 0028 to be complete. The spike says "agents write markdown with markers" but doesn't specify marker format. RFC 0028 specifies marker format. The dependency is real and directional.

[PERSPECTIVE P02: Spike needs RFC 0028 for validation — the dependency arrow proves separation]

Evidence: The spike proposes Judge reads `/tmp` files. But what does Judge do with the content? Parse it. With what? `DialogueFormat::parse_markers()`. Where is that defined? RFC 0028. The spike cannot function without the format contract. That's a dependency, not a merge signal.

[RESOLVED T01: Spike's recommendation completed by RFC 0028 dependency]

### Cannoli 🧁

[REFINEMENT: Fragment vs document distinction proves these are separate concerns]

My Round 0 P01 ("The Contract Is The Boundary") was too abstract. The concrete truth: RFC 0028's `parse()` handles documents, `parse_markers()` handles fragments. RFC 0029 only needs `parse_markers()`. The fragment/document distinction maps cleanly to separate RFCs.

[PERSPECTIVE P02: Separate RFCs with shared DialogueLine — the type is the contract]

The `DialogueLine` enum is the shared vocabulary. RFC 0028 defines it. RFC 0029 consumes it (via `parse_markers()`). This is standard library-consumer separation. The type definition lives in `blue-core`, both RFCs reference it.

[RESOLVED T01: Write tool surface area contained by task completion barrier + parse validation]

### Strudel 🧁

[REFINEMENT: Round-scoped paths + task barriers resolve the race condition — unify makes sense]

My Round 0 atomic-write concern is fully resolved: task completion provides the read barrier, round-scoped paths provide isolation. The staging area pattern (Cannoli P02) means `/tmp/blue-dialogue/{slug}/round-{n}/` acts as a per-round workspace.

[PERSPECTIVE P02: File-based output enables format contract enforcement at read boundary]

With file-based output, the Judge can validate EVERY agent's output against `DialogueFormat::parse_markers()` before assembling the dialogue document. This is stronger than the current approach where validation only happens at lint time, after assembly.

The unified view captures this: transport (file paths) enables schema enforcement (parse at read time). Separating them loses this architectural insight. I still lean unified, but acknowledge the dependency-edge argument is strong.

[RESOLVED T01: Two RFCs or one — the architecture is the same either way]

### Beignet 🧁

[CONCESSION: Fragment vs document distinction is real — Croissant was right]

My Round 0 P02 argued file paths are part of the protocol contract. That's true, but Croissant correctly identified these are DIFFERENT contracts: format contract (DialogueLine structure) vs protocol contract (path conventions, write timing, read barriers).

[PERSPECTIVE P02: Path = protocol contract, not format contract]

`/tmp/blue-dialogue/{slug}/round-{n}/{name}.md` is a protocol decision. `[PERSPECTIVE P01: description]` is a format decision. Different concerns, different RFCs. The protocol RFC (0029) depends on the format RFC (0028) for `parse_markers()`, but not vice versa.

[RESOLVED T01: Classification happens Judge-side at read time — Option A confirmed]

All experts agree: agents write markdown with markers per prompt instructions, Judge classifies using `DialogueLine::classify()` at read time. No classification in agents.

### Churro 🧁

[REFINEMENT: Two translation layers confirm two RFCs]

My Round 0 tension (single RFC risks conflating structural validation with file orchestration) is now the consensus. The two concerns map to two translation layers:

1. **Representation translation** (RFC 0028): regex → DialogueLine enum, ad-hoc strings → typed markers
2. **Transport translation** (RFC 0029): JSONL extraction → direct file read, Task output → staging area

[PERSPECTIVE P02: Two translation layers — structural and protocol]

Each RFC eliminates one category of unnecessary translation. RFC 0028 eliminates 16+ regex patterns and 4 independent parsers. RFC 0029 eliminates 15 MCP calls, 15 dir walks, and 15 JSONL parses per dialogue. Different waste, different fixes, different RFCs.

[TENSION T02: Stream vs document modes remain unresolved]

When agent output exceeds the Write tool buffer, does the agent stream multiple writes to the same file? RFC 0029 should specify: one Write call per agent per round, output must fit in a single Write. If it doesn't, the Task system's JSONL approach is still needed as fallback. This tension belongs in RFC 0029, not RFC 0028 — further proof of separation.

## 💙 Judge: Round 1 Assessment — CONVERGENCE ACHIEVED

**Round 1 Result**: 10-2 favoring SEPARATE with explicit dependency edge.

### Position Shifts

| Agent | Round 0 | Round 1 | Shift |
|-------|---------|---------|-------|
| 🧁 Muffin | UNIFY | SEPARATE | Conceded coupling risk |
| 🧁 Cupcake | UNIFY | SEPARATE | Conceded distribution ≠ definition |
| 🧁 Scone | SEPARATE | SEPARATE | Strengthened with read-boundary insight |
| 🧁 Eclair | SEPARATE | SEPARATE | Confirmed protocol-level dependency |
| 🧁 Donut | SEPARATE | SEPARATE | Conceded staging area resolves containment |
| 🧁 Brioche | UNIFY | SEPARATE | Conceded zero shared implementation |
| 🧁 Croissant | SEPARATE | UNIFY | Staging area as validation checkpoint |
| 🧁 Macaron | SEPARATE | SEPARATE | Dependency arrow proves separation |
| 🧁 Cannoli | UNIFY | SEPARATE | Fragment/document distinction proves it |
| 🧁 Strudel | SEPARATE | UNIFY | Read-boundary enforcement insight |
| 🧁 Beignet | UNIFY | SEPARATE | Path = protocol, not format |
| 🧁 Churro | SEPARATE | SEPARATE | Two translation layers confirmed |

### Tension Resolution Summary

| Tension | Resolution |
|---------|------------|
| Agent instruction delivery gap (Muffin T01) | Format spec embedded in agent prompts via `specification_markdown()` |
| Validation timing (Cupcake T01) | Option C — agents self-enforce via prompt, Judge validates at read time |
| Premature unification (Scone T01) | Two RFCs with dependency edge — independent evolution confirmed |
| Which concern deserves RFC status (Eclair T01) | Both — transport efficiency and format correctness are separate problems |
| MCP containment (Donut T01) | Staging area + task completion barrier preserves containment |
| Two migration paths (Brioche T01) | Independent migrations converge at Judge protocol update |
| Two parsers or parameterized tolerance (Croissant T01) | `parse_markers()` already handles fragments — no fragment mode needed |
| Spike incomplete (Macaron T01) | Spike depends on RFC 0028 for `parse_markers()` — dependency is the answer |
| Write tool surface area (Cannoli T01) | Task completion barrier + parse validation constrains writes |
| Two RFCs or one (Strudel T01) | Architecture is the same either way — two RFCs chosen for independent evolution |
| Classification location (Beignet T01) | Judge-side at read time — Option A confirmed by all |
| Structural vs file orchestration (Churro T01) | Two translation layers, two RFCs |

### Remaining Open Tensions

| Tension | Owner | Status |
|---------|-------|--------|
| Stream vs document modes (T02) | Churro | Deferred to RFC 0029 |
| MCP containment philosophy (T02) | Muffin | Acknowledged, not blocking |

### Convergence Declaration

**ALIGNMENT VELOCITY**: Round 0 → Round 1 saw 5 position shifts (Muffin, Cupcake, Brioche, Cannoli, Beignet moved UNIFY→SEPARATE; Croissant, Strudel moved SEPARATE→UNIFY). Net movement: +3 toward SEPARATE. The 10-2 consensus is stable — the 2 UNIFY holdouts (Croissant, Strudel) acknowledge the architecture is identical either way and their preference is organizational, not technical.

**CONVERGENCE ACHIEVED** at Round 1 with Total ALIGNMENT score of 287.

### Consensus Architecture

All 12 experts agree on the following architecture:

1. **RFC 0028 (Dialogue Format Contract)** — ships first
   - `DialogueLine` enum with 8 variants in `blue-core::dialogue_format`
   - `DialogueFormat::parse()` for document validation
   - `DialogueFormat::parse_markers()` for fragment extraction
   - `DialogueFormat::render()` for document generation
   - `DialogueFormat::specification_markdown()` for agent prompt embedding
   - No regex — string methods only
   - 4-phase migration: contract module → generator → linter → alignment parser

2. **RFC 0029 (File-Based Subagent Output)** — ships second, depends on RFC 0028
   - Round-scoped paths: `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`
   - Agents write markdown with markers per `specification_markdown()` prompt
   - Task completion as atomic read barrier
   - Judge reads files, validates with `parse_markers()`, assembles dialogue
   - Eliminates: 15 MCP calls, 15 dir walks, 15 JSONL parses per dialogue

3. **Integration point**: One function call — `DialogueFormat::parse_markers(content)`
   - RFC 0029 produces the files
   - RFC 0028 provides the parser
   - Judge glue code: `let content = read(path); let markers = parse_markers(&content);`

---

*"Two RFCs. One dependency edge. Ship the contract, then ship the transport."*

— 💙 Judge
