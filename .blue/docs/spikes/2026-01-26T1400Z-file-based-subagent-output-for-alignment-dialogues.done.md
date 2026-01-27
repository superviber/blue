# Spike: File Based Subagent Output for Alignment Dialogues

| | |
|---|---|
| **Status** | Complete |
| **Outcome** | [RFC 0029](../rfcs/0029-file-based-subagent-output.md) |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

Can alignment dialogue subagents write directly to individual /tmp files instead of returning output through the Task tool, allowing the Judge to read from those files directly? What efficiency gains does this offer over the current JSONL extraction pipeline?

---

## Current Architecture

Each alignment dialogue round follows this pipeline:

```
Judge spawns N agents (run_in_background: true)
  → Each agent executes, output captured in Claude Code JSONL format
  → JSONL written to /tmp/claude/.../tasks/{task_id}.output (symlink)
  → Judge calls blue_extract_dialogue(task_id=...) for EACH agent
  → blue_extract_dialogue:
      1. Walks /tmp/claude/ subdirs to find {task_id}.output
      2. Resolves symlink
      3. Checks if jq is installed (shell-out to `jq --version`)
      4. Parses JSONL: extracts assistant messages → text content blocks
      5. Returns extracted text via MCP response
  → Judge receives text, scores, updates dialogue file
```

**Per-agent overhead in current pipeline:**
1. MCP round-trip for `blue_extract_dialogue` call
2. Directory walk across `/tmp/claude/` subdirs to locate output file
3. Symlink resolution
4. jq availability check (shell spawn for `jq --version`)
5. JSONL parsing — either jq subprocess (`select(.type == "assistant") | ...`) or line-by-line Rust JSON deserialization
6. Text extraction from nested `message.content[].text` JSON structure

For a 5-agent, 3-round dialogue: **15 MCP calls + 15 dir walks + 15 JSONL parses**.

## Proposed Architecture

```
Judge spawns N agents, each with an assigned output file path
  → Each agent writes perspective directly to /tmp/blue-dialogue/{slug}/{agent-name}.md
  → Agent completes
  → Judge reads /tmp/blue-dialogue/{slug}/{agent-name}.md using Read tool
  → Judge has plain text immediately — no extraction needed
```

### What Changes

**1. Agent prompt template gains an output file instruction:**
```
WRITE YOUR OUTPUT: Use the Write tool to write your complete response to:
  {{OUTPUT_FILE}}
This is MANDATORY. Write your full perspective to this file, then stop.
```

**2. alignment-expert.md gains Write tool access:**
```yaml
tools: Read, Grep, Glob, Write
```

**3. Judge protocol updated:**
- Instead of "read each agent's output from the results"
- New: "Read each agent's output file from /tmp/blue-dialogue/{slug}/"
- No more `blue_extract_dialogue` calls

**4. `build_judge_protocol` adds output paths per agent:**
```rust
// In the agent list, add output_file per agent:
json!({
    "name": a.name,
    "output_file": format!("/tmp/blue-dialogue/{}/{}.md", slug, a.name.to_lowercase()),
    ...
})
```

**5. Directory setup:**
- `blue_dialogue_create` creates `/tmp/blue-dialogue/{slug}/` directory
- Or: first agent to write creates it (Write tool creates parent dirs)

### What Doesn't Change

- Subagent type remains `alignment-expert`
- Marker format unchanged ([PERSPECTIVE], [TENSION], etc.)
- Judge scoring logic unchanged
- Dialogue file format unchanged
- `blue_extract_dialogue` preserved for backwards compatibility (still works with task_id/file_path for non-alignment uses)

## Efficiency Analysis

| Step | Current | Proposed | Savings |
|------|---------|----------|---------|
| Output collection | MCP call to `blue_extract_dialogue` | Read tool (already available to Judge) | Eliminates MCP round-trip |
| File location | Dir walk across `/tmp/claude/` | Deterministic path `/tmp/blue-dialogue/{slug}/{name}.md` | No search needed |
| Parsing | JSONL → JSON → extract assistant → extract text | Plain markdown file | Zero parsing |
| jq dependency | Checks `jq --version` per extraction | N/A | Removes external dependency |
| Output format | Nested JSON structure | Raw perspective text | Human-readable on disk |

**For a 5-agent, 3-round dialogue:**
- Current: 15 MCP calls, 15 dir walks, 15 JSONL parses
- Proposed: 15 Read calls (lightweight, no MCP, no parsing)

## Risks & Considerations

### Write tool adds surface area to subagent
Adding Write to alignment-expert means agents can write to arbitrary paths. Mitigated by:
- The prompt explicitly tells them which file to write to
- alignment-expert already has Read/Grep/Glob — Write is the same trust level
- Agents operate with 400-word output limit, so file sizes are bounded

### Agent might not write to file
If an agent returns output via Task result but forgets to write the file, the Judge gets nothing. Mitigated by:
- Making the Write instruction prominent and mandatory in the template
- Judge can fall back to `blue_extract_dialogue(task_id=...)` if file missing
- The agent definition's system prompt can reinforce this

### Cleanup
`/tmp/blue-dialogue/` accumulates files across dialogues. Options:
- OS handles it (macOS clears /tmp on reboot)
- `blue_dialogue_save` or `blue_dialogue_create` cleans up stale dirs
- Not a real concern — each file is ~2KB, dialogues are infrequent

### Round N+1 file collisions
Agent writes round 0 output, then round 1 output to the same file. Solutions:
- Include round number in path: `/tmp/blue-dialogue/{slug}/round-{n}/{agent-name}.md`
- Or: Judge reads file before next round (already does), so overwrite is fine — Judge has already consumed it

## Recommendation

Use round-scoped paths: `/tmp/blue-dialogue/{slug}/round-{n}/{name}.md`

This preserves the full dialogue record on disk (useful for debugging) and eliminates any collision concern. The Judge reads round N files, scores, updates the dialogue document, then spawns round N+1. Clean separation.

## Implementation Sketch

1. **`dialogue.rs` — `build_judge_protocol`**: Add `output_dir` field to protocol, add `output_file` field per-agent entry, include round number template `{{ROUND}}`
2. **`dialogue.rs` — `handle_create`**: Create `/tmp/blue-dialogue/{slug}/` directory
3. **Agent prompt template**: Add `WRITE YOUR OUTPUT` instruction with `{{OUTPUT_FILE}}`
4. **Judge protocol instructions**: Replace "call blue_extract_dialogue" with "Read agent output files"
5. **`.claude/agents/alignment-expert.md`**: Add `Write` to tools list

## Open Questions

- Should the Judge verify file existence before reading, or trust that agents wrote them?
- Should `blue_extract_dialogue` gain a mode to read from the new path convention as a fallback?
- Could this pattern extend beyond alignment dialogues to any multi-agent workflow in Blue?
