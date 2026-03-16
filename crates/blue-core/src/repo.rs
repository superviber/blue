//! Git repository detection and operations for Blue
//!
//! Finds Blue's home (.blue/) and manages worktrees.

use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::debug;

/// Blue's directory structure detection result
///
/// Per-repo structure (RFC 0003):
/// ```text
/// repo/
/// ├── .blue/
/// │   ├── docs/           # RFCs, spikes, runbooks, etc.
/// │   ├── worktrees/      # Git worktrees for RFC implementation
/// │   ├── blue.db         # SQLite database
/// │   └── config.yaml     # Configuration
/// └── src/...
/// ```
#[derive(Debug, Clone)]
pub struct BlueHome {
    /// Root directory (git repo root) containing .blue/
    pub root: PathBuf,
    /// Path to .blue/ directory
    pub blue_dir: PathBuf,
    /// Path to .blue/docs/
    pub docs_path: PathBuf,
    /// Path to .blue/blue.db
    pub db_path: PathBuf,
    /// Path to .blue/worktrees/
    pub worktrees_path: PathBuf,
    /// Detected project name (from git remote or directory name)
    pub project_name: Option<String>,
    /// Whether this was migrated from old structure
    pub migrated: bool,
}

impl BlueHome {
    /// Create BlueHome from a root directory
    pub fn new(root: PathBuf) -> Self {
        let blue_dir = root.join(".blue");
        Self {
            docs_path: blue_dir.join("docs"),
            db_path: blue_dir.join("blue.db"),
            worktrees_path: blue_dir.join("worktrees"),
            project_name: extract_project_name(&root),
            migrated: false,
            blue_dir,
            root,
        }
    }

    /// Ensure all required directories exist
    pub fn ensure_dirs(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.blue_dir)?;
        std::fs::create_dir_all(&self.docs_path)?;
        std::fs::create_dir_all(&self.worktrees_path)?;
        Ok(())
    }

    // Legacy compatibility methods - deprecated, will be removed
    #[deprecated(note = "Use docs_path field directly")]
    pub fn docs_path_legacy(&self, _project: &str) -> PathBuf {
        self.docs_path.clone()
    }

    #[deprecated(note = "Use db_path field directly")]
    pub fn db_path_legacy(&self, _project: &str) -> PathBuf {
        self.db_path.clone()
    }
}

/// Information about a git worktree
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// Path to the worktree
    pub path: PathBuf,
    /// Branch name
    pub branch: String,
    /// Whether this is the main worktree
    pub is_main: bool,
}

impl WorktreeInfo {
    /// Extract RFC title from branch name if it follows the pattern rfc/{title}
    pub fn rfc_title(&self) -> Option<String> {
        if self.branch.starts_with("rfc/") {
            Some(self.branch.trim_start_matches("rfc/").to_string())
        } else {
            None
        }
    }
}

/// Repository errors
#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Can't find Blue here. Run 'blue init' first?")]
    NotHome,

    #[error("Git trouble: {0}")]
    Git(#[from] git2::Error),

    #[error("Can't read directory: {0}")]
    Io(#[from] std::io::Error),
}

/// Detect Blue's home directory structure
///
/// RFC 0003: Per-repo .blue/ folders
/// - Finds git repo root for current directory
/// - Creates .blue/ there if it doesn't exist (auto-init)
/// - Migrates from old structure if needed
///
/// The structure is:
/// ```text
/// repo/
/// ├── .blue/
/// │   ├── docs/           # RFCs, spikes, runbooks, etc.
/// │   ├── worktrees/      # Git worktrees
/// │   ├── blue.db         # SQLite database
/// │   └── config.yaml     # Configuration
/// └── src/...
/// ```
pub fn detect_blue(from: &Path) -> Result<BlueHome, RepoError> {
    // First, try to find git repo root
    let root = find_git_root(from).unwrap_or_else(|| {
        debug!("No git repo found, using current directory");
        from.to_path_buf()
    });

    let blue_dir = root.join(".blue");

    // Check for new per-repo structure
    if blue_dir.exists() && blue_dir.is_dir() {
        // Check if this is old structure that needs migration
        let old_repos_path = blue_dir.join("repos");
        let old_data_path = blue_dir.join("data");

        if old_repos_path.exists() || old_data_path.exists() {
            debug!(
                "Found old Blue structure at {:?}, needs migration",
                blue_dir
            );
            return migrate_to_new_structure(&root);
        }

        debug!("Found Blue's home at {:?}", blue_dir);
        return Ok(BlueHome::new(root));
    }

    // Check for legacy .repos/.data/.worktrees at root level
    let legacy_repos = root.join(".repos");
    let legacy_data = root.join(".data");
    if legacy_repos.exists() && legacy_data.exists() {
        debug!("Found legacy Blue structure at {:?}, needs migration", root);
        return migrate_from_legacy_structure(&root);
    }

    // Auto-create .blue/ directory (no `blue init` required per RFC 0003)
    debug!("Creating new Blue home at {:?}", blue_dir);
    let home = BlueHome::new(root);
    home.ensure_dirs().map_err(RepoError::Io)?;
    Ok(home)
}

/// Find the git repository root from a given path
fn find_git_root(from: &Path) -> Option<PathBuf> {
    git2::Repository::discover(from)
        .ok()
        .and_then(|repo| repo.workdir().map(|p| p.to_path_buf()))
}

/// Migrate from old .blue/repos/<project>/docs structure to new .blue/docs structure
fn migrate_to_new_structure(root: &Path) -> Result<BlueHome, RepoError> {
    let blue_dir = root.join(".blue");
    let old_repos_path = blue_dir.join("repos");
    let old_data_path = blue_dir.join("data");
    let new_docs_path = blue_dir.join("docs");
    let new_db_path = blue_dir.join("blue.db");

    // Get project name to find the right subdirectory
    let project_name = extract_project_name(root).unwrap_or_else(|| "default".to_string());

    // Migrate docs: .blue/repos/<project>/docs -> .blue/docs
    let old_project_docs = old_repos_path.join(&project_name).join("docs");
    if old_project_docs.exists() && !new_docs_path.exists() {
        debug!(
            "Migrating docs from {:?} to {:?}",
            old_project_docs, new_docs_path
        );
        std::fs::rename(&old_project_docs, &new_docs_path).map_err(RepoError::Io)?;
    }

    // Migrate database: .blue/data/<project>/blue.db -> .blue/blue.db
    let old_project_db = old_data_path.join(&project_name).join("blue.db");
    if old_project_db.exists() && !new_db_path.exists() {
        debug!(
            "Migrating database from {:?} to {:?}",
            old_project_db, new_db_path
        );
        std::fs::rename(&old_project_db, &new_db_path).map_err(RepoError::Io)?;
    }

    // Clean up empty old directories
    cleanup_empty_dirs(&old_repos_path);
    cleanup_empty_dirs(&old_data_path);

    let mut home = BlueHome::new(root.to_path_buf());
    home.migrated = true;
    home.ensure_dirs().map_err(RepoError::Io)?;

    debug!("Migration complete for {:?}", root);
    Ok(home)
}

/// Migrate from legacy .repos/.data structure at root level
fn migrate_from_legacy_structure(root: &Path) -> Result<BlueHome, RepoError> {
    let legacy_repos = root.join(".repos");
    let legacy_data = root.join(".data");
    let blue_dir = root.join(".blue");

    // Create new .blue directory
    std::fs::create_dir_all(&blue_dir).map_err(RepoError::Io)?;

    let project_name = extract_project_name(root).unwrap_or_else(|| "default".to_string());

    // Migrate docs
    let old_docs = legacy_repos.join(&project_name).join("docs");
    let new_docs = blue_dir.join("docs");
    if old_docs.exists() && !new_docs.exists() {
        debug!(
            "Migrating legacy docs from {:?} to {:?}",
            old_docs, new_docs
        );
        std::fs::rename(&old_docs, &new_docs).map_err(RepoError::Io)?;
    }

    // Migrate database
    let old_db = legacy_data.join(&project_name).join("blue.db");
    let new_db = blue_dir.join("blue.db");
    if old_db.exists() && !new_db.exists() {
        debug!(
            "Migrating legacy database from {:?} to {:?}",
            old_db, new_db
        );
        std::fs::rename(&old_db, &new_db).map_err(RepoError::Io)?;
    }

    // Clean up old directories
    cleanup_empty_dirs(&legacy_repos);
    cleanup_empty_dirs(&legacy_data);

    let mut home = BlueHome::new(root.to_path_buf());
    home.migrated = true;
    home.ensure_dirs().map_err(RepoError::Io)?;

    debug!("Legacy migration complete for {:?}", root);
    Ok(home)
}

/// Recursively remove empty directories
fn cleanup_empty_dirs(path: &Path) {
    if !path.exists() || !path.is_dir() {
        return;
    }

    // Try to remove subdirectories first
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                cleanup_empty_dirs(&entry.path());
            }
        }
    }

    // Try to remove this directory (will fail if not empty, which is fine)
    let _ = std::fs::remove_dir(path);
}

/// Extract project name from git remote or directory name
fn extract_project_name(path: &Path) -> Option<String> {
    // Try git remote first
    if let Ok(repo) = git2::Repository::discover(path) {
        if let Ok(remote) = repo.find_remote("origin") {
            if let Some(url) = remote.url() {
                return extract_repo_name_from_url(url);
            }
        }
    }

    // Fall back to directory name
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Extract repository name from a git URL
fn extract_repo_name_from_url(url: &str) -> Option<String> {
    // Handle SSH URLs: git@host:org/repo.git
    if url.contains(':') && !url.contains("://") {
        let after_colon = url.split(':').next_back()?;
        let name = after_colon.trim_end_matches(".git");
        return name.split('/').next_back().map(|s| s.to_string());
    }

    // Handle HTTPS URLs: https://host/org/repo.git
    let name = url.trim_end_matches(".git");
    name.split('/').next_back().map(|s| s.to_string())
}

/// List git worktrees for a repository
pub fn list_worktrees(repo: &git2::Repository) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();

    // Add main worktree
    if let Some(workdir) = repo.workdir() {
        if let Ok(head) = repo.head() {
            let branch = head
                .shorthand()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "HEAD".to_string());

            worktrees.push(WorktreeInfo {
                path: workdir.to_path_buf(),
                branch,
                is_main: true,
            });
        }
    }

    // Add other worktrees
    if let Ok(wt_names) = repo.worktrees() {
        for name in wt_names.iter().flatten() {
            if let Ok(wt) = repo.find_worktree(name) {
                if let Some(path) = wt.path().to_str() {
                    // Try to get the branch for this worktree
                    let branch = wt.name().unwrap_or("unknown").to_string();

                    worktrees.push(WorktreeInfo {
                        path: PathBuf::from(path),
                        branch,
                        is_main: false,
                    });
                }
            }
        }
    }

    worktrees
}

/// Create a new worktree for an RFC
pub fn create_worktree(
    repo: &git2::Repository,
    branch_name: &str,
    worktree_path: &Path,
) -> Result<(), RepoError> {
    // Derive worktree name from path (directory name = slug, no slashes)
    // Git worktree names are stored in .git/worktrees/<name> and cannot contain slashes
    let worktree_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| RepoError::Git(git2::Error::from_str("Invalid worktree path")))?;

    // Create the branch if it doesn't exist
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;

    let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(branch) => branch,
        Err(_) => repo.branch(branch_name, &head_commit, false)?,
    };

    // Create the worktree
    let reference = branch.into_reference();
    repo.worktree(
        worktree_name,
        worktree_path,
        Some(git2::WorktreeAddOptions::new().reference(Some(&reference))),
    )?;

    Ok(())
}

/// Remove a worktree by path
///
/// Derives the worktree name from the path's directory name.
pub fn remove_worktree(repo: &git2::Repository, worktree_path: &Path) -> Result<(), RepoError> {
    // Derive worktree name from path (same as create_worktree)
    let worktree_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| RepoError::Git(git2::Error::from_str("Invalid worktree path")))?;

    let worktree = repo.find_worktree(worktree_name)?;

    // Prune the worktree (this removes the worktree but keeps the branch)
    worktree.prune(Some(
        git2::WorktreePruneOptions::new()
            .valid(true)
            .working_tree(true),
    ))?;

    Ok(())
}

/// Check if a branch is merged into another
pub fn is_branch_merged(
    repo: &git2::Repository,
    branch: &str,
    into: &str,
) -> Result<bool, RepoError> {
    let branch_commit = repo
        .find_branch(branch, git2::BranchType::Local)?
        .get()
        .peel_to_commit()?
        .id();

    let into_commit = repo
        .find_branch(into, git2::BranchType::Local)?
        .get()
        .peel_to_commit()?
        .id();

    // Check if branch_commit is an ancestor of into_commit
    Ok(repo.graph_descendant_of(into_commit, branch_commit)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_repo_name_ssh() {
        let url = "git@github.com:superviber/blue.git";
        assert_eq!(extract_repo_name_from_url(url), Some("blue".to_string()));
    }

    #[test]
    fn test_extract_repo_name_https() {
        let url = "https://github.com/superviber/blue.git";
        assert_eq!(extract_repo_name_from_url(url), Some("blue".to_string()));
    }

    #[test]
    fn test_worktree_info_rfc_title() {
        let wt = WorktreeInfo {
            path: PathBuf::from("/tmp/test"),
            branch: "rfc/my-feature".to_string(),
            is_main: false,
        };
        assert_eq!(wt.rfc_title(), Some("my-feature".to_string()));

        let main = WorktreeInfo {
            path: PathBuf::from("/tmp/main"),
            branch: "main".to_string(),
            is_main: true,
        };
        assert_eq!(main.rfc_title(), None);
    }
}
