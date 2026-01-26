# Spike: Dialogue To Blue Directory

| | |
|---|---|
| **Status** | Complete |
| **Date** | 2026-01-24 |
| **Time Box** | 1 hour |

---

## Question

How can we move dialogues from docs/dialogues/ to .blue/docs/dialogues/ and what migration is needed?

---

## Findings

### Current State

**Good news:** The code already writes new dialogues to `.blue/docs/dialogues/`.

The dialogue handler (`crates/blue-mcp/src/handlers/dialogue.rs:291-293`) uses:

```rust
let file_path = PathBuf::from("dialogues").join(&file_name);
let docs_path = state.home.docs_path.clone();  // .blue/docs/
let dialogue_path = docs_path.join(&file_path);  // .blue/docs/dialogues/
```

And `BlueHome.docs_path` is set to `.blue/docs/` in `crates/blue-core/src/repo.rs:44`:

```rust
docs_path: blue_dir.join("docs"),
```

**The problem:** 4 legacy dialogue files exist outside `.blue/`:

```
docs/dialogues/persephone-phalaenopsis.dialogue.md
docs/dialogues/cross-repo-realms.dialogue.md
docs/dialogues/cross-repo-realms-refinement.dialogue.md
docs/dialogues/realm-mcp-design.dialogue.md
```

These were created before RFC 0003 was implemented.

### What Needs to Happen

1. **Move files:** `docs/dialogues/*.dialogue.md` → `.blue/docs/dialogues/`
2. **Update database:** Fix `file_path` column in `documents` table
3. **Clean up:** Remove empty `docs/dialogues/` directory

### Options

#### Option A: Manual Migration (One-Time)

```bash
# Move files
mkdir -p .blue/docs/dialogues
mv docs/dialogues/*.dialogue.md .blue/docs/dialogues/

# Update database paths
sqlite3 .blue/data/blue/blue.db <<'EOF'
UPDATE documents
SET file_path = REPLACE(file_path, '../../../docs/dialogues/', 'dialogues/')
WHERE doc_type = 'dialogue';
EOF

# Clean up
rmdir docs/dialogues
```

**Pros:** Simple, done once
**Cons:** Other repos might have same issue

#### Option B: Auto-Migration in `detect_blue()`

Add dialogue migration to the existing migration logic in `repo.rs`:

```rust
// In migrate_to_new_structure():
let old_dialogues = root.join("docs").join("dialogues");
let new_dialogues = new_docs_path.join("dialogues");
if old_dialogues.exists() && !new_dialogues.exists() {
    std::fs::rename(&old_dialogues, &new_dialogues)?;
}
```

Plus update the store to fix file_path entries after migration.

**Pros:** Handles all repos automatically
**Cons:** More code to maintain

#### Option C: Support Both Locations (Read)

Modify `handle_get()` to check both locations:

```rust
let content = if let Some(ref rel_path) = doc.file_path {
    let new_path = state.home.docs_path.join(rel_path);
    let old_path = state.home.root.join("docs").join(rel_path);

    fs::read_to_string(&new_path)
        .or_else(|_| fs::read_to_string(&old_path))
        .ok()
} else {
    None
};
```

**Pros:** No migration needed, backwards compatible
**Cons:** Technical debt, two locations forever

### Recommendation

**Option A (manual migration)** for Blue repo + **Option B (auto-migration)** as follow-up RFC.

Rationale:
- The legacy dialogues only exist in the blue repo
- Manual migration is quick and verifiable
- Auto-migration can be added properly with tests

### Database Path Investigation

**Finding:** The 4 legacy dialogues are not registered in the database at all.

```
sqlite3 .blue/blue.db "SELECT doc_type, COUNT(*) FROM documents GROUP BY doc_type"
rfc|6
spike|7
```

No dialogue entries. They exist only as markdown files.

**This simplifies migration:** Just move the files. No database updates needed.

If we want them tracked in SQLite, we could either:
1. Register them after moving with `blue_dialogue_create`
2. Import them with a script that parses the markdown headers

### Edge Cases

1. **In-flight dialogues:** If someone creates a dialogue during migration, could conflict
2. **Git history:** Moving files loses git blame (use `git mv` to preserve)
3. **Symlinks:** If `docs/dialogues` is a symlink, need to handle differently

### Migration Commands

Since dialogues aren't in the database, migration is just a file move:

```bash
# Create destination
mkdir -p .blue/docs/dialogues

# Move with git history preservation
git mv docs/dialogues/*.dialogue.md .blue/docs/dialogues/

# Clean up empty directory
rmdir docs/dialogues
rmdir docs  # if empty

# Commit
git add -A && git commit -m "chore: move dialogues to .blue/docs/dialogues"
```

### Future Consideration

To register existing dialogues in SQLite, we could add a `blue_dialogue_import` tool that:
1. Parses the markdown header for title, date, linked RFC
2. Creates document entries in SQLite
3. Sets correct file_path relative to `.blue/docs/`

This is optional - the files are still human-readable without database tracking.

---

## Conclusion

**Simpler than expected.** The code already writes to `.blue/docs/dialogues/`. The 4 legacy files just need `git mv` to the new location. No database migration needed since they were never registered.

**Recommend:** Move files with `git mv`, then optionally register them with a new import tool later.
