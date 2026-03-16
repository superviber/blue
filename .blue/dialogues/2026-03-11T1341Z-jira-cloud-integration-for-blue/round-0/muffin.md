[PERSPECTIVE P01: Jira as a projection, not a source of truth]
Blue already treats git as the source of truth for RFCs (files in `.blue/docs/rfcs/`), workflow state (the `RfcStatus` state machine in `workflow.rs`), and forge operations (the `Forge` trait abstracting GitHub/Forgejo). Jira should be modeled as a **projection layer** -- a one-way sync target that reflects Blue's canonical state -- not a bidirectional sync partner. The project-management repo should contain a declarative manifest (e.g., `domains/{domain}/repos.yaml` and `releases/{epic-key}.yaml`) that Blue reads to know which Jira project/epic an RFC belongs to, and Blue pushes status changes to Jira via `jira-cli`, but never pulls Jira state back to override local RFC status. This avoids the "two masters" problem that kills every bidirectional sync integration.

[PERSPECTIVE P02: Extend the Forge trait pattern to an IssueTracker trait]
Blue's `forge/mod.rs` already defines `ForgeType`, `CreatePrOpts`, and `MergeStrategy` behind a trait-like interface. The Jira integration should follow the same pattern: a new `tracker` module with an `IssueTracker` trait (`create_issue`, `transition_issue`, `link_issue`) and a `JiraCloudTracker` implementation that shells out to `jira-cli`. This keeps Jira swappable (Linear, Shortcut, etc.) and lets Blue enforce conventions -- like requiring every RFC to have an `epic_key` in its frontmatter before `blue rfc complete` succeeds -- without hardcoding Jira semantics into core.

[TENSION T01: Who owns status transitions when non-Blue users update Jira directly?]
If PMs or stakeholders move Jira tickets independently of Blue, the one-way projection model will drift, and we need to decide whether Blue ignores Jira state, warns on divergence, or refuses to proceed until reconciled.

---
