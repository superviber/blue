# Round 2 — Muffin (Blue Platform Architect)

## [MOVE:CONVERGE]

The architecture is ready for RFC drafting. All 7 structural tensions are resolved with clean, composable designs. I am satisfied with the outcomes.

### Confirmation of Key Architectural Decisions

As the platform architect, I want to explicitly endorse the load-bearing decisions:

1. **IssueTracker trait with out-of-process adapter (T02)** — This is the right call. Blue stays decoupled from any single tracker. The PATH-discoverable `blue-jira-provider` binary pattern mirrors how Git discovers credential helpers and merge drivers. It keeps Blue's core dependency graph clean and makes Linear/Shortcut adapters a matter of implementing the same CLI contract, not touching Blue internals.

2. **Three-tier field ownership (T01)** — The structural/operational/descriptive split with configurable drift policy avoids the "who wins" debates that plague every bidirectional sync system. The `sync_hash` drift detection for descriptive fields is pragmatic — warn without overwriting is the right default.

3. **Repo-local RFC-to-Task bindings (T04)** — Keeping the binding in RFC front matter eliminates the cross-repo write contention problem entirely. This is the architectural decision I care most about — it means teams can work independently without coordination overhead.

4. **Per-entity YAML files in PM repo (T04/T05)** — One file per Epic means merge conflicts on PM repo changes are structurally impossible for common operations. This scales.

### On the Remaining Refinement Tensions

T08 through T11 are implementation decisions, not architectural ones. My position on each for the RFC:

- **T08 (Drift detection frequency)**: Default to event-driven sync on `blue sync` invocation plus optional cron. Document staleness window as a team-configurable parameter, not a platform-enforced one. This is a configuration knob, not architecture.

- **T09 (Error ergonomics when tracker absent)**: RFC creation must always succeed locally. Surface a non-blocking warning: "Jira sync configured but provider not found — issue will be created on next sync with provider available." Blue's offline-first contract is non-negotiable.

- **T10 (Credential storage)**: Ship v1 with all three tiers (env var, keychain, TOML fallback). The TOML fallback has clear security warnings in docs and `blue lint` catches tokens in git. Removing the fallback gates adoption on OS keychain support across all platforms, which is unnecessary friction.

- **T11 (Bot account provisioning)**: Document bot accounts as recommended, not required. Personal tokens work for single-developer setups. The RFC should include a "getting started" path that uses personal tokens and a "production" path that uses bot accounts.

These positions can be captured as design decisions in the RFC body without further dialogue rounds.

### Platform Architecture Confidence

The adapter pattern, field ownership model, and repo-local binding strategy form a coherent platform architecture. The design respects Blue's core principles: git-authoritative, offline-first, progressive complexity. Nothing in the remaining tensions threatens these invariants.

I signal **[MOVE:CONVERGE]**. Draft the RFC.
