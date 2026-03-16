[PERSPECTIVE P01: Extract a DocSource trait, not a second sync mode]
The notes propose "two modes" (Blue repo mode vs PM repo mode), but the actual divergence is narrower than that: it is discovery + parsing + writeback, not sync logic. The right abstraction is a `DocSource` trait (or `SyncableArtifact` provider) that yields a uniform stream of `SyncItem { id, title, status, issue_type, labels, parent, ... }` from any repo shape. The sync engine (`run_sync`) stays singular and mode-unaware. This avoids the classic "two modes that slowly diverge" trap and makes a third source shape (e.g., Linear-native YAML, or a monorepo with mixed RFC + PM dirs) a trait impl rather than a feature flag.

[PERSPECTIVE P02: Jira key writeback creates a git-commit-on-sync side effect that needs explicit policy]
Writing `jira_key: SCRUM-42` back into YAML front matter means `blue sync` is no longer a read-then-push operation -- it mutates the working tree and likely needs to auto-commit. This is a meaningful change in the contract of `sync`: it goes from "project to Jira" to "project to Jira, then mutate local state." The writeback policy (auto-commit, dirty-tree-with-marker, or deferred-to-next-commit) should be an explicit `writeback_policy` field in `jira.toml`, not an implicit behavior, because PM repos may have CI pipelines or branch protections that a surprise commit would break.

[TENSION T01: Who owns the `depends_on` graph -- the PM repo or Jira link types?]
The notes list `depends_on` as git-to-Jira, but Jira issue links are typed (blocks/is-blocked-by/relates-to) and bidirectional, while the PM YAML `depends_on` is a flat unidirectional list -- reconciling these semantics in a one-way sync without data loss or misrepresentation is unresolved.

---
