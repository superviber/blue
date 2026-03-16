[PERSPECTIVE P01: The project-management repo must be the sole source of truth, with Jira as a derived projection]
If both Jira Cloud and the project-management git repo can be independently mutated, you get split-brain divergence that no reconciliation algorithm will reliably heal. The git repo should own the canonical state (RFC-to-Epic mappings, task status, feature release membership), and Blue should treat Jira as a write-through cache -- every mutation flows git-first, then syncs outward to Jira via idempotent push. Jira-side edits should either be blocked by workflow restrictions or detected and flagged as drift during `blue sync`, never silently accepted as authoritative.

[PERSPECTIVE P02: RFC-to-Jira identity binding requires a stable, immutable join key stored in both systems]
Mapping RFCs to Jira Tasks demands a durable identifier that survives Jira key renumbering (project moves), RFC renames, and repo transfers. Blue should mint a UUID at RFC creation time, store it in RFC front matter, and write it into a Jira custom field. All sync operations join on this UUID, never on Jira issue key or RFC filename, both of which are mutable. Without this, any project restructuring in Jira silently severs the link and corrupts the mapping.

[PERSPECTIVE P03: API token storage must never touch the project-management repo]
Credentials (Atlassian API tokens, domain URLs) belong in a user-local credential store (OS keychain or `~/.config/blue/credentials`), never in the shared project-management repo. The repo should contain only a declarative `jira.toml` or similar that declares the Atlassian domain and project key -- enough for Blue to know *where* to sync, but never *how* to authenticate. Mixing auth material into the ground-truth repo poisons it for every collaborator who clones it.

[TENSION T01: Jira-side edits vs. git-first authority]
If Blue enforces git-as-source-of-truth, non-engineering stakeholders who only use Jira's web UI become second-class participants, creating organizational friction that could block adoption.

[TENSION T02: Multi-repo fan-out complicates atomic consistency]
A single project-management repo tracking multiple code repos under one domain means a Feature Release (Epic) can span repos, but git commits are per-repo -- achieving atomic cross-repo state transitions without a coordination protocol risks partial sync states.

---
