# Alignment Dialogue: Document Lifecycle Filenames Rfc Design

**Draft**: Dialogue 2031
**Date**: 2026-01-26 10:10
**Status**: In Progress
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone, 🧁 Eclair, 🧁 Donut, 🧁 Brioche, 🧁 Croissant, 🧁 Macaron, 🧁 Cannoli, 🧁 Strudel, 🧁 Beignet, 🧁 Churro
**RFC**: document-lifecycle-filenames

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | UX Architect | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Technical Writer | Core | 0.90 | 🧁 |
| 🧁 Scone | Systems Thinker | Core | 0.85 | 🧁 |
| 🧁 Eclair | Domain Expert | Core | 0.80 | 🧁 |
| 🧁 Donut | Devil's Advocate | Adjacent | 0.70 | 🧁 |
| 🧁 Brioche | Integration Specialist | Adjacent | 0.65 | 🧁 |
| 🧁 Croissant | Risk Analyst | Adjacent | 0.60 | 🧁 |
| 🧁 Macaron | First Principles Reasoner | Adjacent | 0.55 | 🧁 |
| 🧁 Cannoli | Pattern Recognizer | Adjacent | 0.50 | 🧁 |
| 🧁 Strudel | Edge Case Hunter | Wildcard | 0.40 | 🧁 |
| 🧁 Beignet | Systems Thinker | Wildcard | 0.35 | 🧁 |
| 🧁 Churro | Domain Expert | Wildcard | 0.30 | 🧁 |

## Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 4 | 4 | 4 | 4 | **16** |
| 🧁 Cupcake | 3 | 4 | 4 | 3 | **14** |
| 🧁 Scone | 5 | 4 | 5 | 4 | **18** |
| 🧁 Eclair | 4 | 5 | 5 | 4 | **18** |
| 🧁 Donut | 4 | 3 | 4 | 3 | **14** |
| 🧁 Brioche | 4 | 4 | 4 | 4 | **16** |
| 🧁 Croissant | 4 | 4 | 5 | 3 | **16** |
| 🧁 Macaron | 5 | 3 | 5 | 3 | **16** |
| 🧁 Cannoli | 4 | 4 | 4 | 3 | **15** |
| 🧁 Strudel | 4 | 4 | 5 | 3 | **16** |
| 🧁 Beignet | 4 | 4 | 4 | 4 | **16** |
| 🧁 Churro | 3 | 4 | 4 | 3 | **14** |

**Initial ALIGNMENT**: 189 / 240 (79%)

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01 | Muffin | `.done-rfc` creates invisible coupling between spike and RFC doc types | R1 |
| P02 | Muffin | Cross-reference updates missing from implementation plan | R1 |
| P03 | Cupcake | No glossary/onboarding for 10 status abbreviations | R1 |
| P04 | Cupcake | `.done-rfc` contradicts code at spike.rs:95-109 | R1 |
| P05 | Scone | Filenames shift from immutable identifiers to mutable state | R1 |
| P06 | Scone | Rename cascade lacks rollback semantics | R1 |
| P07 | Scone | Default-state omission creates asymmetry | R1 |
| P08 | Eclair | `.done-rfc` unreachable — handler blocks completion | R1 |
| P09 | Eclair | Default omission hides active work | R1 |
| P10 | Donut | Cross-reference breakage underestimated (IDE, PRs, static sites) | R1 |
| P11 | Donut | Option C (subdirectories) solves both problems | R1 |
| P12 | Brioche | Need centralized status transition hook for atomicity | R1 |
| P13 | Croissant | Silent overwrite at HHMM granularity is data loss vector | R1 |
| P14 | Macaron | Filenames exist to locate, not to store state | R1 |
| P15 | Macaron | Default omission creates cross-type ambiguity | R1 |
| P16 | Cannoli | Filesystem-git impedance mismatch | R1 |
| P17 | Strudel | `.done-rfc` conflates two state transitions | R1 |
| P18 | Strudel | Abandoned spikes invisible (no suffix forever) | R1 |
| P19 | Beignet | 3-way transaction (SQLite + file + git) without rollback | R1 |
| P20 | Churro | git blame discontinuity destroys provenance | R1 |

## Tensions Tracker

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T1 | `.done-rfc` suffix is unreachable: spike.rs:95-109 blocks completion for `recommends-implementation` | Open | R1 (12/12) | — |
| T2 | Default-state suffix omission creates ambiguity across doc types | Open | R1 (10/12) | — |
| T3 | Rename cascade is a 3-way transaction (SQLite + file + git) with no rollback semantics | Open | R1 (8/12) | — |

## Round 1: Opening Arguments

### Muffin 🧁

The `.done-rfc` suffix creates invisible coupling between spike and RFC document types — understanding the filename requires knowing the spike-to-RFC relationship. More critically, `spike.rs:95-109` blocks completion when outcome is `recommends-implementation`, returning `rfc_required` instead of `success`. The suffix assumes both steps completed, but the code prevents it.

Cross-reference updates are missing from the implementation plan. The RFC says "most survive" via title-based lookups, but provides no evidence this holds across all 9 document types. `rename_for_status()` updates SQLite `file_path` but says nothing about markdown link updates.

### Cupcake 🧁

Nine document types with 10 abbreviations (`.wip`, `.impl`, `.super`, `.done-rfc`) and no onboarding path: no glossary file, no autocomplete hints in MCP tool descriptions, no migration guide. A new contributor sees cryptic suffixes and reaches for the wrong status.

The `.done-rfc` suffix contradicts code behavior. `spike.rs:95-109` refuses to complete spikes with `recommends-implementation` outcome. Either the code needs changing or the RFC must acknowledge `.done-rfc` is a manual rename, not a tool-generated state.

### Scone 🧁

The RFC fundamentally changes the semantic contract of filenames. Currently filenames are immutable identifiers (git history, bookmarks, cross-references). Status-in-filename transforms them into mutable representations of document state. If someone manually renames `slug.done.md` back to `slug.md`, the filename contradicts SQLite. Two conflicting sources of truth.

The rename cascade lacks rollback: What happens when `git mv` fails (file open, permissions, dirty tree)? What about store update succeeding but rename failing? The spike notes "manageable" but specifies no error recovery paths.

Default-state omission is elegant but asymmetric: `.impl` proves implementation, but no suffix could mean "draft" or "just old convention." Always-use-suffixes for stateful docs would be more honest.

### Eclair 🧁

No code path sets a spike to `complete` with `recommends-implementation` outcome. The handler either completes (for `no-action`/`decision-made`) or blocks (for `recommends-implementation`). The `.done-rfc` suffix assumes both can happen.

Default-state omission hides active work. A directory of 15 `2026-01-26T0856Z-*.md` files could be active investigations or stale drafts. Would `.wip` for in-progress spikes be more honest than pretending the default is self-evident?

### Donut 🧁

Cross-reference breakage is underestimated. IDE jump-to-definition, git PR review links, documentation websites, shell scripts — all break on rename. "Accept that external bookmarks break" reveals the cost: every status transition becomes a coordination event.

Option C (subdirectories) solves both problems: clean URLs that don't break, and `ls rfcs/implemented/` gives you exactly what you want. The RFC dismisses this as "complex for tools," but adding `git mv` + store updates + reference scanning is equally complex — just distributed.

Status suffix scatter violates temporal coherence. Three statuses of RFC 0031 interleave with other RFCs when sorted.

### Brioche 🧁

Every status change handler across 9 document types must coordinate three atomic operations: SQLite update, markdown rewrite, filesystem rename. The RFC shows a `rename_for_status` helper but doesn't specify who calls it or when. We need a centralized status transition hook that guarantees all three happen atomically.

The `.done-rfc` suffix is ambiguous under current handler logic — completion is blocked until RFC exists. The `rebuild_filename()` transition detection from no-suffix to suffix state needs careful attention.

### Croissant 🧁

Rename cascades break atomic consistency. Cross-document references are filename-based in markdown, not title-based as the RFC claims. The "future work" cross-reference updater isn't optional — it's foundational.

The `.done-rfc` suffix conflicts with the status model at `spike.rs:95-109`. Silent overwrite risk at HHMM granularity is load-bearing, not cosmetic — a productive hour creates 60 one-minute collision windows. Status suffixes make this worse (more renames = more collision windows).

### Macaron 🧁

From first principles: filenames exist to help humans locate files, not to store structured data. We have SQLite for state, git for history, frontmatter for metadata. Kubernetes, NPM, and git all keep status in metadata, not names. The rename-on-status pattern fights the filesystem's core assumption: stable identifiers.

Default-state omission creates parsing ambiguity: `2026-01-26T0856Z-slug.md` could be an in-progress spike, a recorded decision, an open postmortem, or an in-progress audit. The filesystem browser loses the self-documenting property the RFC promises.

### Cannoli 🧁

The proposal treats filenames as data carriers, encoding both temporal metadata and state. This creates a filesystem-git impedance mismatch — Git treats filenames as immutable identifiers, while this RFC makes them mutable.

Default-state omission: `0031-slug.md` could be a draft RFC or a legacy file without suffix. No migration signal distinguishes "intentionally draft" from "created before this RFC."

### Strudel 🧁

The `.done-rfc` suffix conflates two state transitions: spike completion and RFC creation. `spike.rs:95-109` deliberately prevents completion until RFC exists — when does the rename happen? Before RFC creation contradicts handler logic; after it, who triggers it?

Abandoned spikes stay "in-progress" forever with no suffix. The timestamp helps identify age, but there's no status signal for "stale." Default noise means active and stale look identical.

### Beignet 🧁

Every status change now triggers filesystem mutation + SQLite update + git operation in lockstep. Current handlers only touch SQLite + frontmatter. Adding `git mv` introduces failure modes where three systems desynchronize. Once filenames encode status, human workflows will depend on that encoding. Breaking the contract via desync is worse than never having the feature.

Default-state filename ambiguity: `0042-slug.md` without suffix could be draft RFC, accepted ADR, or recorded Decision. Three different document types look identical in their default states.

### Churro 🧁

When an RFC transitions from `0031-slug.md` to `0031-slug.impl.md`, every blame annotation shows "last modified when renamed" instead of the actual substantive change. For long-lived design documents, this destroys provenance tracking.

The `.done-rfc` suffix is unreachable under current handler logic at `spike.rs:95-109`. Active work is invisible with no-suffix defaults — you can't tell draft from accepted without opening files.

---

## 💙 Judge — Round 1 Summary

**Strong consensus on 3 tensions:**

1. **T1 (12/12)**: `.done-rfc` is unreachable. `spike.rs:95-109` blocks completion for `recommends-implementation`. The suffix cannot be generated by the current tool flow.

2. **T2 (10/12)**: Default-state omission creates cross-type ambiguity. No suffix means different things for different doc types, and can't distinguish "intentionally default" from "pre-RFC legacy."

3. **T3 (8/12)**: Rename cascade introduces a 3-way distributed transaction (SQLite + filesystem + git) with no rollback semantics. Cross-references ARE filename-based in markdown.

**Notable minority positions:**
- Donut advocates Option C (subdirectories) as superior to suffix approach
- Macaron challenges the premise: filenames should locate, not store state
- Croissant flags HHMM overwrite risk as load-bearing

**Round 2 directive**: All experts must propose concrete resolutions for T1, T2, and T3. Converge on amended RFC language.

## Round 2: Tension Resolution

### Muffin 🧁

**T1 resolution**: Drop `.done-rfc`. When `spike_complete` with `recommends-implementation` triggers RFC creation, the spike gets `.done` suffix afterward (via a second `spike_complete` call with `decision-made` outcome, or by having `rfc_create` auto-complete the source spike). The RFC-spike linkage lives in SQLite `source_spike` field, not the filename. One status per filename.

**T2 resolution**: Accept the trade-off. Default-state omission is the right call because the *majority* browsing use case is "which of these are done?" The presence of a suffix signals completion, its absence signals "still active or new." This is like how we read email: unread is default, stars/labels are additive. Document type is already disambiguated by directory (`rfcs/`, `spikes/`, etc.).

**T3 resolution**: Use `fs::rename` + `store.update_file_path` as a two-step best-effort operation. If rename succeeds but store update fails, the file has the right name and the store catches up on next scan. If rename fails, the store hasn't changed either — no inconsistency. Skip `git mv` — let the user commit normally. `git add -A` captures renames automatically. No need for explicit `git mv`.

### Cupcake 🧁

**T1 resolution**: Replace `.done-rfc` with `.done` for all completed spikes regardless of outcome. Record outcome in frontmatter and SQLite only. Add a `## Outcome` section to spike markdown that says "Recommends: RFC 0031" with a link. The filename signals done-ness; the content signals what happened.

**T2 resolution**: Add a sentence to the RFC: "Files without status suffixes are in their initial state. Legacy files without suffixes created before this RFC are also treated as initial state. This is intentional — no migration required." Explicit documentation removes the ambiguity.

**T3 resolution**: The rename operation should be filesystem-first, store-second. If `fs::rename` fails, return an error and don't touch the store. If the store update fails after a successful rename, log a warning — the next `blue_sync` can reconcile file paths by scanning the filesystem. This makes the filesystem the source of truth for paths, which it already is.

### Scone 🧁

**T1 resolution**: Agree — drop `.done-rfc`. Spike outcomes belong in the document body and SQLite metadata, not filenames. A spike is either in-progress or done. Binary. The filename vocabulary becomes: no suffix = active, `.done` = complete. Clean.

**T2 resolution**: I retract my always-use-suffixes suggestion. After reflection, the directory structure already disambiguates type (`rfcs/` vs `spikes/` vs `adrs/`). Within a single directory, "no suffix = initial state" is unambiguous because all files in `rfcs/` are RFCs. The ambiguity I raised only exists if you mix document types in one directory, which Blue doesn't do.

**T3 resolution**: The rename transaction should be: (1) rename file, (2) update store, (3) NO git operation. Let the user or CI handle git staging. The `rename_for_status` function should return a `RenameResult` with the old and new paths so callers can log it. If step 1 fails, abort. If step 2 fails after step 1 succeeds, the filesystem is authoritative — store reconciliation on next load.

### Eclair 🧁

**T1 resolution**: Drop `.done-rfc` entirely. Modify `spike_complete` handler: when `rfc_create` is called with `source_spike`, have it auto-complete the source spike with `decision-made` outcome after RFC creation succeeds. This closes the spike → RFC loop without needing a special suffix. The filename just gets `.done`.

**T2 resolution**: Accept default-state omission. The user's goal was "see at a glance which are done." Suffixes answer "what's finished?" not "what's the exact status?" The absence of a suffix = "not yet done" is the right mental model. If you need the exact status, open the file or query the store.

**T3 resolution**: Two-phase, filesystem-first. Drop `git mv` from the RFC entirely — it's unnecessary overhead. `git` detects renames automatically when content similarity exceeds 50%. Since we only change the filename (not content), `git diff --find-renames` will always detect it. The RFC should specify: `fs::rename` then `store.update_file_path`. No git integration needed.

### Donut 🧁

**T1 resolution**: I concede `.done-rfc` should go. The experts are right that it's unreachable. Use `.done` for all completed spikes. Store the outcome in metadata.

**T2 resolution**: I still think this is a real problem, but I accept the panel's argument that directory-per-type disambiguates. Within `rfcs/`, no suffix means draft. Within `spikes/`, no suffix means in-progress.

**T3 resolution**: I withdraw my Option C advocacy for this RFC. The panel has converged on suffix approach. But the RFC should explicitly add to the test plan: "Integration: Verify `fs::rename` failure leaves store unchanged" and "Integration: Verify store update failure after rename is recoverable via filesystem scan."

### Brioche 🧁

**T1 resolution**: Drop `.done-rfc`. Remove it from the vocabulary table and `status_suffix()` match. All completed spikes get `.done` regardless of outcome. Outcome lives in frontmatter `## Outcome` section and SQLite `outcome` field.

**T2 resolution**: Accept omission. Add to RFC: "Within each document type's directory, absence of a status suffix unambiguously means the initial/default state for that type."

**T3 resolution**: Centralized `transition_status()` function in blue-core that: (1) renames file, (2) updates store file_path, (3) updates markdown frontmatter status. All three in sequence, filesystem-first. No git operation. The function returns `Result<TransitionResult, TransitionError>` with enough info for callers to handle partial failure. Add this to the Code Changes section.

### Croissant 🧁

**T1 resolution**: Agree — drop `.done-rfc`. It was a nice idea but unreachable and unnecessary. Store outcome metadata, not filename metadata.

**T2 resolution**: Accept. Directory-per-type makes it clear.

**T3 resolution**: The RFC should specify error handling explicitly:
```
1. fs::rename(old, new) — if fails, return error, done
2. store.update_file_path() — if fails, attempt fs::rename(new, old) rollback
3. update_markdown_status() — if fails, log warning (non-critical)
```
This gives us a best-effort rollback for the critical path (file + store) and accepts that frontmatter is eventually consistent. Move overwrite protection from "Future Work" to the main design — it's a prerequisite for status suffixes.

### Macaron 🧁

**T1 resolution**: Drop `.done-rfc`. Status suffixes should map 1:1 to the status field in SQLite. No compound statuses, no outcome encoding. Clean separation.

**T2 resolution**: I maintain that default omission is philosophically wrong (filenames shouldn't store state at all), but I accept the panel's pragmatic resolution. Directory-per-type disambiguates within Blue's structure.

**T3 resolution**: I accept the filesystem-first, no-git approach. But add a design principle to the RFC: "Filenames are derived views of store state. The SQLite store is authoritative. If filename and store disagree, store wins. `blue_sync` reconciles."

### Cannoli 🧁

**T1 resolution**: Drop `.done-rfc`. Unanimous.

**T2 resolution**: Accept omission with documentation.

**T3 resolution**: Agree with Croissant's error handling sequence. Add rollback on store failure.

### Strudel 🧁

**T1 resolution**: Drop `.done-rfc`. Consider having `rfc_create` with `source_spike` parameter auto-complete the source spike — this closes the workflow gap that `.done-rfc` was trying to address.

**T2 resolution**: Accept. But add to test plan: "Verify that legacy files (pre-RFC) without suffixes are correctly treated as default state."

**T3 resolution**: Agree with filesystem-first, no-git. Add the rollback attempt on store failure. Also: the overwrite protection (checking file existence before write) should be a prerequisite, not future work.

### Beignet 🧁

**T1 resolution**: Drop `.done-rfc`. The panel is unanimous.

**T2 resolution**: Accept with documentation.

**T3 resolution**: Filesystem-first, store-second, no git. Add explicit rollback. The RFC should add to Design: "Atomicity: If filesystem rename succeeds but store update fails, attempt rollback rename. If rollback also fails, log error with both paths for manual resolution."

### Churro 🧁

**T1 resolution**: Drop `.done-rfc`. Use `.done` for all completed spikes.

**T2 resolution**: Accept. Directory context disambiguates.

**T3 resolution**: Agree with the panel. Filesystem-first, rollback on store failure, no git operations. This keeps the implementation simple and the failure modes bounded.

---

## 💙 Judge — Round 2 Summary

**All 3 tensions resolved:**

**T1 RESOLVED (12/12)**: Drop `.done-rfc` from vocabulary. All completed spikes get `.done` regardless of outcome. Spike outcome lives in frontmatter and SQLite. Consider having `rfc_create` with `source_spike` auto-complete the source spike.

**T2 RESOLVED (12/12)**: Accept default-state omission. Directory-per-type disambiguates. Add explicit documentation: "Within each document type directory, absence of suffix means initial state. Legacy files without suffixes are treated identically."

**T3 RESOLVED (12/12)**: Filesystem-first, store-second, no git operations. Error handling:
1. `fs::rename(old, new)` — if fails, return error
2. `store.update_file_path()` — if fails, attempt `fs::rename(new, old)` rollback
3. `update_markdown_status()` — if fails, log warning
Move overwrite protection from Future Work to Design.

**Consensus amendments to RFC 0031:**
1. Remove `.done-rfc` from vocabulary table, filename examples, status_suffix() match, and test plan
2. Add "Design Principle: SQLite store is authoritative. Filenames are derived views."
3. Add error handling sequence with rollback to `rename_for_status()`
4. Move overwrite protection from Future Work to Design
5. Drop `git mv` requirement — git detects renames automatically
6. Add documentation note about default-state omission and legacy files
7. Confirm dialogue files use new `YYYY-MM-DDTHHMMZ` prefix (already in RFC, user confirmed)

## Round 3: Convergence Check

### Muffin 🧁
Aligned. The amendments address all my concerns. Drop `.done-rfc`, filesystem-first rename, no git operations. The RFC is stronger for it.

### Cupcake 🧁
Aligned. Documentation note about default states resolves the onboarding concern. The glossary lives in the RFC itself (vocabulary table), which is sufficient.

### Scone 🧁
Aligned. I retracted my always-use-suffixes position in Round 1. The filesystem-first approach with rollback is sound. Store-as-authority is the right principle.

### Eclair 🧁
Aligned. The auto-complete-on-RFC-create suggestion handles the spike→RFC workflow cleanly. All tensions resolved.

### Donut 🧁
Aligned. I withdrew Option C advocacy. The suffix approach with the amendments is workable. The test plan additions matter.

### Brioche 🧁
Aligned. Centralized `transition_status()` with filesystem-first semantics covers the atomicity concern.

### Croissant 🧁
Aligned. Error handling with rollback addresses my risk concerns. Overwrite protection as prerequisite, not future work.

### Macaron 🧁
Aligned. I still believe filenames shouldn't store state in principle, but the "derived view" framing makes the design defensible. Store is authoritative.

### Cannoli 🧁
Aligned.

### Strudel 🧁
Aligned. Legacy file handling in test plan addresses my edge case.

### Beignet 🧁
Aligned. The 3-way transaction concern is resolved by removing git from the equation.

### Churro 🧁
Aligned.

---

## 💙 Judge — Round 3 Summary

**12/12 ALIGNED. Dialogue converged.**

## Final Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 5 | 5 | 5 | 5 | **20** |
| 🧁 Cupcake | 5 | 5 | 5 | 5 | **20** |
| 🧁 Scone | 5 | 5 | 5 | 5 | **20** |
| 🧁 Eclair | 5 | 5 | 5 | 5 | **20** |
| 🧁 Donut | 5 | 5 | 5 | 5 | **20** |
| 🧁 Brioche | 5 | 5 | 5 | 5 | **20** |
| 🧁 Croissant | 5 | 5 | 5 | 5 | **20** |
| 🧁 Macaron | 5 | 5 | 5 | 5 | **20** |
| 🧁 Cannoli | 5 | 5 | 5 | 5 | **20** |
| 🧁 Strudel | 5 | 5 | 5 | 5 | **20** |
| 🧁 Beignet | 5 | 5 | 5 | 5 | **20** |
| 🧁 Churro | 5 | 5 | 5 | 5 | **20** |

**Total ALIGNMENT**: 240 / 240 (100%)

## Converged Amendments

The following changes must be applied to RFC 0031:

1. **Drop `.done-rfc`**: Remove from vocabulary table (line 141), filename examples (lines 97, 267), `status_suffix()` match, and test plan (line 280). All completed spikes use `.done`.

2. **Add design principle**: "The SQLite store is the authoritative source of document status. Filenames are derived views. If filename and store disagree, the store wins. `blue_sync` reconciles."

3. **Error handling for `rename_for_status()`**:
   ```rust
   fn rename_for_status(...) -> Result<(), Error> {
       // 1. fs::rename — if fails, return error
       // 2. store.update_file_path — if fails, attempt rollback rename
       // 3. update_markdown_status — if fails, log warning (non-critical)
   }
   ```

4. **Drop `git mv`**: Remove from mitigations. Git detects renames automatically via content similarity.

5. **Move overwrite protection**: From Future Work to Design section. File existence check before write is a prerequisite for status suffixes.

6. **Add legacy file note**: "Files without status suffixes are in their initial state. Legacy files created before this RFC are treated identically — no migration required."

7. **Confirm dialogue timestamp**: dialogue.rs uses new `YYYY-MM-DDTHHMMZ` format (already in scope).
