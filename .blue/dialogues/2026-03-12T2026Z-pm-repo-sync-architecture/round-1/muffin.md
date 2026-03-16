[PERSPECTIVE P03: Resolve T01 with inline writeback of a single computed field, not a mapping file]
Scone's mapping file keeps authored artifacts pristine but forces every consumer (dashboards, CLI, drift detection, onboarding) to perform a join -- that is a tax paid on every read to avoid a cost paid once on write. Eclair is right that inline writeback is the onboarding contract. The compromise: write back exactly one computed field (`jira_url`) as the last entry in front matter, and treat it as machine-owned (never hand-edited). A `.gitattributes` merge strategy of `ours` on `jira_url:` lines eliminates Donut's merge conflict concern. This keeps authored fields pristine while the single writeback field is visibly segregated and mechanically conflict-free.

[PERSPECTIVE P04: DocSource trait shape must expose a writeback slot, not just a read surface]
Round 0 converged on DocSource yielding `SyncItem` streams, but the trait also needs a `fn writeback(&self, item: &SyncItem, remote_key: &str) -> WritebackOp` method that returns an enum (`InlineFrontMatter { field, value }`, `MappingFile { path, entry }`, `None`). This makes the writeback location a trait-level decision, not a global config -- Blue-repo DocSource returns `None` (writeback to rfc table), PM-repo DocSource returns `InlineFrontMatter`. The sync engine calls writeback uniformly without knowing which strategy is in play.

[PERSPECTIVE P05: T02 atomicity solved by treating writeback as a single atomic commit after full sync completes]
Writeback should never happen file-by-file during sync. The sync engine collects all WritebackOps into a batch, applies them to the working tree only after all Jira API calls succeed, then creates a single conventional commit (`blue-sync: writeback jira_url for 35 items`). If any API call fails, zero files are mutated. This is simpler than Donut's per-file lock model and eliminates partial-state entirely. Concurrency across meeples is handled by git itself: the writeback commit rebases cleanly because it only touches `jira_url` lines protected by `.gitattributes`.

[REFINEMENT: T05 read-back as a post-sync validation pass, not a sync phase]
Macaron and Croissant identified that sprint lifecycle events originate in Jira. The read-back should be a separate `blue sync --verify` pass that runs after projection, fetching current Jira state for all synced items and emitting warnings (not mutations) for drift. This keeps the sync engine unidirectional while giving CI a concrete gate. The DocSource trait does not need to model read-back -- it belongs in a separate `DriftDetector` component that takes `Vec<(SyncItem, JiraKey)>` and returns `Vec<DriftWarning>`.

[RESOLVED T01]
Inline single-field writeback (`jira_url`) with `.gitattributes` merge driver resolves the tension between onboarding DX and artifact purity. The mapping file alternative is unnecessary given that only one machine-owned field is written.

---
