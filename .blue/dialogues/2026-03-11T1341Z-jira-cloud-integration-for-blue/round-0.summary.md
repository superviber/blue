# Round 0 Summary — Judge Synthesis

## Strong Convergence

**Git as sole authority, Jira as projection**: Near-unanimous agreement (11/12 experts) that the project-management git repo must be the single source of truth. Jira is a write-through projection, never a bidirectional sync partner. This aligns with Blue's ADR 0005 (Single Source) and avoids the "two masters" problem.

**API tokens must never enter git**: Brioche, Strudel, Macaron, Croissant all independently flagged this. Credentials stay in OS keychain / env vars. PM repo template must ship with `.gitignore` patterns and `blue lint` checks.

**Idempotent, unidirectional sync**: Donut, Eclair, Muffin converge on Blue pushing state to Jira idempotently. No pull-back. Offline-first design so RFC creation doesn't require network.

## Novel Contributions

- **Muffin**: IssueTracker trait (mirrors existing Forge pattern) — keeps Jira swappable for Linear/Shortcut
- **Macaron**: RFC-to-Task binding lives in RFC front matter (repo-local), not centralized PM repo. PM repo only declares Epic structure and domain membership. This solves merge conflict risk.
- **Strudel**: Immutable UUID join key minted at RFC creation — survives Jira ticket renames/moves
- **Churro**: `blue jira import` bootstrapping command — critical adoption path for existing teams
- **Brioche**: Dedicated bot account with least-privilege scoping, not personal tokens
- **Cupcake**: jira-cli as recommended adapter behind provider interface, not hard dependency

## Open Tensions (7)

1. Sync direction policy when PMs edit Jira directly
2. jira-cli dependency model (bundle vs adapter vs trait)
3. Token storage/scoping/rotation lifecycle
4. Multi-repo fan-out atomic consistency
5. Epic cardinality across repos
6. Bootstrapping existing Jira state
7. Convention enforcement progressiveness

## Velocity

Open tensions: 7 | New perspectives: 28 | **Velocity: 35** | Converge: 0%

Round 1 should focus on resolving tensions T01-T03 (the highest-impact architectural decisions) and refining the concrete artifact mapping.
