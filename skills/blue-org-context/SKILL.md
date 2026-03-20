---
name: blue-org-context
description: Show org context — repos, areas, Jira config from org.yaml and domain.yaml
---

# Org Context

Run `blue hook session-start` to get the current org context.

This skill outputs:
- Org name and PM repo location
- All repos (with clone status)
- Domain areas and component mappings
- Jira project configuration
- Where to create RFCs (org-wide vs repo-specific)

## Usage

This skill is automatically invoked by the session-start hook when working inside an org that has `org.yaml` at its root.

You can also invoke it manually: `blue hook session-start`
