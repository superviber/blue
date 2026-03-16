[PERSPECTIVE P01: jira_key writeback creates a bidirectional coupling that contradicts one-way sync]
Writing `jira_key: SCRUM-42` back into YAML front matter means every sync mutates the source repo, generating commits that must be handled (auto-commit? staged for user?). This is not a cosmetic detail -- it turns a "strictly git to Jira" pipeline into a system where sync itself is a git actor. The writeback should target a separate mapping file (e.g., `.sync/jira-keys.yaml`) rather than inlining into story front matter, keeping the authored artifacts pristine and the sync-generated state isolated. This also prevents merge conflicts when multiple contributors sync concurrently.

[PERSPECTIVE P02: depends_on resolution demands a two-pass sync with deterministic ordering]
`depends_on` references like `VOT-002` point to PM-repo IDs, not Jira keys. You cannot create an issue link in Jira until both sides exist. The sync engine must therefore do a full creation pass first (all epics, then all stories), collect the id-to-jira_key mapping, then execute a second pass to create issue links. If this ordering is not explicit in the architecture, partial failures will leave orphaned or unlinked issues with no clean retry path.

[TENSION T01: mapping file vs inline jira_key affects every downstream read path]
If jira_key moves to a separate mapping file, every tool that needs to correlate a PM doc to its Jira issue (dashboards, CLI status commands, drift detection) must join across two files -- adding complexity that inline front matter avoids.

---
