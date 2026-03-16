[PERSPECTIVE P01: Sprint and release planning need bidirectional timestamps, not just unidirectional projection]
The notes say "strictly git → Jira," but sprint ceremonies have temporal semantics that markdown files alone cannot enforce. When s03.md says sprint 3 starts March 18, nothing in the PM repo validates that against the active Jira sprint's actual start/end dates or prevents overlapping sprint assignments. The sync engine needs at minimum a read-back validation step — not full bidirectional sync, but a "project then verify" loop that warns when the Jira sprint state contradicts git's declared reality (e.g., a story assigned to s03 in git but the Jira sprint is already closed). Without this, the "single source of truth" claim breaks the moment someone closes a sprint in Jira's UI.

[PERSPECTIVE P02: The PM repo should own the dependency graph, not just depend_on links]
The `depends_on` field projects to Jira issue links, but cross-epic dependency chains (PTY-001 depends on VOT-002 which depends on LOC-002) form a DAG that has sprint-planning implications. The sync engine should validate this DAG before projection — flagging when a story is assigned to an earlier sprint than its dependency. This is where git-as-authority actually adds value over Jira: you can run `blue sync --check` as a CI gate that rejects PRs introducing impossible sprint orderings.

[TENSION T01: Strict unidirectional sync vs. the reality that Jira sprint lifecycle events originate in Jira]
Sprint start, sprint close, and velocity tracking are Jira-native operations that have no git-side equivalent — the PM repo can declare intent but cannot be authoritative over these runtime events without at least a read-back mechanism.

---
