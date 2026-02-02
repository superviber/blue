# Spike: Sqlite Alignment Dialogue Assets

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-31 |
| **Time Box** | 1 hour |

---

## Question

Would storing alignment dialogue assets (round outputs, scoreboard, tensions, agent responses) in SQLite be faster than the current file-based approach?

---

## Current Architecture

File-based storage in `/tmp/blue-dialogue/<topic>/`:
```
├── scoreboard.md           (~500 bytes, Judge writes)
├── tensions.md             (~1-2KB, Judge writes, agents read)
├── round-0.summary.md      (~1-2KB, Judge writes, agents read)
├── round-0/
│   ├── muffin.md           (~1.2KB, agent writes)
│   ├── cupcake.md          (~1.2KB, agent writes)
│   └── ... (6-12 agents)
└── round-1/...
```

**Total per round**: ~15-25KB

## I/O Pattern Analysis

| Operation | Who | Concurrency | Size |
|-----------|-----|-------------|------|
| Write agent response | 6-12 agents | Parallel (separate files) | 1-1.5KB each |
| Read all agent files | Judge | Sequential | ~10KB |
| Write scoreboard | Judge | Single | ~500B |
| Write tensions | Judge | Single | ~1-2KB |
| Write summary | Judge | Single | ~1-2KB |
| Read context (next round) | Agents | Parallel | ~5KB each |

## Bottleneck Analysis

| Operation | Time |
|-----------|------|
| LLM inference per agent | **30-60 seconds** |
| File write | ~1-5ms |
| File read | ~1-5ms |
| All file I/O per round | ~50ms total |

**The actual bottleneck is LLM inference, not file I/O.** Even eliminating all file operations would save ~50ms on a 3-5 minute round.

## SQLite Trade-offs

### Potential Pros
- Single file instead of directory tree
- Transactional writes
- Queryable (find all tensions across all dialogues)
- Integration with existing blue-core SQLite db

### Significant Cons
1. **Subagents use Write tool** → can't write to SQLite directly
   - Would need new MCP tools: `blue_dialogue_write_response`, `blue_dialogue_read_context`
   - Significant API surface increase
2. **Parallel writes require careful handling**
   - SQLite has write lock; 6-12 agents writing simultaneously would serialize
   - Would need WAL mode + careful transaction design
3. **Files are trivially debuggable**
   - `cat`, `grep`, `less` just work
   - SQLite requires tooling to inspect
4. **No performance gain**
   - Bottleneck is LLM, not I/O
5. **More complexity for same result**

## The Real Problem

The current issue isn't file I/O speed. It's that subagents weren't reliably writing files because:
1. `alignment-expert` agent type had Write tool listed but wasn't using it
2. Switched to `general-purpose` agents which have full tool access

This was a tool reliability / prompting issue, not a storage architecture issue.

## Conclusion

**Don't do this.** SQLite would add complexity without solving any real problem:

- Performance gain: negligible (~50ms on 3+ minute rounds)
- Debugging: harder (need SQLite tools vs cat/grep)
- Agent integration: would require new MCP tools
- Concurrency: more complex (SQLite write locks)

The file-based approach:
- Works with existing Write tool in Task agents
- Easily debuggable
- Naturally parallelizes (separate files)
- Matches how Claude Code agents already work

## Recommendation

Keep file-based approach. The "fix" was using `general-purpose` subagents, not changing storage.
