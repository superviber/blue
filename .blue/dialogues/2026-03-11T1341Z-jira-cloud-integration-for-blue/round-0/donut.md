[PERSPECTIVE P01: RFC-to-Jira sync must be idempotent and direction-explicit]
The most dangerous design flaw in CLI-to-Jira integrations is ambiguous sync direction: does Blue push state to Jira, does Jira pull from Blue, or is it bidirectional? Blue should enforce a single canonical direction -- Blue-to-Jira push -- where `blue rfc create` and `blue rfc complete` emit Jira mutations, but Jira field changes never flow back into Blue's local state. Bidirectional sync across a git repo and a SaaS API invites ghost conflicts where local YAML says "in-progress" but Jira says "done," and no one knows which is authoritative. Every sync operation must be idempotent: running `blue sync --jira` twice produces the same Jira state, with no duplicate tickets or status thrash.

[PERSPECTIVE P02: Feature Release as Epic mapping needs a lifecycle gate, not just a label]
Mapping a Feature Release to a Jira Epic is natural, but Blue must enforce that an Epic cannot be created in Jira until at least one RFC is linked to it, and an Epic cannot transition to "Done" until all child RFCs report complete in the project-management repo. Without these gates, Epics become orphaned placeholders or are prematurely closed, defeating the purpose of ground-truth tracking.

[TENSION T01: Multi-repo fan-out coordination]
When multiple repos under one domain each push RFC state into the shared project-management repo, there is no defined ordering or locking mechanism to prevent a stale-read race where repo A's push overwrites repo B's concurrent push to the same Epic manifest.

---
