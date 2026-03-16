[PERSPECTIVE P01: PM repos need credential isolation by domain, not by repo]
The current `CredentialStore` keys on Jira domain (`blue-jira` service + domain string), which means any user who clones the shared PM repo and runs `blue sync` will use their own keychain/env credentials transparently. This is correct -- but the PM repo's `.env.local` pattern introduces a fourth, repo-local tier that sits outside the existing three-tier hierarchy. If `.env.local` is treated as highest-priority (overriding even env vars), a malicious contributor could submit a PR adding `.env.local` to version control or renaming it to bypass `.gitignore`, hijacking the credential resolution for anyone who pulls. The lint system (`check_for_jira_credentials`) must be extended to run as a pre-sync gate on PM repos specifically, scanning not just committed content but also verifying that `.env.local` remains gitignored and that no tracked file matches credential patterns.

[PERSPECTIVE P02: jira.toml is a trust anchor that needs integrity verification]
Since `jira.toml` controls where credentials are sent (the `domain` field determines the Atlassian endpoint), a tampered `jira.toml` could redirect API tokens to an attacker-controlled domain. Before `blue sync` resolves credentials for a PM repo, it should verify that the domain in `jira.toml` matches the domain in `domain.yaml` and warn (or hard-fail under `drift_policy: deny`) on mismatch. This is the PM-repo equivalent of a supply-chain attack: the config file tells Blue where to send secrets.

[TENSION T01: .env.local creates an implicit fourth credential tier that contradicts the three-tier design]
The RFC 0065 hierarchy is env vars, keychain, TOML file -- all user-scoped and outside the repo. Introducing `.env.local` inside the repo boundary blurs the line between "repo config" and "user credentials," and the existing lint scanner has no awareness of this file's special status in PM repo mode.

---
