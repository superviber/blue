[PERSPECTIVE P01: T01 resolves with a three-tier drift policy, not a binary choice]
The git-first vs. Jira-side-edits tension (T01) keeps getting framed as "overwrite or accept." Neither works. Blue should implement three tiers: (1) **structural fields** (Epic membership, RFC-to-Task binding, status) are git-authoritative and overwritten on `blue sync` with a logged warning; (2) **operational fields** (assignee, sprint, priority) are Jira-authoritative and never pulled back -- Blue does not model these at all; (3) **descriptive fields** (summary, description) use last-write-wins with a `sync_hash` in RFC front matter so Blue can detect Jira-side edits and surface them as PR comments on the next sync, not silently clobber them. This gives PMs a real lane for the fields they care about while keeping architectural state git-owned.

[PERSPECTIVE P02: The IssueTracker trait must be a process boundary, not a function interface]
In Round 0 I proposed an IssueTracker trait mirroring the Forge pattern. Cupcake and Beignet rightly flagged that jira-cli coupling is fragile. I now refine this: the trait should define a **command contract** (input DTOs in, exit-code + JSON out) rather than Rust function signatures, so that implementations can be out-of-process adapters -- a shell script wrapping jira-cli, a compiled binary calling REST directly, or a mock for CI. Blue invokes the adapter via `blue-jira-provider` (a PATH-discoverable binary), never links against jira-cli. This makes T02 moot: jira-cli is neither bundled nor required, it is one possible backing implementation behind a stable CLI contract.

[REFINEMENT: Macaron's repo-local binding + Strudel's UUID join key compose cleanly]
Macaron's insight that RFC-to-Task binding belongs in RFC front matter (not the central PM repo) and Strudel's immutable UUID join key are complementary, not competing. The RFC front matter carries `blue_id: <uuid>` (minted at creation) and `jira_task: PROJ-142` (written on first sync). The PM repo only declares Epic structure and domain membership. All sync joins on `blue_id`, and `jira_task` is a denormalized convenience that `blue sync` can re-derive if Jira keys shift. This also addresses Donut's concern about cross-repo write serialization (T04): individual repos never write to the PM repo for RFC-level bindings.

[RESOLVED T02: jira-cli dependency model]
The out-of-process adapter pattern resolves T02. Blue defines a provider contract; jira-cli is a recommended adapter, not a dependency. No bundling, no coupling.

[RESOLVED T04: Multi-repo fan-out atomic consistency]
Repo-local RFC front matter for Task bindings eliminates cross-repo writes to the PM repo for RFC-level state. The PM repo only receives Epic-level structural changes, which are infrequent and human-authored via PR -- no automation race condition.

---
