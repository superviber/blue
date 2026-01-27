# Spike: Rfc Stub Source Link And Kebab Case Filename Bugs

| | |
|---|---|
| **Status** | In Progress |
| **Date** | 2026-01-26 |
| **Time Box** | 30 minutes |

---

## Question

Why does blue_rfc_create not link the Source Spike field, and why are some spike/RFC filenames created with spaces instead of kebab-case?

---

## Findings

### Bug 1: Source Spike field rendered as plain text

**Root cause:** `Rfc::to_markdown()` in `crates/blue-core/src/documents.rs:227-228`

```rust
if let Some(ref spike) = self.source_spike {
    md.push_str(&format!("| **Source Spike** | {} |\n", spike));
}
```

The spike title is stored as a bare `String` and rendered directly into the markdown table. No link is constructed.

**Contributing factor:** `handle_rfc_create` in `crates/blue-mcp/src/server.rs:2522-2523` stores the raw title without resolving the spike's file path:

```rust
if let Some(s) = source_spike {
    rfc.source_spike = Some(s.to_string());
}
```

The `Rfc` struct has no access to the spike's file path or the document store at render time. The same issue exists for `source_prd` at `documents.rs:230-231`.

**Fix options:**
1. Resolve the spike file path at creation time (in `handle_rfc_create`) and store a markdown link string in `source_spike` — e.g. `[Title](../spikes/2026-01-26-slug.md)`
2. Change `source_spike` from `Option<String>` to a struct carrying both title and path, then render the link in `to_markdown()`

Option 1 is simpler. The spike's `file_path` can be looked up from the store via `find_document(DocType::Spike, title)`.

### Bug 2: Filenames created without kebab-case

**Root cause:** `handle_rfc_create` in `crates/blue-mcp/src/server.rs:2529`

```rust
let filename = format!("rfcs/{:04}-{}.md", number, title);
```

The raw `title` is interpolated directly — no `to_kebab_case()` call. If the title contains spaces or mixed case, the filename will too. There is no `to_kebab_case` function anywhere in `server.rs`.

**Spike handler is correct.** `crates/blue-mcp/src/handlers/spike.rs:34` does call `to_kebab_case(title)`:

```rust
let filename = format!("spikes/{}-{}.md", date, to_kebab_case(title));
```

The existing space-named spike files (e.g. `2026-01-25-Background Agents and Dialogue Creation Not Triggering.md`) were created either before commit `015c21d` applied the kebab-case fix to the spike handler, or by a Claude agent writing files directly with the Write tool (bypassing the MCP handler entirely).

**Systemic issue:** `to_kebab_case()` is duplicated as a private function in 7 handler files (`spike.rs`, `adr.rs`, `decision.rs`, `prd.rs`, `postmortem.rs`, `dialogue.rs`, `runbook.rs`) but is missing from `server.rs` where `handle_rfc_create` lives. RFC creation is the only handler that still generates filenames inline in `server.rs` rather than in a dedicated handler module.

### Summary

| Bug | Location | Cause |
|-----|----------|-------|
| Source Spike not linked | `documents.rs:227` | Renders title as plain text, never constructs a link |
| RFC filenames not kebab-cased | `server.rs:2529` | Uses raw `title` in filename, no `to_kebab_case()` |
| `to_kebab_case` fragility | 7 private copies across handler files | Not shared; `server.rs` has no copy at all |

## Recommendation

Recommends implementation. Both bugs are straightforward fixes:

1. In `handle_rfc_create`: look up the spike's `file_path` from the store, store a markdown link in `source_spike` (same for `source_prd`)
2. In `handle_rfc_create`: apply `to_kebab_case()` to the title when generating the filename
3. Extract `to_kebab_case` into `blue-core` as a shared public utility to prevent future drift
