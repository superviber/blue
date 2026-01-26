# Spike: Dialogue Generation Linter Mismatch

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 30 minutes |

---

## Question

Why does the alignment dialogue generation code + Judge protocol produce output that fails the dialogue linter? What specific mismatches exist between what the generation scaffold creates, what the Judge protocol instructs agents to produce, and what the linter validates?

---

## Findings

Three components interact to produce a dialogue: the **scaffold generator** (`generate_alignment_dialogue_markdown` in `dialogue.rs:805-885`), the **Judge protocol** (`build_judge_protocol` in `dialogue.rs:887-1015`), and the **linter** (`dialogue_lint.rs`). These three disagree on format in several places. The linter is the authority, but the generator and Judge protocol don't always produce what it expects.

### Mismatch 1: Round numbering — generator and linter wrong, agents correct

Round numbering starts at 0. Opening Arguments is Round 0.

| Component | Behavior | Correct? |
|-----------|----------|----------|
| **Generator** | Created `## Round 1: Opening Arguments` (line 878) | **No** — fixed to Round 0 |
| **Judge protocol** | Says nothing about round numbering | Missing — should specify 0-based |
| **Linter** | `check_round_numbering` required first round `1`; `check_round_sequencing` required `[1, 2, ..., N]` | **No** — fixed to 0-based |
| **Agents** | Wrote `## Round 0: Opening Arguments` | **Yes** |

**Root cause:** Generator and linter both hardcoded 1-based round numbering. The Judge protocol was silent. Agents naturally used 0-based numbering, which is correct. Fixed in generator (`dialogue.rs:878`), linter (`dialogue_lint.rs:486, 595`), and associated tests.

### Mismatch 2: Agent header emoji order — generator vs Judge protocol vs linter

| Component | Format |
|-----------|--------|
| **Generator** | `### {} {}` → `### Muffin 🧁` (name first, line 880) |
| **Judge protocol** | "Round headers use emoji prefix (### 🧁 Muffin)" (line 989) |
| **Linter** | Regex `r"###\s*(\w+)\s*([🧁💙]?)"` — expects name first, emoji second |

**Root cause:** The Judge protocol instructs emoji-first (`### 🧁 Muffin`) but the generator scaffold and the linter regex both assume name-first (`### Muffin 🧁`). If agents follow the Judge protocol, the linter's `emoji-consistency` check can't parse the emoji. If they follow the scaffold, the Judge protocol is violated.

### Mismatch 3: Judge assessment sections not in linter's model

The Judge adds `## 💙 Judge: Round N Assessment` sections as h2 headings. These are neither rounds (`## Round N`) nor agent responses (`### Name 🧁`). The linter's round regex `r"(?i)##\s*Round\s+(\d+)"` doesn't match them (correct), but it also doesn't account for them in the document structure model. If a Judge writes `## Round 0 Assessment` without the `💙 Judge:` prefix, it would be parsed as a round and break sequencing.

**Root cause:** No explicit format specification for Judge assessment sections in either the protocol or the linter.

### Mismatch 4: Perspective ID width — protocol vs linter

| Component | Behavior |
|-----------|----------|
| **Agent prompt** | Says `[PERSPECTIVE Pnn: brief label]` — implies 2-digit |
| **Linter** | Regex `r"(?i)\[\s*PERSPECTIVE\s+P(\d{2})\s*:"` — strictly 2-digit |
| **Risk** | If an agent writes `[PERSPECTIVE P1: ...]` (1 digit), the linter silently ignores it |

**Root cause:** The agent prompt template uses `Pnn` which looks like a template placeholder, not a format directive. Agents may use P1 instead of P01. The linter won't parse single-digit IDs, leading to missing entries in the Perspectives Inventory.

### Mismatch 5: Scoreboard row regex fragility

The linter's scoreboard row regex:
```
r"\|\s*([🧁💙]?\s*\w+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*\*\*(\d+)\*\*\s*\|"
```

This assumes:
- Agent name is a single `\w+` word — fails for "Multi Word" names or emoji-prefixed without space
- Score columns are bare digits — fails if agent writes `3/3` or adds notes
- Total is bold `**N**` — fails if agent doesn't bold it

**Root cause:** The regex is tightly coupled to the scaffold's exact output format. Any variation by the Judge when updating scores breaks parsing. The `scoreboard-math` check then silently passes (no data to verify) rather than failing.

### Mismatch 6: No format contract between components

The deepest root cause: there is no shared format contract. The generator, Judge protocol, and linter were built independently. Each encodes its own assumptions:

- Generator assumes its scaffold format is canonical
- Judge protocol instructs a slightly different format (emoji-first headers)
- Linter validates against its own regex patterns

**There is no single source of truth for "what a valid dialogue looks like."**

## Summary

| # | Mismatch | Severity | Status |
|---|----------|----------|--------|
| 1 | Round numbering: generator and linter used 1-based, should be 0-based | Major ×2 | **Fixed** |
| 2 | Agent header emoji order | Minor | Open |
| 3 | Judge assessment section format | Minor | Open |
| 4 | Perspective ID width (P1 vs P01) | Major | Open |
| 5 | Scoreboard regex fragility | Minor | Open |
| 6 | No shared format contract | Systemic | Open |

## Recommendation

Recommends implementation for the remaining open mismatches:

1. ~~Round numbering~~ — **Fixed.** Generator, linter, and tests updated to 0-based.
2. **Settle emoji order**: Pick name-first (matches generator + linter regex) and update Judge protocol line 989 to match
3. **Specify Judge assessment format**: Add `## 💙 Judge: Round N Assessment` to the protocol as mandatory format
4. **Clarify perspective ID format**: Change agent prompt from `Pnn` to explicit "P01, P02, ... (always two digits)"
5. **Loosen scoreboard regex**: Handle multi-word agent names and emoji-prefixed names
