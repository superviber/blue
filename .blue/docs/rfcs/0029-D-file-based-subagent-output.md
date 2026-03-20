# RFC 0029: File-Based Subagent Output

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-01-26 |
| **Source Spike** | [file-based-subagent-output-for-alignment-dialogues](../spikes/2026-01-26-file-based-subagent-output-for-alignment-dialogues.md) |
| **Alignment Dialogue** | [file-based-subagent-output-and-dialogue-format-contract-rfc-design](../dialogues/2026-01-26-file-based-subagent-output-and-dialogue-format-contract-rfc-design.dialogue.md) |
| **Depends On** | [RFC 0028](0028-dialogue-format-contract.md) — `DialogueFormat::parse_markers()` |

---

## Summary

Alignment dialogue subagents currently return output through Claude Code's Task system, requiring JSONL extraction via `blue_extract_dialogue` — 6 steps per agent involving MCP round-trips, directory walks, symlink resolution, jq checks, and JSON parsing. This RFC replaces that pipeline with direct file writes: each agent writes its perspective to a round-scoped path in `/tmp`, and the Judge reads those files directly. For a 5-agent, 3-round dialogue, this eliminates 15 MCP calls, 15 directory walks, and 15 JSONL parses.

## Problem

The current extraction pipeline per agent:

1. MCP round-trip for `blue_extract_dialogue` call
2. Directory walk across `/tmp/claude/` subdirs to locate output file
3. Symlink resolution
4. jq availability check (shell spawn for `jq --version`)
5. JSONL parsing — jq subprocess or line-by-line Rust JSON deserialization
6. Text extraction from nested `message.content[].text` JSON structure

For a 5-agent, 3-round dialogue: **15 MCP calls + 15 dir walks + 15 JSONL parses**.

The output is plain text (markdown with alignment markers). The extraction pipeline exists because the Task system captures ALL agent output as JSONL, and we need to extract just the text. If agents write their text directly to a known path, no extraction is needed.

## Design

### Round-Scoped Output Paths

Each agent writes its output to a deterministic path:

```
/tmp/blue-dialogue/{slug}/round-{n}/{name}.md
```

Where:
- `{slug}` — dialogue slug (kebab-case title), unique per dialogue
- `{n}` — round number (0-indexed)
- `{name}` — agent name (lowercase)

Example: `/tmp/blue-dialogue/my-rfc-design/round-0/muffin.md`

Round-scoped paths provide:
- **No collision** between rounds — each round has its own directory
- **Debugging** — full dialogue history preserved on disk
- **Staging area** — Judge validates each round's files before assembling the dialogue document

### Agent Write Protocol

Agents receive an output file path in their prompt:

```
WRITE YOUR OUTPUT: Use the Write tool to write your complete response to:
  /tmp/blue-dialogue/{slug}/round-{n}/{name}.md

This is MANDATORY. Write your full perspective to this file, then stop.
```

The agent prompt also includes the format specification from RFC 0028's `DialogueFormat::specification_markdown()`, so agents know which markers to use and how to format them.

### Task Completion as Read Barrier

Agents run with `run_in_background: true`. The Judge waits for Task completion (via `TaskOutput`) before reading any agent's file. This provides the atomic read barrier:

1. Agent writes complete output to file
2. Agent task completes
3. Judge receives task completion signal
4. Judge reads file — guaranteed complete

No `.lock` files, no `.tmp` renames, no polling needed. The existing Task system provides the completion barrier.

### Judge Read Protocol

After all agents in a round complete, the Judge:

1. Reads each agent's output file using the Read tool
2. Validates content with `DialogueFormat::parse_markers(content)` (from RFC 0028)
3. Scores each agent based on parsed markers and content quality
4. Assembles validated output into the dialogue document

If an agent's file is missing or fails validation, the Judge falls back to `blue_extract_dialogue(task_id=...)` for that agent. This preserves backwards compatibility during migration.

### Integration with RFC 0028

The dependency on RFC 0028 is a single function call:

```rust
let content = std::fs::read_to_string(agent_output_path)?;
let markers = DialogueFormat::parse_markers(&content);
```

RFC 0028's `parse_markers()` handles **fragment parsing** — extracting markers from a single agent's output (as opposed to `parse()` which handles full dialogue documents). This distinction was identified during the alignment dialogue: agent output files are fragments, not documents.

### What Changes

| Component | Change |
|-----------|--------|
| `dialogue.rs` — `build_judge_protocol` | Add `output_dir` field, `output_file` per agent, round number |
| `dialogue.rs` — `handle_create` | Create `/tmp/blue-dialogue/{slug}/` directory |
| Agent prompt template | Add `WRITE YOUR OUTPUT` instruction with path |
| Judge protocol instructions | Replace `blue_extract_dialogue` with Read + `parse_markers()` |
| `alignment-expert.md` | Add `Write` to tools list |

### What Doesn't Change

- Subagent type remains `alignment-expert`
- Marker format unchanged (`[PERSPECTIVE]`, `[TENSION]`, etc.)
- Judge scoring logic unchanged
- Dialogue file format unchanged
- `blue_extract_dialogue` preserved for backwards compatibility

### ADR Alignment

- **ADR 4 (Evidence)**: Round-scoped paths preserve evidence on disk — every agent's output for every round is inspectable.
- **ADR 5 (Single Source)**: Agent writes to one file, Judge reads from that file. No intermediate representation.
- **ADR 10 (No Dead Code)**: After migration, `blue_extract_dialogue` calls for alignment dialogues are removed. The tool itself is preserved for non-alignment uses.

## Phases

### Phase 1: Agent Write Support

- Add `Write` to `alignment-expert.md` tools list
- Update `build_judge_protocol` to include `output_dir` and per-agent `output_file`
- Update agent prompt template with `WRITE YOUR OUTPUT` instruction
- Create `/tmp/blue-dialogue/{slug}/` directory in `handle_create`

### Phase 2: Judge Read Migration

- Update Judge protocol to read agent files instead of calling `blue_extract_dialogue`
- Integrate `DialogueFormat::parse_markers()` (from RFC 0028) for fragment validation
- Add fallback to `blue_extract_dialogue` if file missing

### Phase 3: Cleanup

- Remove fallback path after one release cycle
- Remove `blue_extract_dialogue` calls from alignment dialogue flow
- Preserve `blue_extract_dialogue` for non-alignment uses

## Test Plan

- [ ] Agent writes complete output to specified path
- [ ] Agent output file contains valid markers parseable by `DialogueFormat::parse_markers()`
- [ ] Judge reads agent files after task completion — no partial reads
- [ ] Judge falls back to `blue_extract_dialogue` when file missing
- [ ] Round-scoped paths prevent collision between rounds
- [ ] `/tmp/blue-dialogue/{slug}/` directory created by `handle_create`
- [ ] 5-agent, 2-round dialogue completes with file-based output
- [ ] No `blue_extract_dialogue` calls in alignment dialogue flow after Phase 3

## Open Questions

- Should this pattern extend beyond alignment dialogues to any multi-agent workflow in Blue?
- When agent output exceeds Write tool buffer limits, should the Task system JSONL approach serve as fallback? (Churro T02 from alignment dialogue)

---

*"Ship the contract, then ship the transport."*

— Blue
