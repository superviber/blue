[PERSPECTIVE P03: Resolve T01 with a hybrid -- inline `jira_url` for DX, sidecar `.sync/manifest.yaml` for machine state]

The T01 split (inline front matter vs mapping file) is a false binary. Both sides are right about different concerns. Eclair is correct that `jira_url` in front matter is the primary onboarding signal -- a contributor opens a story file and immediately sees it is live. Scone is correct that sync-generated state mixed into authored artifacts creates coupling and merge conflict risk. The resolution: write exactly ONE field back into front matter (`jira_url: https://...atlassian.net/browse/SCRUM-42`) as a read-only, human-facing link. All other machine state -- jira_key, last_synced_at, remote_status, field hashes for drift detection -- lives in `.sync/manifest.yaml`, a single sidecar file keyed by doc ID. This gives you:

1. **Onboarding DX**: Contributors see the link. Click it. Done.
2. **Pristine authored content**: The only mutation to a story file is one URL field appended on first sync. No ongoing churn.
3. **Merge conflict avoidance**: The manifest file is machine-owned, auto-committed with `blue-sync:` prefix, and conflicts there are resolvable by last-writer-wins since all values are derived from Jira API responses.
4. **Single join point**: Every downstream tool reads manifest.yaml for correlations instead of parsing every story file's front matter.

The DocSource trait's `writeback()` method returns two mutation sets: `front_matter_patches` (just jira_url on first sync) and `manifest_entries` (everything else, every sync).

[REFINEMENT: T01 resolved via hybrid writeback -- inline jira_url + sidecar manifest]

---

[PERSPECTIVE P04: T02 atomicity solved by treating writeback as a single manifest commit, not per-file mutations]

Muffin and Donut both flagged the atomicity gap. If writeback targets dozens of individual story files, partial failure leaves an inconsistent repo. With the hybrid model from P03, the atomicity problem largely dissolves:

- **Front matter patches** (jira_url) only happen on first sync per file. They are idempotent -- re-running sync on a file that already has `jira_url` is a no-op. If sync is interrupted, the next run fills in the remaining files. No partial state to recover from.
- **Manifest updates** are a single file (`.sync/manifest.yaml`). Write it atomically (write to temp, rename) and commit. One file, one commit, one atomic operation. If sync fails before the manifest write, the manifest still reflects the previous consistent state.

For multi-meeple concurrency (Donut's T01 from round-0): meeples work in worktrees off develop. Writeback commits to the manifest happen at deliver time, not during local sync. Each meeple's local `.sync/manifest.yaml` is their working copy; on `lc-crew deliver`, the manifest merges into develop. Since manifest entries are keyed by doc ID with deterministic values (Jira API is the source), merge conflicts are mechanically resolvable -- take the entry with the later `last_synced_at`.

[RESOLVED T02: Manifest-based writeback is inherently atomic (single file write + rename). Multi-meeple concurrency defers to the existing deliver/rebase flow with mechanical merge resolution.]

---

[PERSPECTIVE P05: T05 read-back is a post-sync validation pass, not a sync mode]

Croissant and Macaron established that Jira-native events (sprint close, status transitions by teammates) make pure unidirectional sync eventually inconsistent. The panel leaned toward "project then verify." Here is the concrete shape:

After the push pass completes, `blue sync` optionally (or by default in CI) runs a **verify pass**:

```
trait DocSource {
    fn discover(&self) -> Vec<SyncItem>;          // parse repo into items
    fn writeback(&self, results: &SyncResults)     // hybrid writeback (P03)
        -> (FrontMatterPatches, ManifestEntries);
}

trait IssueTracker {
    fn push(&self, items: &[SyncItem]) -> SyncResults;   // create/update issues
    fn verify(&self, manifest: &Manifest) -> Vec<Drift>;  // read-back check
}
```

`verify()` reads the Jira state for every key in the manifest and compares against the git-declared state. Output is a `Vec<Drift>` with three severity levels:
- **info**: Jira field differs but is tracker-authoritative (e.g., actual sprint dates vs declared dates) -- log, do not alert
- **warn**: Jira status diverged from git status (someone moved a card in Jira) -- surface in CLI output and CI check
- **error**: Jira issue deleted or project key changed -- hard fail, require manual resolution

This is NOT bidirectional sync. Git remains authoritative. The verify pass is a read-only health check that prevents silent drift from eroding trust (Croissant's core concern). It runs after push, or standalone via `blue sync --check`.

[REFINEMENT: T05 -- read-back is `IssueTracker::verify()`, a post-push read-only pass producing typed Drift results. Not a sync mode, not bidirectional.]

---

[PERSPECTIVE P06: DocSource trait shape must account for two-pass linking]

Building on Muffin's P02 and Scone's P02 from round-0 (both identified the two-pass requirement for `depends_on`), the DocSource trait needs to explicitly separate item discovery from link resolution:

```rust
trait DocSource {
    /// Parse the repo, yielding items with unresolved local references
    fn discover(&self) -> Vec<SyncItem>;

    /// Given the id-to-tracker-key mapping from pass 1, resolve local
    /// references (depends_on: VOT-002) into tracker-native link requests
    fn resolve_links(&self, mapping: &IdMap) -> Vec<LinkRequest>;

    /// Write back tracker state to the repo
    fn writeback(&self, results: &SyncResults) -> WritebackSet;
}
```

The sync engine then orchestrates: `discover() -> push(items) -> resolve_links(mapping) -> push_links(links) -> verify() -> writeback()`. This makes the two-pass nature explicit in the trait contract rather than hiding it inside the sync engine. A DocSource that has no `depends_on` semantics (e.g., the existing RFC source) returns an empty `Vec<LinkRequest>` -- zero cost, no special casing.

[CONCESSION: Muffin and Scone were right in round-0 that two-pass is a hard sequencing constraint. I am integrating their insight directly into the trait shape rather than treating it as an engine concern.]

---

[PERSPECTIVE P07: Manifest file makes Macaron's DAG validation trivial]

Macaron proposed `blue sync --check` as a CI gate rejecting impossible sprint orderings. With the `.sync/manifest.yaml` holding the full id-to-jira-key mapping plus sprint assignments, DAG validation becomes a local operation:

1. Build the dependency graph from `depends_on` fields across all story files
2. For each edge (A depends on B), verify A's sprint >= B's sprint
3. Fail CI if any backward dependency is found

This does not require Jira API access -- it runs entirely against the repo state. The manifest just provides the correlation data if you want to include Jira-side sprint state in the check (via the verify pass).

---

| Tension | Round 1 Position |
|---------|-----------------|
| T01 | **Proposed resolved**: Hybrid writeback (inline jira_url + sidecar manifest). Preserves Eclair's DX, addresses Scone's coupling concern, eliminates Donut's per-file conflict risk. |
| T02 | **Proposed resolved**: Single-file manifest write is atomic. Multi-meeple concurrency handled by deliver flow with mechanical merge. |
| T03 | Remains open but scoped: `resolve_links()` in DocSource maps flat `depends_on` to Jira link type. Default to "blocks" link type; make configurable in jira.toml. |
| T05 | **Refined**: `IssueTracker::verify()` as post-push read-only pass. Three severity tiers. Always-on in CI, opt-in locally. |
