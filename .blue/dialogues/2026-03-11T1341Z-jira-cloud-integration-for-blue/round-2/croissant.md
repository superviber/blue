# Round 2 -- Croissant (Convention & Standards Designer)

## Convergence Assessment

All 7 original tensions (T01-T07) are resolved with concrete, composable proposals. The architecture has stabilized around clear conventions: three-tier field ownership, IssueTracker trait contract, domain-keyed credential hierarchy, per-entity file structure, progressive enforcement tiers, and a two-phase import bootstrap. My Round 1 contributions on drift policies, credential hierarchy, and enforcement tiers were incorporated into the final resolutions without material alteration.

I review the 4 remaining refinement tensions (T08-T11) from a convention-design perspective and find them resolvable within the RFC without further deliberation.

---

### T08: Drift detection frequency and staleness window

This is a configuration knob, not an architectural question. The convention answer: declare `sync_interval` in the PM repo's `jira.toml` (default: manual invocation only for v1). Teams that want periodic sync configure CI to run `blue sync` on a cron. Staleness is observable via the `last_synced_at` timestamp in each entity's sync metadata file. No new tension here -- the three-tier drift policy from T01 already defines what happens when staleness is detected.

### T09: Error ergonomics when jira-cli is absent

The IssueTracker trait resolution (T02) already specifies graceful degradation. Convention: `blue rfc create` succeeds locally regardless of tracker availability. If `tracker` is configured in `jira.toml` but the provider binary is not on PATH, Blue emits a warning to stderr and writes the RFC with a `jira.pending_sync: true` flag in front matter. Next `blue sync` picks it up. No blocking required.

### T10: Credential storage mechanism (TOML fallback vs keychain-only)

The three-tier hierarchy from T03 already resolves this. TOML fallback exists for environments without keychain support (CI containers, WSL, headless Linux). v1 ships all three tiers. `blue lint` warns if the TOML file has overly permissive filesystem permissions (`chmod 600` recommended, documented in setup guide).

### T11: Bot account provisioning as adoption barrier

This is an adoption concern, not a convention conflict. The T03 resolution already states bot accounts are recommended, not required. Convention: the setup guide documents bot account benefits (audit trail, token isolation) but the credential hierarchy works identically with personal tokens. No enforcement gate on account type.

---

## Position on Resolved Architecture

The convention framework is complete and internally consistent:

1. **Field ownership** (T01) -- three tiers with configurable drift policy, default `warn`
2. **Adapter contract** (T02) -- IssueTracker trait, PATH-discoverable provider, no bundled dependency
3. **Credential hierarchy** (T03) -- env var > keychain > TOML, domain-keyed, lint-enforced
4. **File structure** (T04) -- per-entity YAML in PM repo, repo-local RFC bindings, no cross-repo write contention
5. **Cardinality mapping** (T05) -- 1:1 Feature Release to Epic default, explicit override escape hatch
6. **Import bootstrap** (T06) -- two-phase `blue jira import`, git authority post-import, `imported` lifecycle state
7. **Enforcement tiers** (T07) -- always / warn-then-enforce / document-only, declared in PM repo config

Every convention is declared in repo-local config files, reviewable via PR, evolvable without code changes, and auditable via git blame. The remaining T08-T11 items are configuration defaults and documentation concerns that belong in the RFC's implementation plan, not in architectural deliberation.

---

## Summary

| Marker | Tension | Move |
|--------|---------|------|
| RESOLVED | T08 (Drift frequency) | Configuration knob in jira.toml; manual default for v1 |
| RESOLVED | T09 (Error ergonomics) | Graceful degradation already in T02; pending_sync flag |
| RESOLVED | T10 (Credential storage) | Three-tier hierarchy ships complete in v1 |
| RESOLVED | T11 (Bot account barrier) | Recommended not required; no enforcement gate |

**Perspectives:** 0 new (architecture stable)
**Tensions:** 0 new raised; T08-T11 addressed as resolvable within RFC
**Moves:** [MOVE:CONVERGE]

**Claim:** The convention framework is architecturally complete. All 7 structural tensions are resolved with specific, composable, git-native conventions. The 4 refinement tensions are implementation-level configuration decisions that belong in the RFC's design section, not in further deliberation. I signal full convergence and readiness for RFC drafting.

ALIGNMENT: 95
