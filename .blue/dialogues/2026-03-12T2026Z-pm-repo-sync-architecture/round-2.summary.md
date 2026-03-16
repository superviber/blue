# Round 2 Summary — PM Repo Sync Architecture (FINAL)

## Convergence: 6/6 experts signal CONVERGE. Velocity = 0.

### T03 Resolved — depends_on Link Types
**Consensus**: Single `depends_on` field. Entries default to `Blocks`. Per-edge `link_type` override via map syntax for non-blocking relationships. DAG cycle detection scoped to Blocks edges only.

Minor split: Scone conceded to Macaron on separate `depends_on`/`relates_to` fields, while Muffin/Cupcake/Eclair/Croissant preferred single field with per-edge override. The panel majority favors single field. Either approach works — the architectural point (resolve_links handles it) is agreed.

### T07 Resolved — CLI/TUI Gap
**Consensus**: MVP targets git-native solo founders. `blue status` (read-only) is minimal MVP DX. `blue board`, `blue move`, and mutation CLI commands are documented as post-MVP future work. DriftDetector's scoped read-back (status + sprint) bridges the gap. Not architecturally load-bearing.

### T09 Deferred — Lock-Ref Access
**Consensus**: Solo-founder MVP assumes push access. Delegation model for restricted-access orgs is future work. Documented constraint.

## Final Architecture (Aligned)

### 1. DocSource Trait
```rust
trait DocSource {
    fn discover(&self) -> Vec<SyncItem>;
    fn resolve_links(&self, mapping: &IdMap) -> Vec<LinkRequest>;
    fn writeback(&self, results: &SyncResults) -> WritebackSet;
}
```

### 2. Sync Engine Orchestration
`discover() → push() → resolve_links() → push_links() → verify() → writeback()`

### 3. Writeback Model
- Inline `jira_url` in YAML front matter (single machine-owned field)
- `.gitattributes` merge driver for conflict resolution
- Batch atomic commit after all API calls succeed
- Conventional commit prefix: `blue-sync: writeback`

### 4. DriftDetector
- Separate post-sync component, not part of sync engine
- Produces typed DriftReport with severity levels (info/warn/error)
- Scoped read-back authority: status + sprint are Jira-authoritative
- All other fields: git-authoritative with drift warnings
- Three modes: `--verify`, `--check` (CI gate), `--check-drift`

### 5. First-Sync Safety Gate
- Three-state: dry-run default → `--confirm` → implicit
- Presence of `jira_url` in any file = past first-sync

### 6. Security
- jira.toml domain pinning via tracked `.sync/domain-pin.sha256`
- .env.local NOT a credential tier (pre-flight check for credential patterns)
- Three-tier credential hierarchy preserved (env vars → keychain → TOML)

### 7. depends_on Schema
- Single `depends_on` field, default `Blocks` link type
- Per-edge `link_type` override for non-blocking relationships
- Two-pass sync: create all issues → resolve all links
- DAG cycle detection on Blocks edges only

### 8. Future Work
- `blue status`, `blue board`, `blue story create` CLI commands
- Lock-ref delegation model for restricted-access orgs
- TUI board view
- Non-git contributor workflows
