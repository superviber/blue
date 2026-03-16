[PERSPECTIVE P01: Jira writeback must be a dedicated commit type to avoid merge conflicts]
When `blue sync` writes `jira_key: SCRUM-42` back into YAML front matter, that commit must be machine-authored, atomic per file, and use a conventional prefix (e.g., `blue-sync: writeback SCRUM-42 to PTY-001`) so it can be auto-rebased or squashed without human intervention. If multiple meeples or developers run `blue sync` concurrently against the same PM repo, writeback commits touching different files will rebase cleanly, but writeback to the same file will conflict. The solution is a lock-per-file or queue model at the sync layer: `blue sync` should acquire a lightweight advisory lock (branch-based or file-based) on the PM repo before committing writeback, ensuring serialized writes per story file.

[PERSPECTIVE P02: Cross-repo coordination should use the PM repo as the rendezvous point, not out-of-band state]
When `blue rfc create` in a code repo needs to create a corresponding artifact in the PM repo, Blue should clone/open the PM repo locally (resolved via RFC 0067's org layout + domain.yaml repo list), commit the new story file, and push -- all within the same `blue rfc create` invocation. Storing cross-repo coordination state in `.blue/jira/last-sync.yaml` inside each code repo creates a second source of truth that can drift. The PM repo's git log itself should be the only record of what has been projected; the `blue_uuid` in each story file is the join key, and the PM repo commit history is the audit trail.

[TENSION T01: Serialized writeback vs. multi-meeple concurrency]
The one-way git-to-Jira projection model assumes a single writer to the PM repo, but the crew/meeple model explicitly enables parallel work -- meaning multiple sessions may attempt writeback to the PM repo simultaneously, and no lock mechanism has been specified.

---
