[PERSPECTIVE P01: API token storage must never touch the project-management repo]
Jira API tokens and Atlassian credentials must be stored exclusively in a user-local credential store (OS keychain, encrypted dotfile, or environment variable) and never committed to the external project-management git repo. The project-management repo will be cloned across machines and potentially shared across team members; if Blue's setup guide or `blue init` flow even temporarily writes a `.jira-cli` config containing tokens into that repo's tree, a single `git add -A` destroys the security boundary. Blue should enforce a `.gitignore` entry for credential patterns in the project-management repo template and fail loudly if a token is detected in staged files during any `blue sync` or `blue pr` operation.

[PERSPECTIVE P02: Scoped API tokens and least-privilege Jira permissions]
Jira Cloud API tokens are account-scoped, not project-scoped -- a leaked token exposes every Jira project the user can access, not just the one Blue manages. Blue's setup guide should mandate creating a dedicated Atlassian "bot" account with permissions restricted to the specific Jira project, rather than reusing a developer's personal account. This limits blast radius if the token leaks and creates an auditable identity for automated transitions.

[PERSPECTIVE P03: The project-management repo as ground truth creates a split-brain risk with Jira as source of truth]
If the external git repo is "ground truth" but Jira Cloud is also authoritative for task state (assignees, status transitions, sprint membership), any sync lag or conflict resolution policy becomes a security-relevant decision -- whoever controls the merge strategy controls which state wins. Blue must define an explicit conflict resolution order (e.g., git repo wins for RFC metadata, Jira wins for workflow status) and log every reconciliation so teams can audit drift.

[TENSION T01: Account-scoped tokens vs. multi-repo domain management]
A single Jira API token grants access to all projects the account can see, but the project-management repo may track multiple repos under one domain -- Blue needs a strategy for whether one token governs all repos or whether per-project token isolation is enforced, and what happens when a user's Jira permissions change mid-lifecycle.

[TENSION T02: Git-as-ground-truth vs. Jira-as-ground-truth for state reconciliation]
No clear convention yet exists for which system wins when RFC status in the project-management repo diverges from the corresponding Jira Task status, and the wrong default could silently revert security-critical workflow gates (e.g., marking an RFC as approved in git when Jira still shows it blocked by review).
