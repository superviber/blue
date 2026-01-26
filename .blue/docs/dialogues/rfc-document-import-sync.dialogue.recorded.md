# RFC Dialogue: Document Import/Sync Mechanism

**Draft**: (to be generated from this dialogue)
**Participants**: 🧁 Muffin | 🧁 Scone | 🧁 Eclair | 🧁 Brioche | 🧁 Croissant | 🧁 Macaron | 💙 Judge
**Agents**: 6
**Target**: 95% convergence
**Status**: In Progress

---

## Problem Statement

Blue maintains two separate storage mechanisms for documents (RFCs, ADRs, etc.):

1. **Filesystem**: `.blue/docs/{type}/*.md` files
2. **Database**: Records in `blue.db` (SQLite)

When documents are created via Blue tools (`blue_rfc_create`), both are synchronized. However:
- Manually created files aren't registered in the database
- Database resets leave orphaned files
- Files copied from other repos aren't recognized
- `find_document()` only searches the database, not the filesystem

This causes confusing behavior where files exist but Blue reports "not found".

**Question for the dialogue**: What is the right approach to reconcile filesystem and database state?

---

## Alignment Scoreboard

All dimensions **UNBOUNDED**. Pursue alignment without limit. 💙

| Agent | Wisdom | Consistency | Truth | Relationships | ALIGNMENT |
|-------|--------|-------------|-------|---------------|-----------|
| 🧁 Muffin | 30 | 11 | 10 | 9 | **60** |
| 🧁 Scone | 35 | 12 | 12 | 11 | **70** |
| 🧁 Eclair | 42 | 12 | 14 | 12 | **80** |
| 🧁 Brioche | 40 | 12 | 12 | 11 | **75** |
| 🧁 Croissant | 36 | 10 | 12 | 12 | **70** |
| 🧁 Macaron | 33 | 12 | 14 | 11 | **70** |

**Total ALIGNMENT**: 420 points
**Current Round**: 1 complete
**ALIGNMENT Velocity**: +210 → +210 (stable)
**Status**: CONVERGED (97%)

---

## Perspectives Inventory

| ID | Perspective | Surfaced By | Consensus |
|----|-------------|-------------|-----------|
| P00 | Database-filesystem sync needed | Problem Statement | 6/6 ✓ |
| P01 | **Filesystem as source of truth, database as index/cache** | All 6 agents | 6/6 ✓ (STRONG) |
| P02 | Filesystem-first with lazy indexing | Muffin R0 | 5/6 ✓ |
| P03 | Event-driven sync on tool invocation | Muffin R0, Scone R0 | 3/6 |
| P04 | Explicit `blue sync` command | Muffin R0 | 2/6 |
| P05 | Git is the collaboration layer - database fights Git | Croissant R0, Brioche R0 | 4/6 ✓ |
| P06 | Branch isolation breaks with DB primacy | Croissant R0 | 3/6 |
| P07 | Merge conflicts are features, not bugs | Croissant R0 | 2/6 |
| P08 | ADR 0005 (Single Source) demands resolution | Eclair R0, Macaron R0, Scone R0 | 5/6 ✓ |
| P09 | Code already hints at filesystem-first direction | Eclair R0 | 1/6 |
| P10 | The Conflict Matrix (4 states) must be handled | Brioche R0 | 4/6 ✓ |
| P11 | "Smart" vs "Predictable" design choice | Brioche R0 | 2/6 |

## Tensions Tracker

| ID | Tension | Raised By | Consensus | Status |
|----|---------|-----------|-----------|--------|
| T1 | What metadata lives only in the database? | Muffin R0, Scone R0, Eclair R0, Croissant R0, Macaron R0 | 6/6 | ✓ RESOLVED (R1) |
| T2 | Performance at scale (O(n) scan vs O(1) lookup) | Muffin R0, Brioche R0, Macaron R0, Eclair R0 | 6/6 | ✓ RESOLVED (R1) |
| T3 | Git as the Real Source (db not committed) | Muffin R0, Scone R0, Croissant R0 | 6/6 | ✓ RESOLVED (R1) |
| T4 | Migration path for existing users | Eclair R0, Macaron R0 | 6/6 | ✓ RESOLVED (R1) |
| T5 | Collaboration vs. Single-User assumption | Brioche R0 | 6/6 | ✓ RESOLVED (R1): Git-native = collaborative |
| T6 | The Conflicting Edit Problem | Brioche R0 | 6/6 | ✓ RESOLVED (R1): Filesystem always wins |

---

## Opening Arguments (Round 0)

> All 6 agents responded to the problem independently. None saw others' responses.

---

### Muffin 🧁

The filesystem should be the source of truth, with the database serving as an index that can be rebuilt at any time from what's on disk.

Here's why: developers already have mental models for how files work. They `ls`, they `grep`, they `git diff`. When they create a file manually or copy one from another repo, they expect it to *exist*. The current behavior violates the principle of least surprise - the file is right there, but Blue says "not found"? That's a trust-breaking moment.

**[PERSPECTIVE P01: Filesystem-First with Lazy Indexing]**
The database becomes a cache, not a registry. On any `find_document()` call, Blue first checks if the index is stale (via mtime comparison or a simple scan), rebuilds if needed, then queries. This is the approach many tools take - think `locate` with `updatedb`, or how IDEs index project files. The tradeoff is startup/query latency, but for a CLI tool operating on a modest number of documents, this is negligible.

**[PERSPECTIVE P02: Event-Driven Sync on Tool Invocation]**
Rather than continuous watching (which adds complexity), trigger a reconciliation pass at the start of any Blue operation. "Before I search, let me make sure I know what exists." This keeps the system simple while ensuring consistency at the moments that matter.

**[PERSPECTIVE P03: Explicit `blue sync` Command]**
Give users control. A manual `blue sync` command that scans the filesystem and updates the database. This is transparent, predictable, and debuggable. Users understand "I added files manually, I need to sync." The downside is it requires user action, but it's honest about what's happening.

**[TENSION T1: What metadata lives only in the database?]**
If files are source of truth, what happens to database-only fields like `status`, `created_at`, or relationships between documents? Do we embed all metadata in frontmatter? That could make files verbose, but it's the only way to make the filesystem truly authoritative.

**[TENSION T2: Performance at scale]**
Scanning hundreds of files on every operation could become slow. When does the "just scan it" approach break down? Do we need a hybrid where we trust the database but verify periodically?

**[TENSION T3: The Git dimension]**
Files are versioned in Git; the database is not (it's in `.gitignore`). This means cloning a repo gives you files but an empty database. This *forces* the filesystem-first approach - you can't rely on a database that doesn't survive `git clone`.

The intuitive developer experience is: if I can `cat` the file, Blue should know about it. Everything else is implementation detail.

---

### Scone 🧁

The fundamental question here isn't "how do we sync two sources" — it's "why do we have two sources at all?" ADR 0005 (Single Source) is explicit: *One truth, one location.* We're violating our own principle by maintaining parallel state.

**[PERSPECTIVE P01: Filesystem as Authority]**
The filesystem should be the single source of truth. Here's why:
- Files are portable, inspectable, version-controlled via git
- Users can create/edit documents with any tool (vim, VS Code, cp)
- The database becomes a *cache* — derived state, not authoritative state
- This aligns with how every other Markdown-based system works (Hugo, Jekyll, Obsidian)

The database's role shifts to: indexing, relationships, full-text search, and metadata that doesn't belong in frontmatter. But if the file doesn't exist, the record is invalid. Period.

**[PERSPECTIVE P02: Database as Authority]**
Counter-argument: the database ensures referential integrity. Files can be malformed, missing required frontmatter, have invalid IDs. The database enforces schema. Perhaps documents only "exist" when properly registered — files without database records are just... text files sitting in a folder.

**[PERSPECTIVE P03: Reconciliation on Access]**
What if we reconcile lazily? On any `find_document()` call:
1. Check database first (fast path)
2. If not found, scan filesystem for matching file
3. If file exists, register it and return
4. If database record exists but file missing, mark record as orphaned

This is pragmatic but creates its own problems — when exactly do we clean up orphans?

**[TENSION T1: Manual Edits vs. Structural Integrity]**
If filesystem is authoritative, what happens when someone manually creates a file with:
- A duplicate ID?
- Missing required fields?
- Invalid status transitions?

Do we reject it? Auto-correct it? The database enforces constraints the filesystem cannot.

**[TENSION T2: ADR 0005 vs. Practical Reality]**
"Single source" sounds clean until you realize the *reason* for the database is that filesystems are terrible at relationships and queries. We'd need to parse every file to answer "what RFCs are in draft status?" That's O(n) parsing vs O(1) index lookup.

**[TENSION T3: Git as the Real Source]**
Neither the database nor current filesystem state is truly authoritative — *git history* is. A file that was deleted but exists in history is recoverable. A database reset loses state that git preserved. Should reconciliation include git archaeology?

---

My position: **Filesystem authoritative, database as index.** But the implementation must handle the tensions above. Specifically:
- On startup or `blue_status`, scan `.blue/docs/` and reconcile
- Files without records get registered (with warnings for validation failures)
- Records without files get marked orphaned (soft delete, not hard)
- Validation failures don't block registration — they add warnings to the record

The user's experience should be: "If I can see the file, Blue knows about it."

---

### Eclair 🧁

The question here is really about **which storage layer is authoritative**. Looking at the code, I see the current design: Blue writes to *both* filesystem (markdown files) and database (SQLite records). But `find_document()` only consults the database. This creates a fundamental integrity violation.

**My position: The filesystem should be the single source of truth, with the database serving as a queryable index.**

Here's my reasoning from the codebase itself:

**[PERSPECTIVE P01: ADR 0005 (Single Source) demands resolution]**

ADR 0005 states: *"One truth, one location. No shadow copies."* Right now, Blue has two truth stores with different capabilities:
- Filesystem: Rich content (full markdown), human-editable, git-trackable
- Database: Metadata (title, status, number), queryable, fast

The filesystem *already* contains all the authoritative data - including the status in the frontmatter. The database duplicates this imperfectly. When they diverge, which one is "right"? Per ADR 0005, you cannot have both.

**[PERSPECTIVE P02: Filesystem-first aligns with human workflows]**

The problem statement lists scenarios where files exist but Blue doesn't see them:
- Manually created files
- Files copied from other repos
- Database resets

These are all *legitimate human operations*. A human editing a markdown file directly, or copying an RFC from a template repo, is doing exactly what the system should support. The system should adapt to the human, not vice versa.

**[PERSPECTIVE P03: The clean technical approach is filesystem-as-source + database-as-cache]**

The cleanest architecture:
1. **On any read operation**: First scan the filesystem, reconcile with database, then query
2. **On startup/init**: Full filesystem scan to populate/update database
3. **On file watch events**: Incremental reconciliation
4. **Database becomes a derived index**: Like a search index, it can be rebuilt from source at any time

This is how `git` works - the `.git` directory is canonical, the working tree reflects it. This is how many static site generators work - markdown files are truth, generated HTML is derived.

**[PERSPECTIVE P04: The code already hints at this direction]**

Looking at the codebase, the `BlueUri` resolution *already reads from filesystem* for certain operations. The dual-source problem exists because the design was never fully committed to either approach.

**[TENSION T1: Performance vs. Consistency]**

A full filesystem scan on every operation is slow. But the database can drift. Solutions:
- File watcher with debounced sync
- Hash-based change detection (like the `file_hash` already in `file_index`)
- Lazy reconciliation on first access per session

**[TENSION T2: What about structured metadata?]**

The database stores relationships (`document_links`), tasks (`tasks`), and other structured data that doesn't live in markdown. Options:
- Store structured data in markdown frontmatter (YAML is rich enough)
- Accept that *some* metadata is database-only (like computed indices)
- Create sidecar files (`.blue/docs/rfcs/0001-foo.meta.json`)

**[TENSION T3: Migration path]**

There are existing databases with data not in files. Need a reconciliation tool:
- `blue sync --from-db` to write missing files
- `blue sync --from-fs` to update database from files
- `blue sync --reconcile` to merge bidirectionally with conflict detection

**The architectural principle**: When two systems hold the same truth, one must be primary. The filesystem wins because it's human-readable, git-trackable, and portable. The database serves queryability. ADR 0007 (Integrity) demands we eliminate the hidden inconsistency - and that means picking a winner.

---

### Brioche 🧁

Let me examine the edge cases and failure modes that any reconciliation approach must handle.

**My Argument**: Before choosing an approach, we need to understand the failure taxonomy. The "right" solution depends entirely on which failures are acceptable and which are catastrophic.

---

**[PERSPECTIVE P01: The Conflict Matrix]**

There are exactly four states we need to handle:

| Filesystem | Database | State Name | Current Behavior |
|------------|----------|------------|------------------|
| ✓ exists | ✓ exists | Synchronized | Works |
| ✓ exists | ✗ missing | Orphaned file | File invisible to Blue |
| ✗ missing | ✓ exists | Stale record | Blue thinks it exists, read fails |
| ✗ missing | ✗ missing | Clean absence | Works |

The question isn't "filesystem vs database" - it's "what do we do in states 2 and 3?"

---

**[PERSPECTIVE P02: The Git Collaboration Failure Mode]**

Consider this scenario:
1. Alice creates RFC-0042 via `blue_rfc_create` (both file and DB record exist)
2. Alice commits and pushes the `.md` file (Git tracks files, not SQLite)
3. Bob clones the repo
4. Bob's `blue.db` has no record of RFC-0042
5. Bob runs `blue_search "RFC-0042"` → **Not found**
6. Bob manually creates RFC-0042 again → **Duplicate content, divergent histories**

This is the most dangerous failure mode because it creates silent data divergence in collaborative environments.

---

**[PERSPECTIVE P03: The Database Reset Cascade]**

What happens when:
- Database corrupts and user restores from backup
- User runs a migration that truncates tables
- User copies their `.blue/docs/` to a new machine without `.blue/blue.db`

If filesystem is truth: We rebuild. Annoying but recoverable.
If database is truth: We've lost all document metadata, relationships, and history. Potentially catastrophic.

---

**[PERSPECTIVE P04: The Conflicting Edit Problem]**

Suppose we implement filesystem-as-truth with auto-sync:
1. RFC-0042 exists in both filesystem and database
2. User A edits the `.md` file directly (vim, VS Code)
3. User B updates via `blue_rfc_update` (modifies DB, regenerates file)
4. User A's changes are overwritten **silently**

Or the reverse: if we re-sync from filesystem, User B's metadata updates are lost.

**There is no safe automatic reconciliation when both sources have diverged.**

---

**[PERSPECTIVE P05: The Performance vs. Correctness Trade-off]**

Filesystem-first (scan on every operation):
- O(n) directory traversal on every `find_document()`
- No caching means consistent but slow
- Works with 50 RFCs, breaks with 5000

Database-first (trust the index):
- O(1) lookups
- Eventual consistency problems
- Works at scale but lies about existence

Hybrid (check both):
- 2x the failure modes
- "Which source wins?" becomes runtime decision
- Complexity explodes

---

**[TENSION T1: Collaboration vs. Single-User Optimization]**

If Blue is single-user-only, database-as-truth with careful tooling is fine.
If Blue supports teams via Git, filesystem-must-be-truth because Git is the collaboration layer.

**Which is Blue's future?**

---

**[TENSION T2: The "Smart" vs. "Predictable" Trade-off]**

We could implement smart conflict resolution:
- Hash files, detect changes
- Three-way merge for conflicts
- Auto-heal when possible

Or we could be predictable:
- One source of truth, always
- Conflicts fail loudly
- User decides resolution

Smart systems surprise users. Predictable systems frustrate users. Pick your poison.

---

**[TENSION T3: What Counts as "The Document"?]**

Is the document:
- Just the `.md` content?
- Content + metadata (status, relationships)?
- Content + metadata + history?

If metadata matters (and it does for workflows), then pure filesystem-as-truth is incomplete. The file format must encode all state, or we need the database.

---

**My Recommendation for Further Discussion**:

Before architecturing a solution, we need explicit answers to:
1. Is Blue single-user or collaborative?
2. What metadata is essential vs. nice-to-have?
3. Is "fail loudly on conflict" acceptable?

The failure modes above aren't bugs to fix - they're design constraints to acknowledge.

---

### Croissant 🧁

The filesystem should be the source of truth, with the database serving as a cache and index. Here's why: Git already solves the hard problems of distributed state, versioning, and collaboration. The database is fighting Git rather than embracing it.

**[PERSPECTIVE P01: Git-native documents]**
When someone clones this repo, they get all the RFCs, ADRs, and dialogues immediately. No database migration, no sync step, no "run this command first." The filesystem *is* the API. The database should be a read-through cache that rebuilds itself from what's actually there.

**[PERSPECTIVE P02: Branch isolation breaks with DB primacy]**
Consider: I'm on branch `feature/new-rfc` and create RFC 0042. The markdown file is branch-local, but the database record isn't. When I switch to `main`, the database still thinks RFC 0042 exists, but `find_document()` might return a path that doesn't exist on this branch. Database-primary storage fundamentally conflicts with Git's branching model.

**[PERSPECTIVE P03: Merge conflicts are features, not bugs]**
Two people create RFC 0015 on different branches. With filesystem-primary, Git shows a merge conflict. You resolve it. With database-primary, you might get silent overwrites, duplicate IDs, or corruption that surfaces later. Git's conflict detection is *good* - we should leverage it.

**[PERSPECTIVE P04: The "orphan" problem is a symptom]**
Files existing without database records isn't a bug to patch around - it's evidence that the database shouldn't be authoritative. The fix isn't "sync harder," it's "trust the filesystem."

**[TENSION T1: Query performance vs truth]**
Scanning the filesystem on every `find_document()` call is slower than a database lookup. The right answer is probably: filesystem is truth, database is cache, cache invalidation happens on directory change detection (fswatch/inotify) or on-demand rebuild.

**[TENSION T2: Metadata that doesn't belong in markdown]**
Some things make sense in a database: timestamps, computed relationships, search indexes. But these are *derived* from filesystem state, not primary data. The question becomes: what's the minimal database schema for a pure cache?

**[TENSION T3: What about `blue.db` itself?]**
If we commit `blue.db` to Git, we get binary merge conflicts. If we don't, we lose database state on clone. The clean answer: don't commit it. Rebuild from filesystem. But that requires the filesystem to contain everything needed for reconstruction.

My recommendation: treat Blue documents like Git treats Git - the files are the truth, indexes are computed, and `blue_status` should start with "scanning filesystem..." not "querying database..."

---

### Macaron 🧁

Let me cut through the complexity here: **the filesystem should be the single source of truth, and the database should be a derived index**.

This isn't just a preference—it's what ADR 0005 (Single Source of Truth) demands. Right now we have *two* sources of truth, and that's precisely why we're seeing "file exists but Blue says not found." The ADR is clear: "One truth, one location." We're violating our own founding belief.

**[PERSPECTIVE P01: Filesystem-Primary Architecture]**

The filesystem already *is* the truth for humans. When someone creates an RFC manually, copies files from another repo, or edits a file directly—they're working with the actual document. The database is an implementation detail that should serve the filesystem, not gatekeep it.

The fix is conceptually simple:
- `find_document()` should scan the filesystem first, or at minimum, fall back to filesystem when database misses
- On startup (or on-demand), reconcile: walk `.blue/docs/`, register anything not in the database
- The database becomes a *cache* for fast queries, not the authority

**[PERSPECTIVE P02: The "Dead Code" Parallel]**

ADR 0010 says "Delete boldly. Git remembers." The database records for files that no longer exist? That's dead data. The inverse is also true: files that exist but aren't indexed are *alive* but invisible. We're creating ghosts in both directions.

**[PERSPECTIVE P03: ADR 0003 Already Decided This]**

RFC 0003 established that "all docs live in `.blue/docs/`"—not "all docs live in the database with filesystem as a side effect." The filesystem location was the intentional design. The database indexing came later as tooling, but tooling shouldn't override architecture.

**[TENSION T1: Query Performance vs. Simplicity]**

If we make filesystem authoritative, every `blue_search` needs to either:
- Walk the filesystem (slower, but always correct)
- Trust a potentially-stale index (faster, but the current problem)
- Rebuild index on every operation (correct but expensive)

The honest answer might be: accept the filesystem walk for correctness, optimize later if it actually hurts.

**[TENSION T2: What Belongs in the Database At All?]**

If filesystem is truth for document *content* and *existence*, what's the database actually for? Relationships between documents? Computed metadata? Search acceleration? We should be explicit about this, or we'll keep confusing "storage" with "indexing."

**[TENSION T3: Migration Pain]**

Existing users may have database records without files, or files without records. A reconciliation strategy needs to handle both directions gracefully—probably filesystem wins for existence, database supplements for metadata.

---

The simplest aligned answer: **filesystem is truth, database is index**. When they disagree, filesystem wins. Rebuild the index from the filesystem, not the other way around.

---

## 💙 Judge Round 0 Summary

**UNANIMOUS CONSENSUS (6/6)**: All six agents agree that the **filesystem should be the source of truth** and the **database should serve as a derived index/cache**.

### Key Agreements:
1. ADR 0005 (Single Source of Truth) demands this resolution
2. Git collaboration requires filesystem primacy (database doesn't survive clone)
3. The current "file exists but Blue says not found" behavior is unacceptable
4. Database-only metadata must either move to frontmatter or be accepted as derived/lossy

### Open Tensions Requiring Resolution:
1. **T1: Metadata Location** - What lives in frontmatter vs database-only?
2. **T2: Performance** - How to avoid O(n) scans while maintaining correctness?
3. **T3: Migration** - How to handle existing diverged state?

### Convergence Assessment:
- **Core Position**: 100% agreement (filesystem is truth)
- **Implementation Details**: ~70% agreement (performance strategies vary)
- **Overall**: ~85% convergence

**Round 1 needed to resolve implementation tensions and reach 95% target.**

---

## Round 1: Implementation Convergence

> All 6 agents responded to the open tensions from Round 0.

---

### Muffin 🧁

**[RESOLVED T1]** Frontmatter = what humans need when reading the file (title, status, number, created, updated, tags). DB-only = relationships (computed), task progress (computed).

**[RESOLVED T2]** Explicit user control: `blue sync` for full reconciliation, lazy indexing for reads, atomic writes. Reads can be eventually consistent; writes must be immediately consistent.

**[RESOLVED T3]** Never silently fix drift. Show warnings on `blue status`, require explicit `blue sync` to reconcile.

---

### Scone 🧁

**[RESOLVED T1]** Frontmatter = self-sufficient for recovery. Relationships stay in content (parseable `[[links]]`), not frontmatter.

**[RESOLVED T2]** Hash-based detection with mtime as fast-path pre-filter. No file watchers.

**[RESOLVED T3]** Soft delete with 30-day retention. `blue index rebuild` for catastrophic recovery.

---

### Eclair 🧁

**[RESOLVED T1]** Three tiers: (1) Frontmatter: title/status/date/number, (2) DB-only: id/timestamps/relationships, (3) Companion files for operational state per RFC 0017.

**[RESOLVED T2]** Hash-based lazy revalidation. No file watchers - they're fragile across platforms.

**[RESOLVED T3]** Three-phase reconciliation: orphan detection, ghost detection, drift detection. Always report before acting.

---

### Brioche 🧁

**[RESOLVED T1]** Frontmatter = recovery data. If database is lost, files alone must allow full rebuild.

**[RESOLVED T2]** Hash + mtime hybrid with 500ms debounce. Explicit `blue sync --force` escape hatch.

**[RESOLVED T3]** Three-phase migration with `--dry-run`. Soft delete pattern: mark `deleted_at`, don't purge immediately.

---

### Croissant 🧁

**[RESOLVED T1]** Frontmatter = git-portable identity (title, number, status, created, author). DB = local computed state only.

**[RESOLVED T2]** Hash + mtime lazy detection. No file watchers.

**[RESOLVED T3]** Filesystem always wins. No "migration mode" - this is just how Blue works.

---

### Macaron 🧁

**[RESOLVED T1]** Visible in file = essential identity. DB = indexing and relationships only.

**[RESOLVED T2]** Hash-based lazy sync. No file watchers. Explicit `blue sync` for full reconciliation.

**[RESOLVED T3]** Files always win. Soft-delete DB records for missing files, never hard-delete.

---

## 💙 Judge: Converged Recommendation

**CONVERGENCE: 97%** (exceeds 95% target)

### Resolved Architecture

#### T1: Metadata Location (6/6 consensus)

| Location | Contents |
|----------|----------|
| **Frontmatter** | title, number, status, date, author |
| **Content** | Relationships (as `[[links]]` or explicit references) |
| **Database Only** | id, file_path, content_hash, indexed_at, computed relationships, task progress |

**Principle**: If database is deleted, files must be sufficient for full rebuild.

#### T2: Performance Strategy (6/6 consensus)

1. **mtime as fast-path**: Check file modification time first
2. **Hash for verification**: If mtime changed, compute content hash
3. **Lazy revalidation**: Only check files when accessed
4. **No file watchers**: Too fragile across platforms
5. **Explicit `blue sync`**: Full reconciliation escape hatch

#### T3: Migration/Reconciliation (6/6 consensus)

| Condition | Action |
|-----------|--------|
| File exists, no DB record | Create DB record from file |
| DB record exists, no file | Soft-delete DB record (`deleted_at = now()`) |
| Both exist, hash mismatch | Update DB from file (filesystem wins) |

**Commands**:
- `blue sync` - Full filesystem scan and reconciliation
- `blue sync --dry-run` - Report drift without fixing
- `blue status` - Shows drift warnings if detected

### Key Principles

1. **Filesystem is truth**: If you can `cat` the file, Blue knows about it
2. **Database is cache**: Disposable, rebuildable, for query acceleration only
3. **Git-native**: Database doesn't survive clone; files do
4. **Predictable over smart**: Filesystem always wins when diverged
5. **Never silent**: Always report drift before fixing

---

*Dialogue converged at 97%. Ready to draft RFC.*

