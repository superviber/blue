[PERSPECTIVE P01: The project-management repo must be a declarative manifest, not a sync mirror]
The external project-management repo should contain declarative YAML manifests (one per domain) that declare the canonical mapping: which repos belong to which domain, which Epics exist, and which RFC-to-Task bindings are active. Blue should reconcile Jira state against this manifest on `blue sync`, treating the repo as ground truth and Jira as a projection. This avoids the trap of bidirectional sync (which invariably drifts) and gives teams a reviewable, git-blamed audit trail for every structural change to their Jira topology. The manifest format must be stable across Blue versions since multiple repos under a domain will depend on it.

[PERSPECTIVE P02: RFC-to-Task binding must be repo-local, not centralized in the project-management repo]
The mapping of a specific RFC to its Jira Task key (e.g., `jira: PROJ-142`) should live inside the RFC front matter within each repo, not solely in the central manifest. The project-management repo should only declare the Epic-level structure and repo membership. This keeps the RFC self-describing (you can read it without consulting another repo) and avoids merge conflicts when multiple repos push RFC bindings to a shared manifest simultaneously. The central repo then discovers these bindings by scanning member repos during reconciliation, not by being the write target.

[PERSPECTIVE P03: Jira CLI credential storage needs a trust-boundary-aware convention]
Bundling ankitpokhrel/jira-cli means Blue inherits its credential model (~/.config/.jira/.config.yml with API token in plaintext). Blue should enforce that credentials are never committed to any repo (including the project-management repo) by adding a `.gitignore` convention and a `blue lint` check. More importantly, the guide should distinguish between personal API tokens (user-scoped, stored locally) and CI service account tokens (org-scoped, stored in CI secrets), because the project-management repo reconciliation will eventually need to run in CI, and mixing these trust boundaries is a common source of token sprawl.

[TENSION T01: Declarative manifest vs. Jira as source of truth for Epic creation]
If the project-management repo is ground truth, who creates Epics first -- a human in Jira or a PR to the manifest? Both workflows have valid use cases, and the reconciliation direction (repo-to-Jira vs. Jira-to-repo) must be explicitly chosen per artifact type, or teams will fight the tool.

[TENSION T02: Multi-domain isolation in a single Jira instance]
Multiple Blue domains (each with their own project-management repo) may target the same Jira Cloud instance. Without a namespace convention (e.g., Jira project key per domain), label and Epic collisions are inevitable, and Blue has no mechanism today to enforce Jira-side isolation.

---
