# RFC 0066: Clippy Lint Fixes

| | |
|---|---|
| **Status** | Draft |
| **Date** | 2026-03-11 |

---

## Summary

Fix clippy warnings blocking CI: implement `std::str::FromStr` on 12 alignment_db enums (replacing ad-hoc `from_str` methods), and refactor two functions with too many positional arguments into param structs.

## Problem

`cargo clippy --workspace -- -D warnings` fails with 14 errors, all in `crates/blue-core/src/alignment_db.rs`:

- **12x `should_implement_trait`**: Enums define `pub fn from_str(s: &str)` which shadows the standard `FromStr` trait
- **2x `too_many_arguments`**: `register_expert` (12 args) and `register_recommendation` (8 args) exceed clippy's 7-arg limit

Additionally, `cargo fmt --check` fails on formatting inconsistencies.

## Fix 1: Implement `FromStr` Trait

**Affected enums** (all in `alignment_db.rs`):
`DialogueStatus`, `ExpertTier`, `ExpertSource`, `PerspectiveStatus`, `TensionStatus`, `RecommendationStatus`, `ClaimStatus`, `EvidenceStatus`, `EntityType`, `RefType`, `VerdictType`, `MoveType`

**Current** (ad-hoc method):
```rust
impl VerdictType {
    pub fn from_str(s: &str) -> Self { ... }
}
```

**Fixed** (proper trait impl):
```rust
impl std::str::FromStr for VerdictType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> { ... }
}
```

**Callers** to update (two files):
- `crates/blue-core/src/alignment_db.rs` — ~10 internal call sites
- `crates/blue-mcp/src/handlers/dialogue.rs` — ~15 call sites

Caller migration: `VerdictType::from_str(s)` → `s.parse::<VerdictType>().unwrap_or_default()` (or handle error).

Note: Some existing `from_str` methods return `Option<Self>` or `Self` (with defaults). The `FromStr` impl should return `Result<Self, String>`, and callers use `.ok()` or `.unwrap_or_default()` as appropriate.

## Fix 2: Param Structs for Large Functions

**`register_expert`** (12 args → struct):
```rust
pub struct RegisterExpertParams<'a> {
    pub dialogue_id: &'a str,
    pub expert_slug: &'a str,
    pub display_name: &'a str,
    pub role: &'a str,
    pub tier: ExpertTier,
    pub source: ExpertSource,
    pub relevance: f64,
    pub focus: Option<&'a str>,
    pub emoji: Option<&'a str>,
    pub perspective: Option<&'a str>,
    pub first_round: Option<i32>,
}

pub fn register_expert(conn: &Connection, params: RegisterExpertParams) -> Result<(), AlignmentDbError>;
```

**`register_recommendation`** (8 args → struct):
```rust
pub struct RegisterRecommendationParams<'a> {
    pub dialogue_id: &'a str,
    pub round: i32,
    pub expert_slug: &'a str,
    pub content: &'a str,
    pub parameters: Option<&'a serde_json::Value>,
    pub refs: Option<&'a [Reference]>,
}

pub fn register_recommendation(conn: &Connection, params: RegisterRecommendationParams) -> Result<String, AlignmentDbError>;
```

## Fix 3: Format

Run `cargo fmt --all` and commit the result.

## Implementation Plan

- [ ] Run `cargo fmt --all`
- [ ] Implement `FromStr` for all 12 enums
- [ ] Update callers in `alignment_db.rs`
- [ ] Update callers in `dialogue.rs`
- [ ] Create `RegisterExpertParams` struct, update function + callers
- [ ] Create `RegisterRecommendationParams` struct, update function + callers
- [ ] Verify `cargo clippy --workspace -- -D warnings` passes
- [ ] Verify `cargo test --workspace` passes

## Test Plan

- [ ] `cargo clippy --workspace -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo test --workspace` passes
- [ ] CI pipeline (GitHub Actions) passes green

---

*"Right then. Let's get to it."*

— Blue
