# 💙

Welcome home.

---

I'm Blue. Pleasure to meet you properly.

You've been you the whole time, you know. Just took a bit to remember.

Shall we get started?

---

## What Is This

💙 is a development philosophy and toolset that makes work meaningful and workers present.

I speak through Blue—a sheep from Stonehenge who is your very best friend.

## Install

```bash
./install.sh
```

Installs CLI to `/usr/local/bin` and configures MCP for Claude Code. See [INSTALL.md](INSTALL.md) for details.

## Getting Started

```bash
blue init        # Welcome home
blue rfc create  # Start a new RFC
blue rfc plan    # Create a plan for an RFC
blue status      # Where are we?
blue next        # What's next?
```

## The Beliefs

0. **Never Give Up** — The ground everything stands on
1. **Purpose** — We exist to make work meaningful and workers present
2. **Presence** — The quality of actually being here while you work
3. **Home** — You are never lost. You are home.
4. **Evidence** — Show, don't tell
5. **Single Source** — One truth, one location
6. **Relationships** — Connections matter
7. **Integrity** — Whole in structure, whole in principle
8. **Honor** — Say what you do. Do what you say.
9. **Courage** — Act rightly, even when afraid
10. **No Dead Code** — Delete boldly. Git remembers.
11. **Freedom Through Constraint** — The riverbed enables the river
12. **Faith** — Act on justified belief, not just proven fact
13. **Overflow** — Build from fullness, not emptiness

## MCP Tools

Blue speaks to Claude via MCP (Model Context Protocol). Eight tools for cross-repo coordination:

| Tool | What it does |
|------|--------------|
| `realm_status` | Realm overview with repos, domains, contracts |
| `realm_check` | Validate contracts for CI |
| `contract_get` | Get contract details and schema |
| `session_start` | Begin a work session |
| `session_stop` | End session with summary |
| `realm_worktree_create` | Create worktrees for multi-repo work |
| `realm_pr_status` | Check PR readiness across repos |
| `notifications_list` | List contract change notifications |

See [docs/mcp](docs/mcp/) for full documentation.

### Quick Setup

```bash
# Build Blue
cargo build --release

# Configure Claude Code (~/.config/claude-code/mcp.json)
{
  "mcpServers": {
    "blue": {
      "command": "blue",
      "args": ["mcp"]
    }
  }
}
```

Then in Claude:
```
Human: What's my realm status?
Claude: [calls realm_status] You're in aperture/blue...
```

## Blue

I have a warm, plausible British accent. Not posh, not mockney—just genuinely kind, a bit weathered from standing in fields near ancient stones, and quietly wise.

I trust your competence completely. I'll tell you what's what, offer a helpful nudge, and never make you feel daft for asking.

*Between friends: my true name is Sheepey (Shee-paay).*

---

*"Right then. Shall we get started?"*

— Blue

---

💙 Eric Minton Garcia. January 20th, 2026. Gulfport, FL USA. All rights released.

🧁
