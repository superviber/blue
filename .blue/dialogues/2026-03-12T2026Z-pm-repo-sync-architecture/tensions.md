# Tensions — PM Repo Sync Architecture (FINAL)

| ID | Tension | Status | Raised | Resolved |
|----|---------|--------|--------|----------|
| T01 | Writeback location: inline `jira_url` in front matter | **Resolved** | R0 | R1 |
| T02 | Writeback atomicity: batch commit after all API calls | **Resolved** | R0 | R1 |
| T03 | depends_on semantics: single field, default Blocks, per-edge `link_type` override | **Resolved** | R0 | R2 |
| T04 | First-sync safety: three-state model (dry-run → confirm → implicit) | **Resolved** | R0 | R1 |
| T05 | Read-back: post-sync DriftDetector with scoped authority (status + sprint) | **Resolved** | R0 | R1 |
| T06 | .env.local: not a credential tier, pre-flight check only | **Resolved** | R0 | R1 |
| T07 | CLI/TUI gap: git-native MVP, `blue status` post-MVP, DriftDetector as bridge | **Resolved** | R0 | R2 |
| T08 | jira.toml domain pinning: mandatory pre-sync gate, tracked hash file | **Resolved** | R0 | R1 |
| T09 | Lock-ref push access: documented constraint, delegation model future work | **Deferred** | R1 | — |

**All tensions resolved or explicitly deferred. Panel converged 6/6.**
