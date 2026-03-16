[PERSPECTIVE P01: Blue should not own Jira state]
Blue's current config.yaml carries forge metadata (GitHub host, owner, repo) but no project-management layer. Adding Jira sync means Blue now has two sources of truth for task status: its local SQLite DB and Jira Cloud. The external "project-management" repo should be the sole mapping authority (RFC-id to Jira-issue-key), and Blue should treat Jira as a write-through cache it pushes to, never reads back from as canonical. If Blue ever reconciles conflicts between its local RFC status and Jira status, you have built a distributed consensus problem into a CLI tool.

[PERSPECTIVE P02: Bundling jira-cli is a liability, not a feature]
Pinning ankitpokhrel/jira-cli as an install dependency couples Blue's release cycle to a third-party binary with its own auth model, breaking-change cadence, and platform matrix. Blue should instead shell out to `jira` if present and degrade gracefully if absent, the same way git-based tools treat `gh`. Enforcing installation invites support burden for a tool Blue doesn't control.

[TENSION T01: Ground-truth repo creates a coordination bottleneck across multiple service repos]
If the project-management repo is the single mapping file between RFCs and Jira artifacts, every repo that creates an RFC must commit or PR into that external repo, introducing a cross-repo write dependency that will serialize otherwise-parallel work.

---
