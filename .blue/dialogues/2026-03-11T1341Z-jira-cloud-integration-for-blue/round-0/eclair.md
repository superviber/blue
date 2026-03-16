[PERSPECTIVE P01: RFC-to-Jira sync must be idempotent and offline-first]
Developers work on planes, in coffee shops, and behind VPNs that block *.atlassian.net. Blue must record RFC-to-Jira intent locally (e.g., a `.blue/jira/pending-sync.yaml` queue) and reconcile on next connectivity, rather than requiring live Jira access at RFC creation time. Every sync operation must be idempotent -- running `blue sync` twice with no local changes should produce zero Jira API calls and zero state drift. This is the only pattern that respects ADR 0005 (single source = local git) while treating Jira as a projection.

[PERSPECTIVE P02: Convention enforcement needs a schema contract, not just docs]
Blue should ship a machine-readable schema (JSON Schema or similar) that defines the valid RFC-to-Jira field mappings, required labels, Epic naming conventions, and allowed status transitions. CLI validation at `blue rfc create` time catches convention violations before they reach Jira, and the schema itself lives in the project-management repo so conventions evolve via PR review rather than tribal knowledge.

[TENSION T01: Who owns the canonical state -- git or Jira?]
If the project-management repo is ground truth per ADR 0005, then Jira becomes a read-only projection, but PMs and stakeholders will edit Jira fields directly, creating inevitable drift that Blue must either detect-and-warn or overwrite-and-lose.

---
