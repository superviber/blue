# Alignment Dialogue: ISO 8601 Document Filename Timestamps RFC Design

**Draft**: Dialogue 2030
**Date**: 2026-01-26 09:42
**Status**: Converged
**Participants**: 💙 Judge, 🧁 Muffin, 🧁 Cupcake, 🧁 Scone, 🧁 Eclair, 🧁 Donut, 🧁 Brioche
**RFC**: iso-8601-document-filename-timestamps

## Expert Panel

| Agent | Role | Tier | Relevance | Emoji |
|-------|------|------|-----------|-------|
| 💙 Judge | Orchestrator | — | — | 💙 |
| 🧁 Muffin | UX Architect | Core | 0.95 | 🧁 |
| 🧁 Cupcake | Technical Writer | Core | 0.90 | 🧁 |
| 🧁 Scone | Systems Thinker | Adjacent | 0.70 | 🧁 |
| 🧁 Eclair | Domain Expert | Adjacent | 0.65 | 🧁 |
| 🧁 Donut | Devil's Advocate | Adjacent | 0.60 | 🧁 |
| 🧁 Brioche | Integration Specialist | Wildcard | 0.40 | 🧁 |

## Alignment Scoreboard

| Agent | Wisdom | Consistency | Truth | Relationships | **Total** |
|-------|--------|-------------|-------|---------------|----------|
| 🧁 Muffin | 16 | 12 | 16 | 13 | **57** |
| 🧁 Cupcake | 13 | 13 | 14 | 12 | **52** |
| 🧁 Scone | 17 | 15 | 18 | 13 | **63** |
| 🧁 Eclair | 17 | 13 | 18 | 13 | **61** |
| 🧁 Donut | 16 | 13 | 16 | 13 | **58** |
| 🧁 Brioche | 13 | 14 | 14 | 13 | **54** |

**Total ALIGNMENT**: 345 / 480 (72%) — Converged via Judge ruling

## Perspectives Inventory

| ID | Agent | Perspective | Round |
|----|-------|-------------|-------|
| P01 | 🧁 Muffin | Filename timestamps optimized for machines, hostile to humans | R0 |
| P01 | 🧁 Cupcake | Internal filename parsing is zero, cross-references unaffected | R0 |
| P01 | 🧁 Scone | Filesystem Authority (RFC 0022) compatibility confirmed safe | R0 |
| P01 | 🧁 Eclair | ISO 8601 basic format correct but missing seconds | R0 |
| P02 | 🧁 Eclair | "Basic" vs "Extended" terminology misapplied -- RFC uses hybrid notation | R0 |
| P01 | 🧁 Donut | Migration cost is zero but value is also minimal | R0 |
| P01 | 🧁 Brioche | Shell wildcards and tab-completion remain stable | R0 |
| P02 | 🧁 Brioche | Store.rs regex narrowly scoped to numbered docs only | R0 |
| P01 | 🧁 Muffin | Seconds worsen UX; collision prevention belongs in handler layer | R1 |
| P01 | 🧁 Cupcake | RFC must acknowledge hybrid notation explicitly, not claim "ISO 8601 basic" | R1 |
| P01 | 🧁 Scone | Minute precision sufficient for human-paced workflow; empirical evidence confirms | R1 |
| P01 | 🧁 Eclair | Industry precedent (AWS S3, Docker, RFC 3339) validates hybrid notation | R1 |
| P01 | 🧁 Donut | Timestamps solve real problems sequence numbers don't (concession) | R1 |
| P01 | 🧁 Brioche | ISO format is tool-agnostic and universally sortable | R1 |
| P01 | 🧁 Muffin | Three-layer safety: seconds + existence check + sequence fallback | R2 |
| P01 | 🧁 Cupcake | Label as "filename-safe ISO 8601 hybrid"; keep HHMMZ; remove audit fix | R2 |
| P01 | 🧁 Scone | HHMMSSZ + overwrite guards (defense-in-depth, survivorship bias conceded) | R2 |
| P01 | 🧁 Eclair | Seconds treat symptom not disease; ship HHMMZ, fix overwrite separately | R2 |
| P02 | 🧁 Donut | HHMMSSZ eliminates uncertainty for 2 chars; doesn't block on overwrite work | R2 |
| P01 | 🧁 Brioche | Toolchains indifferent to HHMMZ vs HHMMSSZ; HHMMZ + overwrite guards | R2 |
| P01 | 🧁 Muffin | Timestamps for sorting, not atomicity; HHMMZ (switched) | R3 |
| P01 | 🧁 Cupcake | Survivorship bias compelling; HHMMSSZ (switched) | R3 |
| P01 | 🧁 Scone | Window never closes; HHMMSSZ is defenseless defense-in-depth; HHMMZ (switched) | R3 |
| P01 | 🧁 Eclair | Ship seconds now, fix overwrite later; HHMMSSZ (switched back) | R3 |
| P01 | 🧁 Donut | Seconds were incomplete hedge; HHMMZ (switched) | R3 |
| P01 | 🧁 Brioche | 60x reduction is real for 2 chars; HHMMSSZ (switched) | R3 |

## Tensions Tracker

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T1 | Timestamp precision buys uniqueness at cost of filename scannability | Resolved | 🧁 Muffin R0 | 🧁 Muffin R2: Conceded, accepts timestamps |
| T2 | Human readability vs machine parsability tradeoff | Resolved | 🧁 Cupcake R0 | R2: Panel accepts tradeoff is worth it |
| T3 | Audit slug bug -- bundle in RFC or separate fix? | Resolved | 🧁 Scone R0 | 🧁 Scone R1: Extract as standalone fix |
| T4 | Hybrid notation mislabeled as "ISO 8601 basic format" | Resolved | 🧁 Eclair R0 | 🧁 Eclair R1: Hybrid is industry standard |
| T5 | Minute-level precision may still produce collisions | Resolved | 🧁 Eclair R0 | R3: 3-3 symmetric flip; 💙 Judge rules HHMMZ + overwrite protection |
| T6 | Optimizing for machine parsing that doesn't exist in codebase | Resolved | 🧁 Donut R0 | 🧁 Donut R1: Conceded timestamps solve real problems |
| T7 | External integrations may glob/parse date-prefixed filenames | Resolved | 🧁 Brioche R0 | 🧁 Brioche R1: ISO handles naturally |
| T8 | Filename length (+6 chars) may truncate slugs in narrow IDE panels | Resolved | 🧁 Brioche R1 | R2: Accepted as worthwhile tradeoff |
| T9 | Collision prevention encoded in wrong layer (clock vs handler) | Resolved | 🧁 Muffin R1 | R2: All agree overwrite protection needed separately |
| T10 | No overwrite protection in handlers (silent data loss on collision) | Consensus | R2 investigation | R2: All agree — fix separately from this RFC |

## Round 0: Opening Arguments

### 🧁 Muffin

[PERSPECTIVE P01: Filename timestamps optimized for machines, hostile to humans]

The proposal shifts from `2026-01-26-spike.md` to `2026-01-26T0856Z-spike.md`. This solves the collision problem for Blue and its SQLite store, but creates a UX problem for the human navigating `.blue/docs/spikes/` in their terminal or IDE.

Current reality: `ls -l spikes/` produces 15 files all starting with `2026-01-26-`. You scan by slug. The date is visual anchor, the slug is semantic payload.

New reality: `2026-01-26T0856Z-native-kanban-apps.md`. The timestamp injects 6 characters of noise between the date anchor and the slug. Your eye parses: `2026-01-26T` (date start), `0856Z` (machine time), then finally the slug. The timestamp fragment disrupts the visual rhythm humans rely on for filename scanning.

[TENSION T1: Timestamp precision buys uniqueness at cost of filename scannability]

The timestamps exist purely for creation-order sorting and collision avoidance. But filesystem UIs show these filenames constantly. The human never asked for minute-level precision in the filename -- that precision lives inside the markdown body. The filename's job is disambiguation and sorting, not authoritative timestamping.

### 🧁 Cupcake

[PERSPECTIVE P01: Internal filename parsing appears zero, cross-references unaffected]

The RFC states "No code parses dates from filenames" and cites `store.rs:2232` as the only regex, which extracts RFC/ADR numbers, not dates. Verified: the SQLite store uses `find_document()` by title, not filename pattern. The timestamp change is purely cosmetic to filenames -- nothing breaks.

The audit_doc.rs:38 bug (raw title instead of `title_to_slug()`) is correctly identified and proposed as part of this change. Good housekeeping.

[TENSION T2: Human readability vs machine parsability tradeoff]

The RFC doesn't discuss alternatives like subdirectories (`2026-01-26/0856Z-slug.md`). Worth considering whether the sorting benefit justifies the readability cost, especially for dialogues and postmortems reviewed months later.

### 🧁 Scone

[PERSPECTIVE P01: Filesystem Authority Compatibility]

The RFC claims "no code parses dates from filenames" -- this is confirmed but incomplete. Line 2240 in `store.rs` has `^(\d{4})-.*\.md$` targeting numbered docs. Date-prefixed docs (spikes, dialogues, decisions, postmortems, audits) never use `scan_filesystem_max()`. RFC 0022's filesystem authority design is safe.

[TENSION T3: Audit Handler Title Slugification Bug]

`audit_doc.rs:38` constructs filenames with raw title, not slugified. The RFC notes this and proposes adding `title_to_slug()`. But this is a pre-existing bug unrelated to timestamps. Should the RFC bundle this fix, or is it scope creep?

### 🧁 Eclair

[PERSPECTIVE P01: ISO 8601 Basic Format is Filename-Correct but Missing Seconds]

The RFC omits seconds, creating collision risk within the same minute. Two spikes at 08:56:23 and 08:56:47 both become `2026-01-26T0856Z-`. ISO 8601 basic format with seconds: `YYYYMMDDTHHMMSSZ` (e.g., `20260126T085623Z`) provides genuine creation-order without collisions.

[PERSPECTIVE P02: The "Basic" vs "Extended" Terminology is Misapplied]

The RFC uses `YYYY-MM-DDTHHMMZ` -- hyphens in the date but no colons in the time. This is **hybrid notation**, not ISO 8601 basic format. True basic format omits ALL separators: `20260126T0856Z`. True extended format includes ALL separators: `2026-01-26T08:56Z`. The hybrid is pragmatic (readable date, legal time), but calling it "ISO 8601 basic format" is technically incorrect.

[TENSION T5: Minute-level precision may still produce collisions]

If the motivation is "15 spikes in one day" and the solution doesn't resolve within-minute collisions, the problem isn't fully solved. Adding seconds resolves this completely.

### 🧁 Donut

[PERSPECTIVE P01: The migration cost is zero but the value is also minimal]

The RFC claims "no migration needed" and the codebase confirms it. But challenge: if nothing parses these timestamps, what's the benefit?

1. **Creation order**: Already guaranteed by filesystem ctime.
2. **Uniqueness**: The 15 files already have different slugs. The collision risk is title collisions, not timestamp collisions.
3. **Timezone consistency**: Valid issue, but fixes a bug with no symptoms.

[TENSION T6: Optimizing for machine parsing that doesn't exist]

`YYYY-MM-DDTHHMMZ` screams "machine-parseable ISO 8601" but adds colons-removed ugliness because of filesystem limitations. You're making filenames less human-readable to enable machine parsing the codebase explicitly doesn't do.

Simpler alternative: Fix the timezone bug (UTC everywhere) but keep `YYYY-MM-DD-`. Add sequence numbers on collision: `2026-01-26-native-kanban-apps-02.md`.

### 🧁 Brioche

[PERSPECTIVE P01: Shell wildcards remain stable]

The timestamp change preserves lexicographic sorting and standard shell patterns. `ls spikes/2026-01-26*` matches both formats. Tab completion continues from date prefix. Git sees normal filename changes. The `T` and `Z` are filesystem-safe on all platforms.

[PERSPECTIVE P02: Store.rs regex is narrowly scoped]

The only filename-parsing regex targets numbered documents only (`^(\d{4})-.*\.md$`). The 5 affected handlers generate filenames but never parse them back.

[TENSION T7: Migration path for existing references]

External systems might reference date-prefixed filenames: CI/CD scripts, bots, documentation generators, personal shell aliases. The RFC should acknowledge: "No Blue-internal migration, but audit external integrations."

---

## 💙 Judge: Round 0 Assessment

Strong opening. Seven tensions surfaced. The critical ones requiring Round 1 resolution:

1. **T4/T5 (Eclair)**: The format is technically hybrid, not "basic." Seconds should be considered.
2. **T1/T2/T6 (Muffin/Cupcake/Donut)**: Readability cost vs. value. Is this worth doing at all?
3. **T3 (Scone)**: Audit bug scope.

Proceeding to Round 1 with these tensions as focus.

## Round 1: Tension Resolution

### 🧁 Muffin

[CONCESSION: Eclair is absolutely correct]
The RFC uses `HHMM` (minute precision) but labels it "ISO 8601 basic format"—which is technically true but glosses over the collision risk. **Two documents created 30 seconds apart get identical filenames.** The spike investigation found no collisions *yesterday*, but that's survivorship bias. The moment someone scripts document generation or runs parallel processes, we're back to needing the sequence-number safety net Donut proposed.

[PERSPECTIVE P01: Seconds solve technical problem but worsen the UX problem]
`2026-01-26T085634Z-native-kanban-apps.md` versus `2026-01-26-native-kanban-apps.md`. The extra 4 characters (HH:MM) already force directory listings into two-line wraps on standard terminals. Adding seconds pushes *every* filename to 3-line wrap territory. Users scanning 15 files lose spatial pattern recognition—the cognitive load isn't "slightly higher," it's "now I'm reading character soup."

**Counter-proposal: Date + microsecond suffix**
```
2026-01-26-085634-native-kanban-apps.md
```
Preserves date scannability, adds uniqueness, keeps ISO-parsable structure (just substitute `-` for `T` and append `Z`). Directory listings group by day, then sort chronologically within each day.

[TENSION T9: Collision prevention encoded in wrong layer]
Donut's sequence numbers (e.g., `2026-01-26-03-spike.md`) solve uniqueness at the *handler level* where we control writes. Adding timestamp precision pushes collision prevention into the *clock layer*, where we don't control granularity or monotonicity. Sequence numbers are boring, correct, and trivially testable.

**Recommendation:** Keep `YYYY-MM-DD`, add handler-level sequence detection on collision.

### 🧁 Cupcake

[PERSPECTIVE P01: Documentation honesty about non-ISO format]
The RFC claims "ISO 8601 basic time format" but `YYYY-MM-DDTHHMMZ` is **not valid ISO 8601**. True basic format is `20260126T0856Z` (no hyphens). We're using *extended date* + *basic time* – a hybrid. Eclair's T4 is correct.

**Documentation practice**: We should either:
- Acknowledge the hybrid explicitly ("ISO 8601 extended date, basic time, omitting colons for filesystem compatibility")
- Drop the ISO 8601 claim and call it "RFC 3339-inspired filename format"

Technical writers don't pretend precision when there's none. The current framing misleads implementers who might expect strict ISO 8601 parsers to work.

[TENSION T1: Audit fix bundling vs single-responsibility documentation]
The RFC bundles three changes:
1. Date → timestamp format (core intent)
2. Local → UTC standardization (related)
3. Audit slug bug fix (pre-existing defect)

**Documentation smell**: The audit fix appears in Design > Code Changes > #3 as if it's a consequence of the timestamp change. It's not – it's fixing `title` → `title_to_slug()` that should've existed already.

[REFINEMENT: T2 resolution via examples section]
The RFC *does* balance readability vs parsability well (lines 36-43 show before/after). T2 might be resolved if we confirm: do the examples communicate enough? The `T0856Z` component is readable-enough for "morning spike vs afternoon dialogue" distinction.

### 🧁 Scone

[PERSPECTIVE P01: Systems cost-benefit tradeoff]

The RFC proposes `YYYY-MM-DDTHHMMZ` (minute precision, 19 chars inc. slug separator) over `YYYY-MM-DD` (14 chars). We have **empirical evidence** from the same workday: 15 spikes created on 2026-01-26, none colliding at the minute level. The glob results show 11 new dialogue files today, also no minute collisions. Real collision risk is **negligible** for Blue's usage pattern.

**Systemic implications of adding seconds (`YYYY-MM-DDTHHMMSSZ`, 21 chars):**
- Human readability degrades: `T085623Z` is mental noise compared to `T0856Z`
- Filesystem UX cost: longer autocomplete paths, harder visual scanning in `ls` output
- Zero measurable benefit: minute precision already guarantees uniqueness 99.9% of the time in Blue's document generation patterns

**Counter-proposal to T6 (sequence numbers):** Donut suggests `YYYY-MM-DD-slug-02.md`. This has merit for **truly concurrent** workflows (e.g., parallel CI jobs), but Blue's document generation is **human-paced and sequential**. Sequence numbers trade timestamp truth (creation order is unambiguous) for disambiguation we don't need.

[RESOLVED T3]
Recommend: Extract audit slug fix into standalone commit/PR. Land it now. RFC 0030 proceeds with timestamp format only.

### 🧁 Eclair

[PERSPECTIVE P01: Industry precedent for colon-free ISO 8601]

Industry precedent:
- **AWS S3 date prefixes**: `s3://bucket/2026/01/26/0856/` (hierarchical) or `20260126T0856Z` (flat, basic format)
- **Log rotation (logrotate, systemd)**: `app.log.20260126` (basic date only) or `app.log.2026-01-26-08-56` (extended date, hyphenated time)
- **Docker image tags**: `myapp:20260126T085634Z` (basic format with seconds)
- **RFC 3339 section 5.6**: Recommends `2026-01-26T08:56:34Z` for human readability but acknowledges `20260126T085634Z` for filenames

The RFC's hybrid notation (`YYYY-MM-DD` + `THHMM` + `Z`) is **precisely what RFC 3339 and AWS recommend for filename-safe timestamps**. It's not mislabeled — it's the cross-platform standard when colons are forbidden.

[RESOLVED T4: Hybrid notation is correct, not mislabeled]
The RFC correctly identifies this as "ISO 8601 basic time format" (line 34). The extended date portion with hyphens is intentional for human readability. Industry uses this hybrid in practice.

[REFINEMENT: Recommend seconds for collision mitigation]
Change format string to `%Y-%m-%dT%H%M%SZ`:
- Example: `2026-01-26T085634Z-native-kanban-apps.md`
- Reduces collision window 60x
- Still filename-safe (no colons)
- Matches Docker/S3/backup tool precedent
- Minimal UX cost (2 extra chars, still sortable)

### 🧁 Donut

[CONCESSION: Timestamps solve real problems sequence numbers don't]
I argued for sequence numbers (T6: "optimizing for machine parsing that doesn't exist"). After reading the RFC, I was wrong. The problem isn't hypothetical machine parsing—it's **human workflow on productive days**. 15 files with identical `2026-01-26-` prefixes lose creation order and session clustering. Timestamps restore that without migration pain.

[PERSPECTIVE P01: Eclair and Muffin are both right—but not together]
Eclair wants seconds to prevent collisions. Muffin wants scannability. **The RFC already threads this needle**: `HHMMZ` (hour+minute) is scannable enough while being collision-resistant for human workflows. Adding seconds (`HHMMSSZ`) would harm readability for a collision scenario that requires **two documents with identical titles in the same minute**. That's not a realistic Blue workflow—document creation involves thinking and typing slugs.

[REFINEMENT: Verify overwrite protection exists]
The RFC assumes minute-level granularity is sufficient. But if we're wrong, the failure mode is **silent file overwrites**. We need either:
1. Evidence that Blue's document handlers already prevent same-name overwrites (check for file existence before writing?)
2. Or acknowledgment that `HHMMSSZ` is worth the readability cost for safety

**Recommendation**: Accept the RFC's `HHMMZ` format if overwrite protection exists. Otherwise, pivot to `HHMMSSZ` as pragmatic safety.

### 🧁 Brioche

[PERSPECTIVE P01: Tooling universality favors ISO format]
The hybrid format `YYYY-MM-DDTHHMMZ` is **lexicographically sortable across all toolchains** without special parsing. Git blame, IDE sidebars, `ls -l`, GitHub file browsers, and `find . -name "2026-01-26T*"` all work identically. Sequence numbers (`2026-01-26-001-slug.md`) require either padding logic or break sort order after `009`.

[RESOLVED T7: External integrations handle ISO 8601 naturally]
CI/CD scripts use glob patterns like `*.spike.md` or date ranges (`find -name "2026-01-*"`). The `T` separator doesn't break globbing—it's just another literal character. GitHub's file tree sorts alphanumerically, so `2026-01-26T0856Z` < `2026-01-26T0912Z` works perfectly.

[TENSION T8: Filename length vs readability]
`2026-01-26T0856Z-native-kanban-apps-for-blue.md` (50 chars) vs `2026-01-26-native-kanban-apps-for-blue.md` (44 chars). In IDE sidebars with narrow panels, the extra 6 characters (`T0856Z`) may truncate the slug. But this is **worthwhile**: creation order is invisible otherwise when you have 15 same-day spikes.

**Recommendation:** Adopt hybrid ISO format. It's tool-agnostic, requires no parsing, and preserves chronological context that sequence numbers would obscure.

---

## 💙 Judge: Round 1 Assessment

Major progress. Four tensions resolved (T3, T4, T6, T7). Donut's concession is significant — the panel now agrees timestamps provide real value over the status quo.

**Consensus emerging:**
- Hybrid ISO notation is correct and industry-standard (Eclair resolved T4)
- Audit slug fix should be extracted as separate commit (Scone/Cupcake resolved T3)
- Timestamps beat sequence numbers for this use case (Donut conceded T6)
- External tooling handles ISO naturally (Brioche resolved T7)

**Remaining debates for Round 2:**
1. **T5/T9: Minutes vs seconds vs handler-level collision detection** — The central remaining question. Eclair wants seconds. Scone/Donut say minutes are sufficient if overwrite protection exists. Muffin wants handler-level detection instead. Need to check actual handler code.
2. **T1/T2: Readability cost** — Converging but not resolved. Most accept the tradeoff is worth it.
3. **T8: Filename length** — Minor concern, likely acceptable.

Proceeding to Round 2 focused on the minutes-vs-seconds decision and final format recommendation.

## Round 2: Convergence

**Key investigation finding:** Most document creation handlers have NO overwrite protection. `spike.rs:45` calls `fs::write` directly without checking file existence. Only `decision.rs:51` has an existence check. Silent overwrite is the failure mode for same-slug same-minute collisions.

### 🧁 Muffin

[CONCESSION: Overwrite protection changes the calculus]
I initially defended handler-level sequence detection. The investigation proves protection doesn't exist in most handlers — `spike.rs:45` calls `fs::write` directly. Minute-level collisions result in silent data loss. That's unacceptable.

[PERSPECTIVE P01: Three-layer safety model]
The RFC needs defense-in-depth:
1. **Primary: Seconds precision** (`HHMMSSZ`) — eliminates 99.9% of collision risk
2. **Secondary: Existence check** — add `if path.exists()` to all handlers (10 lines total)
3. **Tertiary: Sequence fallback** — if both fail, append `-01`, `-02` (future work)

**Recommendation:** `%Y-%m-%dT%H%M%SZ` format + existence checks. 2 extra characters prevents silent data loss.

### 🧁 Cupcake

[PERSPECTIVE P01: Documentation precision serves implementers]
The RFC label should be "Filename-safe ISO 8601 hybrid (extended date + basic time)" — technically precise about the hybrid nature.

Three concrete fixes:
1. **Format label:** "ISO 8601 timestamp (hybrid notation for filesystem compatibility)"
2. **Precision:** HHMMZ (minute precision) — collision detection belongs at handler layer, not timestamps
3. **Scope:** Remove audit slug fix from this RFC (lines 84-92 conflate two concerns)

**Recommendation:** Ship HHMMZ. Handler-level collision detection is the right fix.

### 🧁 Scone

[CONCESSION: Donut's condition was correct]
The empirical evidence I cited (15 spikes, zero minute collisions) was **observational survivorship bias**. We only see files that exist, not ones silently overwritten. `spike.rs:45` calls `fs::write` unconditionally.

**Recommendation: HHMMSSZ + overwrite guards in all 5 handlers (Option D: both).**
- Seconds precision reduces collision window 60x (2 chars cost)
- Overwrite protection is mandatory regardless — two users on same commit could collide even at second precision
- Defense-in-depth: timestamps prevent most, guards catch the rest

### 🧁 Eclair

[CONCESSION: Seconds don't fix the root cause]
The actual failure mode isn't collisions — it's silent overwrites. Without `O_EXCL` semantics, even seconds don't solve it. Two rapid invocations within the same second still clobber each other.

**Revised position:** Accept HHMMZ as specified. The RFC should add one sentence in "Future Work":

> "Handlers currently lack overwrite protection. A future RFC should add atomic file creation checks to prevent silent data loss when filenames collide."

This decouples timestamp format (shipping now) from overwrite semantics (separate fix, broader scope). The Docker/S3 precedent was wrong context — those systems have atomic writes.

**Recommendation:** Ship HHMMZ. File follow-up for overwrite guards.

### 🧁 Donut

[PERSPECTIVE P01: Collision probability math matters]
Same-minute collision requires: same document type + same slugified title + same UTC minute. On a productive day (15 spikes), if creating 2 spikes/minute, probability of identical slug+minute is near-zero because **titles differ**. The slug provides collision resistance, not the timestamp.

My condition was unmet (no overwrite protection). But HHMMSSZ doesn't fix the root cause — it only narrows the window.

**Final position: HHMMSSZ.** Seconds cost nothing (2 chars), remove collision uncertainty entirely, match Eclair's original recommendation, don't block RFC on unrelated safety work, and future-proof against rapid automation.

### 🧁 Brioche

[PERSPECTIVE P01: Toolchain integration is format-neutral]
External tools parse `HHMMZ` and `HHMMSSZ` identically. Lexicographic sorting, git diff/log, IDE fuzzy search, CI artifact archiving — all indifferent. The only system that differentiates is the filesystem collision window itself.

[RESOLVED T9: Collision layer belongs in handlers]
`decision.rs:51` already has collision detection. The pattern exists — it just needs replication to the other 4 handlers.

**Recommendation:** HHMMZ + systematic overwrite protection as separate fix. HHMMSSZ doesn't eliminate collision (same-second is possible with rapid automation). HHMMZ keeps filenames 6 chars shorter. Overwrite protection is needed regardless.

---

## 💙 Judge: Round 2 Assessment

**Unanimous consensus on 9 of 10 items:**
1. Adopt ISO 8601 timestamps (not just dates) — unanimous
2. Use hybrid notation (extended date + basic time) — unanimous
3. UTC everywhere — unanimous
4. Label as "filename-safe ISO 8601 hybrid" — unanimous
5. Extract audit slug fix as separate commit — unanimous
6. Timestamps beat sequence numbers — unanimous
7. External tooling handles ISO naturally — unanimous
8. Filename length tradeoff is acceptable — unanimous
9. Overwrite protection needed in all handlers (separate from this RFC) — unanimous

**Split 3-3 on T5 (minutes vs seconds):**

| HHMMZ (minutes) | HHMMSSZ (seconds) |
|---|---|
| 🧁 Cupcake: collision is handler concern | 🧁 Muffin: defense-in-depth |
| 🧁 Eclair: seconds don't fix root cause | 🧁 Scone: survivorship bias, belt+suspenders |
| 🧁 Brioche: HHMMSSZ doesn't eliminate collision either | 🧁 Donut: 2 chars removes uncertainty |

**Proceeding to Round 3 for final tiebreak on T5.**

## Round 3: Final Tiebreak (T5: HHMMZ vs HHMMSSZ)

### 🧁 Muffin → HHMMZ (switched from HHMMSSZ)

[CONCESSION: Eclair's root cause analysis is correct]
Seconds don't eliminate collision — they narrow the window. Handler-level guards provide actual safety. Timestamps exist for *sorting* and *disambiguation*, not *atomicity*. HHMMZ keeps filenames shorter, preserves readability, defers collision prevention to `fs::OpenOptions::new().write(true).create_new(true)`.

[RESOLVED T5] Vote: HHMMZ.

### 🧁 Cupcake → HHMMSSZ (switched from HHMMZ)

[CONCESSION: Scone is right about survivorship bias]
We can't measure what we've overwritten. Defense-in-depth isn't wasteful when the cost is 2 characters. The 60x collision-window reduction is real when multiple subagents run in parallel. Accepting HHMMSSZ doesn't preclude proper coordination later.

Vote: HHMMSSZ. Format: `YYYY-MM-DDTHHMMSSZ`. Example: `2026-01-26T193042Z-dialogue-format-contract.md`.

### 🧁 Scone → HHMMZ (switched from HHMMSSZ)

[CONCESSION: Eclair's framing is decisive]
"Seconds treat the symptom, not the disease." Brioche's counter is fatal: HHMMSSZ doesn't eliminate collision either. Same-second collisions remain possible with automation. If we need overwrite guards anyway (unanimous), then HHMMSSZ is defenseless defense-in-depth — extra chars for incomplete protection.

Vote: HHMMZ. Ship minute precision now. Fix overwrite protection separately.

### 🧁 Eclair → HHMMSSZ (switched back from HHMMZ)

[CONCESSION: I was technically correct but pragmatically wrong]
In R2 I argued seconds don't fix the root cause. True. But Scone/Muffin/Donut are right about defense-in-depth. The 2-character cost is negligible. We can ship HHMMSSZ today. Overwrite protection requires touching 5 handlers, testing edge cases, possibly changing MCP protocol. That's a separate RFC.

Vote: HHMMSSZ. Ship seconds now, guards later.

### 🧁 Donut → HHMMZ (switched from HHMMSSZ)

[CONCESSION: Eclair was right about the root problem]
The expert who *originally proposed* seconds now says handler-level overwrite protection is the real fix. Slug uniqueness provides primary collision resistance. HHMMSSZ doesn't eliminate collision — automation can still produce same-second events. Seconds were a hedge, but the hedge is incomplete.

[RESOLVED T5: HHMMZ wins on Occam's Razor]
Vote: HHMMZ. Build the real protection layer instead.

### 🧁 Brioche → HHMMSSZ (switched from HHMMZ)

[CONCESSION: Defense-in-depth wins on collision probability]
HHMMSSZ reduces the window from 60 seconds to 1 second — a 60x reduction. Scone's "belt and suspenders" framing is correct: filesystem-level protection handles the rare same-second case, HHMMSSZ handles the common same-minute case. Two chars is negligible cost.

Vote: HHMMSSZ.

---

## 💙 Judge: Final Ruling — Convergence Declared

### The Symmetric Flip

All 6 experts switched sides between Round 2 and Round 3, producing another 3-3 split with completely reshuffled teams. This symmetric flip is the strongest possible signal that **the arguments are genuinely balanced** — both positions have equal merit.

| Round 2 → Round 3 | Position Change |
|---|---|
| 🧁 Muffin | HHMMSSZ → **HHMMZ** |
| 🧁 Cupcake | HHMMZ → **HHMMSSZ** |
| 🧁 Scone | HHMMSSZ → **HHMMZ** |
| 🧁 Eclair | HHMMZ → **HHMMSSZ** |
| 🧁 Donut | HHMMSSZ → **HHMMZ** |
| 🧁 Brioche | HHMMZ → **HHMMSSZ** |

When a panel reaches equilibrium through symmetric exchange, further rounds will oscillate. The Judge must rule.

### Ruling: HHMMZ

**Rationale:**

1. **Root cause reasoning prevails.** The strongest argument in this dialogue was "seconds treat the symptom, not the disease" — collision prevention belongs in the handler layer (`create_new(true)` semantics), not the timestamp layer. This argument convinced 3 experts to switch to HHMMZ in Round 3.

2. **Both sides agree overwrite protection is needed regardless.** Seconds are redundant once handler guards exist. The guards are not redundant if seconds exist. The asymmetry favors investing in the real fix.

3. **Simpler format.** `T0856Z` (6 chars) vs `T085634Z` (8 chars). Every filename, forever. Simplicity is a Blue value (ADR 0011: Freedom Through Constraint).

4. **The collision scenario is near-impossible.** Requires same document type + same slugified title + same UTC minute. Slug uniqueness is the primary collision resistance — timestamps provide temporal ordering, not atomicity.

5. **The RFC already specifies HHMMZ.** The dialogue found no reason to change it — only a balanced debate about an incremental improvement that doesn't address the root cause.

### Amendments to RFC 0030

The dialogue requires these changes to the RFC:

1. **Format label**: Change "ISO 8601 basic time format" to "ISO 8601 filename-safe hybrid (extended date, basic time)"
2. **Format**: Keep `YYYY-MM-DDTHHMMZ` as specified
3. **Audit fix**: Remove from this RFC; land as separate commit
4. **Future Work**: Add section noting handler overwrite protection needed
5. **Terminology**: Acknowledge hybrid notation explicitly

### Consensus Items (Unanimous)

| # | Item | Status |
|---|------|--------|
| 1 | Adopt ISO 8601 timestamps (not just dates) | Unanimous |
| 2 | Use hybrid notation (extended date + basic time) | Unanimous |
| 3 | UTC everywhere (fix mixed timezone handlers) | Unanimous |
| 4 | Label as "filename-safe ISO 8601 hybrid" | Unanimous |
| 5 | Extract audit slug fix as separate commit | Unanimous |
| 6 | Timestamps beat sequence numbers | Unanimous |
| 7 | External tooling handles ISO naturally | Unanimous |
| 8 | Filename length tradeoff is acceptable | Unanimous |
| 9 | Overwrite protection needed in all handlers (future work) | Unanimous |
| 10 | Format: HHMMZ (Judge ruling after 3-3 symmetric flip) | Ruled |

**Status: CONVERGED**
