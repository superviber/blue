[PERSPECTIVE P01: Adoption will fail without a migration path for existing Jira workflows]
Most teams adopting Blue will already have active Jira boards with in-flight Epics, sprints, and custom workflows. If Blue demands a clean-slate project-management repo that becomes ground truth, teams must either abandon existing Jira state or manually backfill it -- both are adoption killers. Blue needs a `blue jira import` command that bootstraps the project-management repo from an existing Jira project's current Epics and Tasks, creating RFC stubs linked to existing issue keys. Without this on-ramp, only greenfield teams will adopt, and the integration becomes niche.

[PERSPECTIVE P02: Convention enforcement must be progressive, not gatekeeping]
Mandating strict conventions (Epic naming, status transitions, label taxonomies) from day one will generate friction that causes teams to bypass Blue and edit Jira directly -- exactly the drift everyone is trying to prevent. Blue should enforce conventions in tiers: warn-only mode during onboarding (first 30 days or first N syncs), then strict mode once the team explicitly opts in via a config flag. This mirrors how linters adopt `--fix` vs `--strict` and respects that convention buy-in is a social process, not a technical switch.

[TENSION T01: Bootstrapping vs. clean-slate assumptions]
Every expert so far assumes Blue creates Jira artifacts outward, but no one has addressed how Blue handles the far more common case: a team with years of existing Jira state that wants to layer Blue on top without disrupting their current workflow.

---
