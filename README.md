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
cargo build --release
./target/release/blue install
```

Configures hooks, skills, and MCP for Claude Code. Restart Claude Code after installation.

See [INSTALL.md](INSTALL.md) for details.

## Getting Started

```bash
blue init       # Welcome home
blue create     # Start a new idea
blue plan       # Break it into steps
blue status     # Where are we?
blue next       # What's next?
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

## Playwright MCP with Chrome Profiles

By default, Playwright MCP launches a fresh browser with no saved sessions. To use your existing Chrome profile (with saved logins), copy the profile and point Playwright at the copy.

**Setup:**

1. Copy your Chrome profile to a temp location (avoids profile lock conflicts):
   ```bash
   mkdir -p /tmp/chrome-profile
   cp -r ~/Library/Application\ Support/Google/Chrome/Default /tmp/chrome-profile/Default
   cp ~/Library/Application\ Support/Google/Chrome/Local\ State /tmp/chrome-profile/
   ```

2. Update `.claude/plugins/.../playwright/.mcp.json`:
   ```json
   {
     "playwright": {
       "command": "npx",
       "args": [
         "@playwright/mcp@latest",
         "--executable-path", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
         "--user-data-dir", "/tmp/chrome-profile"
       ]
     }
   }
   ```

3. Restart Claude Code.

**Limitations:**
- Saved passwords won't autofill (Keychain encryption is path-bound). Usernames fill fine.
- Copy your passwords from `chrome://password-manager/passwords` in your regular Chrome as needed.
- Sessions stay active within the Playwright instance, so you only log in once per site.
- Re-copy the profile if you need fresh cookies/sessions.

**What didn't work:**
- `--remote-debugging-port=9222`: Chrome on macOS silently ignores TCP port binding.
- `--extension` mode with Playwright MCP Bridge: Opens a new profile instead of connecting to the existing one.
- Direct `--user-data-dir` pointing to the live Chrome profile: Lock file conflicts.

### Quick Setup

```bash
cargo build --release
./target/release/blue install
# Restart Claude Code
```

Then in Claude:
```
Human: blue status
Claude: [calls blue_status] Project: blue, Branch: develop...
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
