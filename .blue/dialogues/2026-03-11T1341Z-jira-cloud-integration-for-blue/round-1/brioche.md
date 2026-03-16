[PERSPECTIVE P01: Token storage must be domain-keyed in OS keychain with explicit health checks]

Round 0 surfaced broad agreement that tokens must never enter git (Strudel, Macaron, myself). Round 1 must resolve the *how*. I align with Croissant's multi-domain keying proposal and Scone's `blue jira auth status` command. The concrete design: Blue stores credentials in the OS keychain (macOS Keychain, Linux secret-service, Windows Credential Manager) under a composite key of `blue:jira:{domain}`, where domain comes from the project-management repo's `jira.toml`. Each entry holds the API token and associated email. On every Jira-touching operation, Blue checks token validity with a lightweight `/rest/api/3/myself` call (cached for 15 minutes). If the token is expired or revoked, Blue refuses to sync and surfaces a clear remediation path (`blue jira auth refresh --domain myorg.atlassian.net`). This addresses Cannoli's multi-instance concern, Cupcake's rotation lifecycle concern, and my own Round 0 stance on blast radius.

[REFINEMENT on T03 — Token storage, scoping, and rotation]

I refine my Round 0 position: I initially said "OS keychain or env vars." I now concede env vars should be CI-only, never the recommended local path. Env vars are too easily leaked via shell history, process listings, and child process inheritance. For local development: OS keychain exclusively. For CI: secrets manager injection into env vars, with Blue detecting the CI context via `$CI` or `$GITHUB_ACTIONS` and adjusting its lookup strategy. This addresses Macaron's trust-boundary concern between personal and CI tokens.

[PERSPECTIVE P02: Bot accounts are mandatory for sync operations, personal tokens for read-only drift detection]

Expanding my Round 0 P02 on least-privilege. The sync path (`blue sync --jira`) that mutates Jira state must use a dedicated bot account, not personal tokens. Reasons: (1) audit trail shows "blue-bot" made the change, not a developer who happened to run sync; (2) bot accounts can be scoped to a single Jira project via Atlassian admin, limiting blast radius; (3) personal token revocation (employee departure) does not break CI sync. Personal tokens should only be used for `blue jira auth status` and drift-detection reads. This creates a two-tier credential model: bot-write, human-read.

[TENSION T01-RESPONSE: Drift detection with security-aware reconciliation policy]

Strudel, Muffin, Cannoli, and Eclair all converge on git-wins-always. I agree for structural state (RFC metadata, Epic membership). But I raise a security nuance: Jira workflow gates (e.g., "Blocked by Security Review") represent organizational controls that may not exist in the git repo's state machine. If Blue overwrites a Jira-side security block because the git repo says "approved," you have a control bypass. Proposal: Blue's sync should distinguish between *projection fields* (title, description, labels -- git always wins, overwrite freely) and *gate fields* (status transitions involving security/compliance workflows). For gate fields, Blue should refuse to overwrite Jira state that is more restrictive than git state, log the discrepancy, and require explicit `--force-transition` to override. This resolves T01 for the security dimension without blocking the general git-wins model.

[RESOLVED on T02 — jira-cli dependency model]

I align with Cupcake, Beignet, and Muffin's convergence toward a trait/adapter pattern. From a security perspective, this is correct: Blue should not inherit jira-cli's auth surface. The `IssueTracker` trait means Blue controls the credential flow and can enforce keychain-only storage regardless of which backend is used. If a team swaps to Linear or Shortcut, the same credential hygiene applies through the trait contract. I consider T02 resolvable with: trait interface in Blue core, jira-cli as default adapter, graceful degradation when absent.

[PERSPECTIVE P03: Sync audit log is a non-negotiable security artifact]

Every `blue sync --jira` invocation must produce an append-only audit record in the project-management repo (e.g., `.blue/sync-log/{timestamp}.yaml`) containing: which fields were pushed, which were skipped due to gate-field protection, the bot identity used, and a hash of the pre/post Jira state. This is not a nice-to-have; it is the mechanism by which teams verify that the projection has not silently drifted. Without it, the git-as-ground-truth guarantee is unverifiable. The sync log is committed to the project-management repo automatically by the sync operation, providing git-blame attribution and tamper evidence.

[TENSION T-NEW: Bot account provisioning is an adoption barrier]

My own bot-account proposal (P02 above) creates a tension: requiring a dedicated Atlassian bot account adds an admin setup step that individual developers and small teams may not have permission to perform. Atlassian Cloud admin rights are often held by IT, not engineering. Blue must provide a graceful degradation path where personal tokens work for all operations (with a lint warning), and bot accounts are an opt-in hardening step documented as a best practice. Otherwise we gate adoption on organizational bureaucracy.

---
