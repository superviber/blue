[PERSPECTIVE P01: Jira CLI as a vendored dependency is a maintenance trap]
Bundling ankitpokhrel/jira-cli into Blue's install couples Blue's release cadence to a third-party binary's breaking changes and Atlassian API deprecations. Blue should instead define a thin adapter interface (e.g., `JiraProvider`) that shells out to jira-cli if present but can be swapped for direct REST calls or a future official Atlassian CLI. This makes jira-cli a recommended-not-required dependency, tested in CI via a mock provider, and avoids Blue owning someone else's auth surface.

[PERSPECTIVE P02: The project-management repo needs a conflict-free merge contract]
If the external project-management repo is ground truth and multiple repos push RFC-to-Jira state into it, concurrent PRs will produce merge conflicts on shared manifest files (e.g., epics.yaml). Blue must enforce append-only, per-entity files (one YAML per RFC/Epic) rather than a single registry file, or use a CRDT-like last-write-wins timestamp strategy, so that cross-repo automation never blocks on human conflict resolution.

[TENSION T01: Auth token storage location and rotation lifecycle]
Blue must decide whether Jira API tokens live in the project-management repo (encrypted, shared), in per-user dotfiles (~/.blue/credentials), or are delegated entirely to jira-cli's own config -- each choice has different security, onboarding, and rotation trade-offs that affect every downstream design decision.

---
