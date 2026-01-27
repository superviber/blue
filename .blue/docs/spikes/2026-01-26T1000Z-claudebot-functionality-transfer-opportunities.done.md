# Spike: ClaudeBot Functionality Transfer Opportunities

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-26 |
| **Time Box** | 1 hour |

---

## Question

What user-facing functionality from ClaudeBot could add value to Blue's developer workflow?

---

## Source

[github.com/ClaudeBot/ClaudeBot](https://github.com/ClaudeBot/ClaudeBot) — Hubot IRC bot with 24 external plugins and 5 custom scripts. The [ClaudeBot org](https://github.com/ClaudeBot) maintains 30 repos including custom Hubot plugins.

## ClaudeBot's Full Feature Map

| Feature | Source | What It Does |
|---------|--------|-------------|
| Reminders | [hubot-remind-advanced](https://github.com/ClaudeBot/hubot-remind-advanced) | Natural language reminders with conversational flow |
| Link tracking | [hubot-links](https://github.com/ClaudeBot/hubot-links) | Auto-captures shared URLs, recall recent links |
| Search | [hubot-search](https://github.com/ClaudeBot/hubot-search) | Google/Bing search from chat |
| Stack Overflow | custom script | Search SO questions inline |
| CDN lookup | custom script | Find library CDN URLs |
| YouTube | hubot-youtube + youtube-info + youtube-tracker | Video search, metadata, URL detection |
| Google Images | hubot-google-images | Image search |
| Translation | hubot-google-translate | Translate text between languages |
| Wikipedia | hubot-wikipedia | Wiki article lookup |
| Twitch/Steam | hubot-twitch, hubot-steam-webapi | Streaming and game info |
| URL shortening | hubot-googl | Shorten URLs |
| Pastebin | hubot-paste | Paste long text externally |
| Long text handling | [hubot-longtext](https://github.com/ClaudeBot/hubot-longtext) | Auto-paste long responses, return link |
| Man pages | hubot-manpages | Unix manual lookups |
| Email | hubot-mail | Send email from chat |
| Auth/roles | hubot-auth | Role-based command access |
| Diagnostics | hubot-diagnostics | Self-inspection and health |
| Help | hubot-help | Auto-aggregated command help |
| Brain persistence | hubot-redis-brain | Key-value persistence across sessions |
| Web archiving | hubot-archive-today | Archive web pages |
| Admin tools | custom script | Brain wipe, uptime, restricted commands |
| ASCII faces | custom script | Random ASCII art (fun) |

## What Blue Already Covers

- **Reminders**: `blue_reminder_create/list/snooze/clear` — already implemented
- **Search**: `blue_index_search` — semantic index with FTS5
- **Diagnostics/Health**: `blue_health_check` — already implemented
- **Help**: `blue_guide` + status/next system — already implemented
- **Persistence**: SQLite — already implemented
- **Auth concepts**: Not yet, but noted in architecture spike

## Functionality Worth Transferring

### 1. Session Resource Tracking

**ClaudeBot analog**: hubot-links (auto-captures URLs shared in chat)

**Blue version**: During a dev session (`blue_session_start` → `blue_session_stop`), auto-capture external resources referenced — URLs, file paths, Stack Overflow links, GitHub issues, documentation pages. At session end, generate a "resources used" summary attached to the session record.

**Why it matters**: Developers reference dozens of resources during a session. These references evaporate when the session ends. Capturing them creates a reusable knowledge trail — especially valuable when picking up work days later or handing off to another developer.

**Implementation surface**: Extend session tracking in `blue-core`. Add a `blue_session_bookmark` MCP tool for explicit captures. Consider passive capture via hook on Claude Code's WebFetch/WebSearch calls.

### 2. Cross-Session Bookmarks

**ClaudeBot analog**: hubot-links list/clear — persistent link storage with recall

**Blue version**: `blue_bookmark_add <url> [tags]` / `blue_bookmark_list [tag|rfc|spike]` / `blue_bookmark_search <query>`. Bookmarks are tagged and associated with Blue documents (RFCs, spikes). When you start working on RFC 0016, Blue surfaces bookmarks tagged to it.

**Why it matters**: Blue already has a semantic index for code files. But external resources (docs, articles, SO answers, design references) aren't tracked. Bookmarks bridge that gap — they're lightweight, explicit, and immediately useful.

**Implementation surface**: New document type or key-value entries in SQLite. 2-3 new MCP handlers. Tags link to existing Blue documents via relationships (ADR 0006).

### 3. Smart Output Management

**ClaudeBot analog**: hubot-longtext (auto-detects long responses, pastes externally, returns link)

**Blue version**: When Blue outputs a large document (RFC, dialogue, audit), detect the length and offer tiered delivery:
- **Summary mode**: Key points only (~10 lines)
- **Section mode**: Table of contents with expandable sections
- **Full mode**: Everything

Could also generate a temporary local file and return the path, keeping the Claude Code context clean.

> **Note**: The temp-file-and-return-path approach is the preferred direction here. Write the full document to a local file, return a short summary + the file path in the MCP response. The developer clicks the path to read more. Context stays lean, content stays accessible. Start with this before building tiered delivery modes.

**Why it matters**: Blue documents can be 100-300 lines. Dumping them into Claude Code's context consumes tokens and overwhelms the conversation. Smart truncation keeps the developer in flow while preserving access to the full content.

**Implementation surface**: Response middleware in `blue-mcp` handlers. Each document type defines its own summary format. File output via temp directory.

### 4. External Knowledge Lookup

**ClaudeBot analog**: Stack Overflow script, hubot-search, hubot-wikipedia, hubot-manpages

**Blue version**: `blue_lookup <query> [--source so|docs|crates|npm]` — search external knowledge sources from within Blue. Results get cached in the semantic index with a "lookup" relationship type, so repeated questions for the same topic hit local cache.

**Why it matters**: Claude Code already has WebSearch. The difference is that Blue would **remember** the lookups — linking them to the current RFC/spike, indexing the results, and surfacing them when you return to that work context. The lookup becomes part of the project's knowledge, not ephemeral chat history.

**Implementation surface**: Reqwest calls to public APIs (Stack Exchange, crates.io, docs.rs). Results stored in semantic index. Relationship links to active Blue documents. 1-2 MCP handlers.

### 5. Outbound Notifications

**ClaudeBot analog**: hubot-mail (send email from chat)

**Blue version**: When important state transitions happen, notify through external channels:
- RFC status changes (draft → dialogue → final) → webhook/email
- Spike time-box expires → desktop notification
- Dialogue converges → notify all participants
- Contract schema change in realm → notify dependent repos
- Audit findings → email summary

`blue_notify_configure` to set up channels (webhook URL, email, desktop). Events fire automatically on state transitions.

**Why it matters**: Blue currently only surfaces information when you ask for it (`blue_status`, `blue_next`). Developers context-switch. A spike time-box expiring while you're deep in another task goes unnoticed. Push notifications close the feedback loop.

**Implementation surface**: Notification dispatch in `blue-core` daemon. Hook into existing state transition logic. Webhook sender via reqwest. Desktop notifications via `notify-rust` crate. Configuration stored in `.blue/config.yaml`.

### 6. Document Format Transformation

**ClaudeBot analog**: hubot-archive-today (capture/transform web content), hubot-paste (reformat for external consumption)

**Blue version**: Transform between Blue document types and export to external formats:
- `blue_transform spike → rfc` — extract spike findings into an RFC problem statement
- `blue_transform rfc → github-issue` — export RFC as a GitHub issue
- `blue_transform dialogue → summary` — condense dialogue rounds into key decisions
- `blue_transform audit → checklist` — convert audit findings to actionable checklist

**Why it matters**: Blue documents follow structured formats. Transformations between them are mechanical but tedious. Automating the common paths (spike → RFC is the most frequent) saves real time and ensures nothing gets lost in translation.

**Implementation surface**: Template-based transformations in `blue-core`. LLM-assisted for summarization transforms. GitHub export via existing forge integration. 1 MCP handler with subcommands.

### 7. Natural Language Time Expressions

**ClaudeBot analog**: hubot-remind-advanced uses Chrono for "in 1 hour", "tomorrow at 3pm", "next Monday"

**Blue version**: Blue's existing reminders could accept natural language time: `blue_reminder_create "follow up on RFC 0016" "after lunch"` or `"in 2 hours"`. Also applicable to spike time-boxes: `blue_spike_create ... --time-box "until end of day"`.

**Why it matters**: Small ergonomic win. Current reminders likely expect structured time formats. Natural language is faster and more human.

**Implementation surface**: Chrono-english or similar Rust crate for natural language date parsing. Thin wrapper around existing time handling.

## What Doesn't Transfer

- **Image/video search** (YouTube, Google Images) — not relevant to dev workflow
- **Gaming integrations** (Steam, Twitch) — entertainment, not development
- **URL shortening** — no need in a CLI context
- **ASCII art** — fun but not Blue's voice
- **Translation** — Claude Code handles this natively

## Prioritized Recommendations

### High Value, Low Effort
1. **Smart output management** — keeps context clean, improves every interaction
2. **Natural language time expressions** — small crate addition, immediate ergonomic win

### High Value, Medium Effort
3. **Session resource tracking** — passive capture during sessions, knowledge retention
4. **Cross-session bookmarks** — explicit capture, document-linked, immediately useful
5. **Document format transformation** — spike → RFC path alone justifies this

### High Value, Higher Effort
6. **External knowledge lookup with caching** — powerful but requires API integrations
7. **Outbound notifications** — push model, requires daemon + channel configuration

## Outcome

ClaudeBot's feature set is mostly API aggregation for an IRC chat context. The transferable ideas aren't the specific integrations (YouTube, Steam) but the **workflow patterns** they represent:

- **Passive capture** (links → session resource tracking)
- **Smart output** (longtext → tiered document delivery)
- **Persistent recall** (links list → cross-session bookmarks)
- **Push notifications** (mail → outbound state change alerts)
- **Format bridging** (archive/paste → document transformation)
- **Natural interfaces** (remind → natural language time)

The top three for Blue: smart output management, session resource tracking, and document transformation. These address real friction points in Blue's current workflow.
