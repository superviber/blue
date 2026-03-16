[PERSPECTIVE P01: Zero-config must mean zero-surprise]
Auto-detecting PM repo mode via `jira.toml` presence is the right call, but the first run must print exactly what it detected and what it will do before touching Jira. Something like: `Detected PM repo (jira.toml) -> themovesocial.atlassian.net / SCRUM. Found 9 epics, 35 stories. Run with --dry-run to preview, or --confirm to sync.` The danger is not missing config -- it is a user who clones a PM repo, runs `blue sync` out of curiosity, and accidentally creates 35 Jira tickets. First-sync should require an explicit `--confirm` or interactive yes/no; subsequent syncs (where `jira_key` writeback already exists) can be implicit. This is the difference between "zero config" and "zero friction" -- they are not the same, and conflating them is how you lose trust on day one.

[PERSPECTIVE P02: Writeback is the onboarding contract]
The `jira_key: SCRUM-42` writeback into YAML front matter is not just a sync mechanism -- it is the primary onboarding signal that tells a new contributor "this file is connected to something real." If a teammate opens `PTY-001-create-party.md` and sees no `jira_key`, they know it has never been synced. If they see one, they can click through. This means writeback format and placement should be treated as a first-class DX surface: always the last field in front matter, always a clickable URL (not just a key), e.g., `jira_url: https://themovesocial.atlassian.net/browse/SCRUM-42`.

[TENSION T01: First-sync safety vs zero-config promise]
The marketing promise of "just run `blue sync`" conflicts with the operational reality that creating dozens of Jira issues is an irreversible side effect that demands explicit user intent on first run.

---
