# RFC 0037: Single-Source Protocol Authority

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Dialogue** | [Remove SKILL.md / build_judge_protocol Redundancy](/tmp/blue-dialogue/remove-skill-protocol-redundancy/dialogue.md) |
| **Depends On** | RFC 0033 (round-scoped dialogue files), RFC 0036 (expert output discipline) |

---

## Summary

The alignment dialogue system had two sources of truth describing the same workflow: `SKILL.md` (315-line skill file) and `build_judge_protocol()` in `dialogue.rs` (Rust function generating runtime JSON). This RFC strips SKILL.md to a thin entry point (73 lines) and establishes `build_judge_protocol()` as the sole authority for all runtime behavior.

## Problem

`SKILL.md` and `build_judge_protocol()` both contained:
- Round workflow steps (7-step process)
- Agent prompt template structure
- File architecture (scoreboard.md, tensions.md, round-N.summary.md)
- Artifact writing instructions
- Context instructions for round 1+
- Scoring dimensions and convergence criteria

When one changed, the other had to change. They had already drifted — SKILL.md lacked artifact writing instructions while the protocol included them, causing the Judge to skip writing scoreboard.md, tensions.md, and round-N.summary.md.

**Root cause**: SKILL.md tried to be both a discovery document and a runtime specification.

## Analysis (from 5-Expert Alignment Dialogue)

The dialogue explored three approaches and reached 5/5 convergence on Approach C (validated redundancy). However, post-dialogue review overruled that conclusion: **we don't need to build for human operators**. SKILL.md is not read by humans browsing documentation — it's loaded by the skill system at invocation time, immediately before `blue_dialogue_create` returns the Judge Protocol.

This makes Approach A (strip SKILL.md) correct. The dialogue's concern about "orphaning human operators" was based on a false premise.

## Solution

### Strip SKILL.md to Entry Point

SKILL.md retains only:
- **Frontmatter** — name, description (required by skill system)
- **Usage** — invocation syntax and parameters
- **How It Works** — 3-line summary: call `blue_dialogue_create`, get protocol, follow it
- **Expert Selection** — pastry names and tier distribution
- **Blue MCP Tools** — tool names and one-line descriptions
- **Key Rules** — 3 rules (don't participate, spawn parallel, follow protocol)
- **The Spirit of the Dialogue** — philosophy (unique to SKILL.md, not in protocol)

SKILL.md does NOT contain:
- Round workflow steps
- Agent prompt template (or any description of it)
- File architecture diagram
- Artifact writing instructions
- Context instructions for round 1+
- Scoring dimensions or formulas
- Convergence criteria
- `.dialogue.md` format specification

All of the above live exclusively in `build_judge_protocol()`.

### Authority Hierarchy

| Source | Role | Contains |
|--------|------|----------|
| `build_judge_protocol()` | Runtime protocol | Everything: workflow, prompts, file paths, scoring, convergence |
| `SKILL.md` | Thin entry point | Invocation syntax, parameters, "call blue_dialogue_create and follow the protocol" |

There is no overlap to validate. No sync test needed.

## What Changed

- **SKILL.md**: 315 lines → 73 lines. Removed all workflow, scoring, file architecture, template details, and `.dialogue.md` format specification.
- **No code changes**: `build_judge_protocol()` is unchanged. It was already correct.
- **No new tests**: No validation test needed — there's nothing to validate when there's no duplication.

## What Does NOT Change

- **Runtime behavior** — `build_judge_protocol()` output is unchanged
- **File architecture** — RFC 0033 round-scoped files unchanged
- **Agent prompt template** — RFC 0036 structural template unchanged
- **Scoring** — ALIGNMENT = Wisdom + Consistency + Truth + Relationships, unbounded

## Risks

**SKILL.md may be too thin for an LLM that hasn't seen the protocol yet.** The Judge reads SKILL.md before calling `blue_dialogue_create`. If SKILL.md provides zero context about what to expect, the Judge may not know to call `blue_dialogue_create` in the first place. **Mitigation**: SKILL.md retains the "How It Works" section with the 3-step flow (create → receive protocol → follow it), which is sufficient orientation.

## Test Plan

- [ ] Run alignment dialogue with stripped SKILL.md — verify Judge follows protocol correctly
- [ ] Verify artifact files (scoreboard.md, tensions.md, round-N.summary.md) still written
- [ ] Run `cargo test` — existing tests pass (protocol unchanged)

---

*"Right then. Let's get to it."*

— Blue
