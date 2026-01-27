# Spike: Borrowing Ideas from ClaudeBot Hubot Architecture

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

What architectural patterns from ClaudeBot (a Hubot-based IRC bot) could improve Blue's extensibility, plugin system, and developer experience?

---

## Source

[github.com/ClaudeBot/ClaudeBot](https://github.com/ClaudeBot/ClaudeBot) — a general-purpose IRC bot built on GitHub's Hubot framework. CoffeeScript, Redis, Heroku. Deployed on FyreChat IRC.

## Context

ClaudeBot is a classic Hubot-era chatbot. Blue is a philosophy-driven development workflow system built in Rust with MCP integration. They're different beasts — but Hubot's 10+ years of community use have refined certain patterns that are worth examining, particularly around extensibility and developer onboarding.

## What ClaudeBot Does Well

### 1. Declarative Plugin Manifest

ClaudeBot uses `external-scripts.json` — a flat JSON array of npm package names:

```json
["hubot-auth", "hubot-help", "hubot-redis-brain", "hubot-wikipedia", ...]
```

That's the entire plugin configuration. No flags, no options, no nesting. Add a line, install the package, restart. Done.

**Blue parallel**: Blue's emerging plugin architecture (spike: `blue-plugin-architecture`) discusses `.claude-plugin/` directories with skills, hooks, agents, and MCP config. That's richer but also heavier. A simpler "manifest of active extensions" layer could complement the full plugin system — a `.blue/plugins.json` or section in the manifest that just lists what's on.

### 2. Two-Tier Script Discovery

Hubot loads scripts from two places:
- **External**: npm packages declared in `external-scripts.json` (community/shared)
- **Local**: CoffeeScript files dropped into `scripts/` (custom/one-off)

No registration ceremony. Drop a file in `scripts/`, it gets loaded. Install an npm package and add it to the JSON, it gets loaded.

**Blue parallel**: Blue already has `skills/` for Claude Code skills. But MCP handlers are compiled into the Rust binary — there's no "drop a handler in a directory" path. The thin-plugin-fat-binary spike already identified this tension. Hubot's approach suggests: keep the core binary fat, but allow a `scripts/` equivalent for lightweight automation — maybe `.blue/scripts/` that Blue loads as simple shell commands or YAML-defined tool wrappers. Not full MCP handlers, just thin command aliases.

### 3. Self-Describing Commands (hubot-help)

Every Hubot script declares its own help text in a comment header:

```coffeescript
# Commands:
#   hubot translate <text> - Translates text to English
```

`hubot-help` aggregates these automatically. No separate docs to maintain.

**Blue parallel**: Blue has `blue_guide` and the status/next system, but individual MCP tools don't self-describe their purpose in a way that's aggregatable. Each handler module could export a structured description block that `blue_guide` and `blue_next` automatically aggregate — making the help system fully derived rather than manually maintained. This aligns with ADR 0005 (Single Source) — the tool IS the documentation.

### 4. Brain Abstraction

Hubot's "brain" is a dead-simple key-value API:

```coffeescript
robot.brain.set('key', value)
robot.brain.get('key')
```

Redis, in-memory, whatever — the script doesn't know or care. The persistence layer is fully pluggable.

**Blue parallel**: Blue uses SQLite directly via `rusqlite`. The DB is already treated as derived state (filesystem is authoritative per ADR 0005/RFC 0022). But scripts or plugins that want to persist small amounts of data have no simple API. A `blue.brain.set/get` equivalent — a simple key-value namespace per plugin, backed by SQLite but abstracted away — would lower the barrier for plugin authors.

### 5. Adapter Pattern

Hubot abstracts the transport layer:
- IRC adapter
- Slack adapter
- Shell adapter (for testing)

Same bot logic, different communication channels.

**Blue parallel**: Blue speaks MCP over stdio exclusively. If Blue ever needs to support other transports (HTTP API, WebSocket for a dashboard, direct CLI piping), the adapter pattern would cleanly separate "what Blue does" from "how Blue communicates." The daemon already uses Axum for HTTP — formalizing this as a second adapter would be a natural extension.

### 6. Auth Middleware (hubot-auth)

Hubot-auth provides role-based access:

```
hubot: user1 has admin role
hubot: what roles does user2 have?
```

Scripts can gate commands behind roles.

**Blue parallel**: Blue currently trusts all callers equally. As Blue moves toward multi-user realms and plugin distribution, some form of capability scoping becomes relevant. Not full RBAC — that's over-engineering for now — but the concept of "this plugin can read docs but not create RFCs" maps to a capability-based permission model. Worth noting for the plugin architecture RFC.

## What Doesn't Transfer

- **CoffeeScript/Node ecosystem** — Blue is Rust. The npm plugin model doesn't apply directly.
- **Heroku deployment** — Blue is a local binary + daemon. Different model entirely.
- **Redis brain** — SQLite is the right choice for Blue. No need to add a Redis dependency.
- **Regex-based command matching** — Hubot matches chat messages with regex. Blue uses structured MCP tool calls. The structured approach is better.
- **Synchronous script loading** — Blue's async Tokio runtime is more appropriate for its workload.

## Actionable Ideas (Ranked by Feasibility)

### Likely Worth Doing

1. **Self-describing tools** — Each MCP handler exports a structured help block. `blue_guide` aggregates them. Zero manual documentation overhead. Aligns with Single Source.

2. **Plugin manifest simplification** — A `.blue/plugins.yaml` that lists active extensions with minimal config. Complements the richer plugin architecture without replacing it.

3. **Brain API for plugins** — `blue_kv_set` / `blue_kv_get` MCP tools that provide namespaced key-value storage. Simple, pluggable, useful immediately for scripts and plugins.

### Worth Exploring

4. **Local scripts directory** — `.blue/scripts/` for lightweight automation (shell commands, tool aliases) that Blue discovers and exposes as MCP tools. Not full Rust handlers — just thin wrappers.

5. **Transport adapter formalization** — Abstract MCP stdio as one adapter. Name the daemon HTTP as another. Future-proofs for WebSocket/dashboard use cases.

### Not Yet

6. **Capability-scoped plugins** — Wait until plugin distribution is real. Note the pattern for future RFC.

## Outcome

ClaudeBot's value isn't in its code (it's a dated Hubot bot). Its value is in the **developer experience patterns** that survived a decade of community use: declarative configuration, drop-in extensibility, self-describing tools, and simple persistence abstractions. Blue's current plugin architecture work should consider these patterns as design constraints — particularly self-describing tools and the brain API, which are small lifts with immediate payoff.
