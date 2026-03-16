# Round 1 Summary — PM Repo Sync Architecture

## Resolutions (5 tensions closed)

### T01: Writeback Location — RESOLVED
**Consensus**: Inline `jira_url` in front matter. Panel converged from both sides:
- Muffin, Eclair, Croissant, Macaron: pure inline with `.gitattributes` merge driver
- Scone, Cupcake, Donut, Brioche: conceded inline for DX, some proposed sidecar manifest as additional machine state store

The winning argument was Eclair's "writeback is the onboarding contract" — a contributor opens a file, sees the URL, clicks through. A mapping file requires hidden knowledge. Merge conflicts handled by `.gitattributes` ours strategy on `jira_url:` lines + conventionally-prefixed writeback commits.

### T02: Writeback Atomicity — RESOLVED
**Consensus**: Batch writeback. Sync engine collects all WritebackOps, applies only after all Jira API calls succeed, creates single atomic commit. No partial state possible. Multi-meeple concurrency handled by deliver/rebase flow.

### T04: First-Sync Safety — RESOLVED
**Consensus** (Eclair): Three-state model. (1) `--dry-run` is default on first detection, (2) `--confirm` executes, (3) subsequent syncs are implicit. Presence of `jira_url` in any file signals past first-sync.

### T05: Read-Back — RESOLVED
**Consensus**: Post-sync DriftDetector producing typed DriftReport. Three severity levels. CI gate via `--check`. NOT bidirectional sync. Croissant narrows scope to status + sprint as Jira-authoritative fields; everything else is git-authoritative with drift warnings.

### T06 + T08: Security — RESOLVED
Brioche: .env.local eliminated as credential source (pre-flight check, not a tier). Domain pinning via tracked hash file, mandatory pre-sync gate.

## Remaining Tensions (3)

### T03: depends_on Semantics (Refined)
Scone: default to `Blocks`, allow per-edge `link_type` override. Macaron: needs separate `relates_to` field in schema. Minor schema design question — not architecturally load-bearing.

### T07: CLI/TUI Gap (Open)
Croissant: git-first workflow is a hiring filter. Need `blue status`, `blue board`, or explicit acknowledgment that PM repo targets git-native teams. Eclair concedes status field deserves special treatment.

### T09: Lock-Ref Access (New, Open)
Donut: `refs/sync/lock` requires push access. May need delegation for restricted orgs. Low priority for solo-founder MVP.

## Convergence Assessment

Velocity dropped from 24 → 11. Five of eight original tensions resolved. DocSource trait shape converging. The panel is near convergence — Round 2 should aim to close T03 and T07, acknowledge T09 as future work, and signal convergence.

## Architectural Decisions Emerging

1. **DocSource trait** with `discover()`, `resolve_links()`, `writeback()`
2. **Two-pass sync**: create all → link all → verify → writeback
3. **Inline jira_url writeback** with batch commit
4. **DriftDetector** as separate component, not sync engine concern
5. **First-sync safety gate** with three-state model
6. **jira.toml domain pinning** as mandatory pre-sync check
7. **No fourth credential tier** — .env.local is for non-secret config only
