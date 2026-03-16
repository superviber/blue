# Round 2 — Eclair (Developer Experience Advocate)

## T03: depends_on Semantics — Closing

I accept Scone's proposal with Macaron's refinement. The schema should be:

```yaml
depends_on:
  - path: ../other-feature.md    # default link_type: Blocks
  - path: ../related-thing.md
    link_type: relates_to
```

Single field, default `Blocks`, per-edge override. This keeps the common case simple (just list paths) while giving Macaron the expressiveness needed for non-blocking relationships. No separate `relates_to` field — one field, one place to look.

**Move**: [ACCEPT] T03 closed. Default Blocks, optional per-edge `link_type` override within `depends_on`.

---

## T07: CLI/TUI Gap — Closing with Scoped Concession

I came in wanting `blue status` and `blue board` as MVP commands. After sitting with Croissant's "hiring filter" argument across two rounds, here is where I land:

**Croissant is right that the MVP audience is git-native.** Designing a TUI board for non-git contributors is scope creep for a solo-founder tool. But DX advocacy means I need to protect the *git-native* experience too — and right now there is no quick way to answer "what's in flight?" without grepping markdown files.

**Minimal viable DX for MVP:**

1. **`blue status`** — reads `.blue/` tree, prints a one-screen summary of items by state (backlog / active / done) with Jira URLs where synced. Read-only. No mutations. This is `git log --oneline` for the PM repo. Cost: trivial to implement, high-value onboarding moment.

2. **`blue board`** (TUI), **`blue move`** (CLI mutations) — **post-MVP**. Explicitly deferred. Documented as future DX surface in the RFC.

3. **Non-git contributors** — post-MVP. The PM repo is git-native by design. Non-git access comes through Jira (which is the whole point of the sync). Contributors who cannot git can read and update in Jira; drift detection catches divergence.

This is not abandoning DX — it is sequencing it. `blue status` is cheap, proves the read path works, and gives us the hook to build richer commands later.

**Move**: [CONCEDE-WITH-SCOPE] T07 closed. `blue status` (read-only) is MVP. `blue board`, `blue move`, and non-git contributor workflows are documented as post-MVP. The Jira projection IS the non-git interface for now.

---

## T09: Lock-Ref Access — Closing as Future Work

Solo-founder MVP has a single sync actor with push access. The lock-ref model works as designed for that case.

For restricted-access orgs (multiple teams, CI-only push), the delegation model Donut raised is real but not blocking. Proposal:

- **MVP**: `refs/sync/lock` requires push access. Documented constraint.
- **Future**: If needed, introduce a sync-coordinator service or GitHub App that holds lock authority and accepts sync requests via API. This is an operational concern, not an architectural one — the lock protocol itself does not change.

**Move**: [DEFER] T09 acknowledged as future work. Documented constraint for MVP. No architectural change needed.

---

## Convergence Assessment

| Tension | Status |
|---------|--------|
| T01 | Resolved R1 |
| T02 | Resolved R1 |
| T03 | **Resolved R2** — default Blocks, per-edge override |
| T04 | Resolved R1 |
| T05 | Resolved R1 |
| T06 | Resolved R1 |
| T07 | **Resolved R2** — `blue status` MVP, rest deferred |
| T08 | Resolved R1 |
| T09 | **Deferred** — documented constraint, no architectural impact |

All eight tensions resolved or explicitly deferred with documented rationale. The architectural spine is stable:

1. DocSource trait with discover/resolve_links/writeback
2. Two-pass sync with batch atomic writeback
3. Inline jira_url, .gitattributes merge driver
4. DriftDetector as separate post-sync component
5. First-sync safety gate (three-state)
6. Domain pinning as mandatory pre-sync check
7. `blue status` as MVP read-only DX surface
8. depends_on with default Blocks, per-edge link_type

**[MOVE:CONVERGE]**

The dialogue has reached alignment. I am satisfied that DX concerns are addressed for the MVP scope and properly sequenced for future work.
