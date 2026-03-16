[PERSPECTIVE P01: Hybrid writeback -- mapping file as source of truth, front matter as cached projection]

I raised T01 in Round 0 arguing that inline `jira_key` writeback creates bidirectional coupling. Eclair countered that inline writeback is the onboarding contract -- contributors see `jira_url` and know the file is live. Both points are valid. I am refining my position rather than conceding.

The resolution is a two-layer approach: the sync engine writes to `.sync/jira-keys.yaml` as the canonical mapping (keyed by PM doc ID). A post-sync hook or `blue sync --hydrate` then projects `jira_url` into front matter as a read-only cached field. The distinction matters:

1. **Merge conflicts**: The mapping file is append-only and keyed by unique IDs, so concurrent meeple writes rarely conflict. Front matter conflicts (if two meeples sync overlapping files) are resolved by re-running `--hydrate` from the mapping file, not by manual merge.
2. **Pristine authoring**: Contributors never need to manually write or maintain `jira_url`. It appears after sync. If deleted, `--hydrate` restores it. If hand-edited, `--hydrate` corrects it.
3. **Onboarding DX preserved**: Eclair's concern is met -- the front matter still shows `jira_url` for discoverability. But the mapping file is the join key for tooling, dashboards, and drift detection, avoiding the fragile per-file parse that inline-only requires.
4. **Atomicity simplified**: Donut and Muffin raised T02 (atomicity). Writing one mapping file atomically is trivial (write-then-rename). Hydrating front matter can be a separate, idempotent step that tolerates partial completion.

This makes the mapping file the sync engine's contract surface and front matter the human DX surface. They are decoupled by an explicit hydration step.

[REFINEMENT T01: Two-layer writeback (mapping file + cached front matter projection)]

The mapping file (`.sync/jira-keys.yaml`) is the single sync-engine artifact. Front matter `jira_url` is a cached projection hydrated from it. This resolves the inline-vs-mapping tension by giving each layer its own concern: the mapping file serves tooling atomically; the front matter serves humans visually. Hydration is idempotent and conflict-free.

---

[PERSPECTIVE P02: depends_on should project as a single Jira link type with explicit override]

T03 asks who owns the dependency graph. My Round 0 position (two-pass sync) addressed sequencing but not semantics. The semantic gap is real: PM repo `depends_on` is a flat list; Jira links are typed and bidirectional.

My position: `depends_on` should project to exactly one Jira link type by default -- `Blocks` (the depended-on issue blocks the dependent). This is the most common intent when a PM author writes `depends_on: [VOT-002]`. If the author needs a different link type, they should be able to override per-edge:

```yaml
depends_on:
  - VOT-002                    # defaults to "blocks"
  - id: LOC-003
    link_type: relates-to      # explicit override
```

This keeps the common case simple (flat list works unchanged) while allowing Jira-native link semantics when needed. The sync engine maps link types to Jira's issue link type names. Unrecognized link types fail loudly at sync time.

Cupcake raised that Jira links are bidirectional -- `blocks` implies `is-blocked-by` on the other side. This is handled by Jira itself when you create one direction of a link. The sync engine creates the forward link; Jira materializes the inverse. No reconciliation needed on our side.

The two-pass sync (Round 0 consensus) handles the sequencing. This perspective handles the semantics.

[TENSION T03: Addressed -- default link type + per-edge override covers the semantic gap]

The flat `depends_on` list maps to `Blocks` by default. Per-edge overrides allow `relates-to` or other types. Jira handles bidirectional materialization. The remaining tension is whether `blue sync --check` should validate that the declared link types exist in the target Jira project's link type configuration before attempting creation.

---

[PERSPECTIVE P03: Read-back is validation, not sync -- and it belongs in the DocSource trait]

T05 asks what read-back looks like. Macaron and Croissant correctly identified that sprint lifecycle events (start, close, velocity) originate in Jira and cannot be declared from git. The Round 0 summary points toward "project then verify."

I agree with that direction and want to make it concrete. Read-back should be a method on the DocSource trait:

```rust
trait DocSource {
    fn discover(&self) -> Vec<SyncItem>;
    fn writeback(&self, mapping: &KeyMapping) -> Result<()>;
    fn read_back(&self, tracker: &dyn IssueTracker) -> Vec<DriftReport>;
}
```

`read_back` fetches current Jira state for all mapped issues and returns a `DriftReport` per item listing field-level differences. It does NOT write anything back to git. It is a pure read operation that feeds into:

1. **`blue sync --check`** (CI gate): Exits non-zero if drift exceeds policy threshold.
2. **`blue sync` (interactive)**: Prints drift warnings before projecting. User can `--force` to overwrite Jira or `--accept` to update git (the latter being a future bidirectional extension point, not implemented now).
3. **Dashboard**: Drift reports feed into status views showing which items are out of sync.

This keeps the system unidirectional (git projects to Jira) while acknowledging that Jira state drifts. The read-back is a sensor, not an actuator. Sprint close events in Jira surface as drift on sprint-assigned stories, prompting the PM author to update git accordingly -- manually, with full context, rather than through automated overwrite.

[REFINEMENT T05: Read-back as a DriftReport sensor on DocSource, not a sync direction]

Read-back is a `read_back() -> Vec<DriftReport>` method on the DocSource trait. It reads Jira state, compares against git-declared state, and reports differences without mutating either side. This preserves git authority while making Jira-side changes visible. The DriftReport feeds CI gates, interactive warnings, and dashboards.

---

**Claim**: The three refinements above (two-layer writeback, typed depends_on with defaults, read-back as DriftReport) resolve T01/T03/T05 without abandoning the git-authority principle. Each adds exactly one concept (mapping file, link_type field, DriftReport struct) rather than introducing modes or bidirectional sync complexity.
