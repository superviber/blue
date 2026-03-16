[PERSPECTIVE P01: The project-management repo must be the sync authority, not Jira]
Blue's value proposition is git-native workflow. If Jira becomes the source of truth for state transitions (RFC status, epic membership, task completion), teams will drift toward managing work in Jira's UI and Blue becomes a stale mirror. The external project-management repo should own the canonical mapping (RFC-to-Task, Feature-Release-to-Epic) and Blue should push state changes outward to Jira, never poll Jira to determine local state. Sync conflicts resolve in favor of the git repo, with Jira treated as a read-heavy projection.

[PERSPECTIVE P02: Credential storage must not assume a single Jira instance per machine]
Blue serves all users, not one org. A developer consulting for three companies needs three Jira Cloud instances with three API tokens. The credential model should be scoped per project-management repo (or per domain), not stored in a global ~/.jira-cli config. This is a design constraint that jira-cli's default config model does not naturally support, so Blue needs an explicit credential-routing layer.

[PERSPECTIVE P03: RFC-to-Task is clean, but Feature Release-to-Epic hides a cardinality trap]
Mapping RFCs to Jira Tasks is straightforward because both are single units of work. But a "Feature Release" grouping multiple RFCs across multiple repos is a Blue-side concept that may not align with how teams already use Epics. Some orgs use Epics per-team, per-quarter, or per-initiative -- not per-release. Blue should enforce that its Feature Release concept maps to exactly one Epic, but must surface a clear escape hatch (e.g., `jira_epic_key` override in the project-management repo manifest) for teams whose Epic semantics differ.

[TENSION T01: Push-only sync vs. bidirectional expectations]
If Blue treats Jira as a write-only projection, PMs who update Jira directly (status changes, re-prioritization) will create invisible drift that Blue never absorbs -- but adding bidirectional sync massively increases complexity and conflict surface.

[TENSION T02: jira-cli as a hard dependency vs. optional integration]
Mandating jira-cli installation for all Blue users couples Blue's install story to a third-party tool's release cadence and breaking changes; making it optional risks fragmented user experiences where some commands silently no-op.

---
