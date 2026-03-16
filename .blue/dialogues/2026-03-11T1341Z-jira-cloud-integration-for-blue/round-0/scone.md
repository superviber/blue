[PERSPECTIVE P01: The project-management repo must enforce a declared mapping schema, not just store state]
The external project-management repo is only useful as ground truth if it declares the *shape* of the mapping (e.g., "RFC maps to Jira Task, Feature Release maps to Epic, domain owns repos X/Y/Z") in a machine-readable schema file, not just as accumulated sync artifacts. Without a schema contract, different Blue installations will drift into incompatible conventions, and the repo becomes a graveyard of stale YAML instead of an authoritative source. Blue should validate every sync operation against this schema and reject writes that violate it, the same way `blue lint` enforces RFC structure today.

[PERSPECTIVE P02: Jira API token lifecycle is a multi-user governance problem, not a setup step]
Treating API token configuration as a one-time "getting started" guide underestimates the operational reality: tokens expire, get rotated by Atlassian admins, and must be scoped per-user rather than per-repo. Blue should store tokens in a platform keychain (macOS Keychain, Linux secret-service) with an explicit expiry check on every Jira operation, and surface a `blue jira auth status` command that reports token health. Embedding tokens in dotfiles or environment variables will inevitably lead to tokens committed to git or shared across users in CI.

[TENSION T01: Ground truth conflict resolution between Jira and the project-management repo]
When Jira state and the project-management repo diverge (e.g., a PM moves a ticket in Jira while a developer updates the RFC locally), which source wins, and does Blue enforce last-write-wins, manual merge, or a declared hierarchy of authority?

---
