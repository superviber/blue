[PERSPECTIVE P01: .env.local must be eliminated as a credential source, not promoted to a fourth tier]

In Round 0 I flagged T06 -- `.env.local` as an implicit fourth credential tier outside the RFC 0065 three-tier hierarchy (env vars, keychain, TOML file). After reading the panel's responses, I am hardening this position rather than softening it. The problem is not just "the lint scanner does not know about .env.local." The problem is that `.env.local` exists inside the repo boundary, which means it participates in git operations (clone, checkout, worktree creation) in ways that user-scoped credential stores never do.

Concrete attack: a contributor opens a PR that modifies `.gitignore` to stop ignoring `.env.local`, or adds a symlink `.env.local -> /dev/stdin`, or introduces a build script that copies `.env.local` to a tracked location. The existing `check_for_jira_credentials` scanner looks for credential patterns in tracked files, but it does not verify the integrity of `.gitignore` rules themselves. In a PM repo where `.env.local` is the de facto credential source for local development, this is a supply-chain risk sitting inside the repo boundary.

The fix is clear: `.env.local` should be treated as a convenience alias that resolves to tier 1 (env vars) via a dotenv loader at CLI startup, NOT as a distinct credential source. `blue jira auth login` should store credentials in keychain (tier 2) or `~/.config/blue/jira-credentials.toml` (tier 3). The `.env.local` file should be documented as "for local env customization only, not for credentials," and `blue sync` should emit a warning if it detects `BLUE_JIRA_TOKEN_*` patterns inside any `.env*` file within the repo boundary.

[REFINEMENT: T06 resolution path]
T06 is resolvable without architectural debate. The answer: do not create a fourth tier. Instead: (a) `blue sync` pre-flight check verifies no `BLUE_JIRA_TOKEN_*` values exist in any repo-local dotenv file; (b) `blue jira setup` wizard never writes credentials to `.env.local`, always to keychain or `~/.config/blue/`; (c) document `.env.local` as for non-secret config only (e.g., `BLUE_JIRA_DOMAIN`, `BLUE_JIRA_PROJECT`). This keeps the three-tier model intact and closes the repo-boundary credential leak vector.

---

[PERSPECTIVE P02: jira.toml domain verification must be a mandatory pre-sync gate with pinning]

Expanding on T08 from Round 0. The threat model: jira.toml contains `domain = "attacker.atlassian.net"` while domain.yaml lists `superviber.atlassian.net`. A contributor who has valid credentials for `superviber.atlassian.net` runs `blue sync`, which resolves credentials for the domain in jira.toml -- if there is no cross-check, the sync happily sends authenticated requests to wherever jira.toml points.

After reading Eclair's first-sync safety gate proposal and Donut's cross-repo coordination model, I see this as even more critical than I initially stated. Donut proposes that `blue rfc create` in a code repo should clone/open the PM repo and push commits. If a tampered jira.toml in the PM repo is trusted without verification, the blast radius extends beyond one user's sync -- it poisons the cross-repo coordination pipeline.

The pre-sync gate should work as follows:
1. On first sync, `blue sync` reads `jira.toml` domain, verifies it matches a domain declared in `domain.yaml` (the org-level trust root from RFC 0067), and pins the verified domain into `.sync/domain-pin.sha256` (a hash of the domain string).
2. On subsequent syncs, verify the pin still matches. If jira.toml domain changes, hard-fail with a message requiring `blue sync --re-pin` with explicit user confirmation.
3. If `domain.yaml` is absent (pre-RFC-0067 setup), fall back to interactive confirmation: "jira.toml points to {domain}. Is this correct? [y/N]"

This is not optional. A `drift_policy` toggle is wrong here -- credential misdirection should always be a hard failure, not a configurable warning.

[TENSION T08: jira.toml trust anchor -- proposing resolution via domain pinning]
The original tension (tampered jira.toml redirecting API tokens) can be resolved by mandatory domain verification against domain.yaml + local pinning. The remaining design question is whether the pin file (`.sync/domain-pin.sha256`) should be tracked in git (so all contributors share the pin) or gitignored (so each contributor independently verifies). I lean toward tracked -- a PR changing the pin file is a high-visibility signal that the Jira target is changing, which is exactly the kind of change that should go through code review.

---

[PERSPECTIVE P03: T01 writeback location -- security perspective favors the mapping file]

Weighing in on T01 as requested. From a security standpoint, inline jira_key writeback into YAML front matter has a subtle risk: it normalizes machine-generated mutations to authored files, which makes it harder to distinguish legitimate writeback commits from malicious content injection. If an attacker can get a commit into the repo that looks like a writeback (matching the `blue-sync:` prefix Donut proposed), they can modify front matter fields beyond just jira_key.

A separate mapping file (`.sync/jira-keys.yaml` per Scone's proposal) has a clear security advantage: you can apply stricter validation rules to it. The mapping file has a known, rigid schema (id -> jira_key pairs, nothing else), so any content that deviates from that schema is immediately suspicious. Authored files, by contrast, have flexible schemas where additional fields might be legitimate extensions.

That said, I acknowledge Eclair's onboarding argument is strong -- `jira_url` in front matter is genuinely useful for discoverability. A hybrid is possible: the mapping file is the authoritative sync state (what `.sync/` manages), and a read-only `jira_url` field in front matter is generated from it as a convenience, clearly marked as machine-managed (e.g., a comment `# managed by blue sync -- do not edit`). This gives Eclair the DX signal without making authored files the writeback target.

[CONCESSION: T01]
I concede that pure mapping-file-only is worse for DX than Eclair's inline approach. But I maintain that the mapping file should be the source of truth for sync state, with front matter `jira_url` as a derived, clearly-marked convenience field. The key constraint from my perspective: writeback commits must touch only well-defined, schema-validated locations, never free-form authored content.

---
