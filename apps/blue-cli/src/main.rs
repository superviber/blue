//! Blue CLI - Welcome home
//!
//! Command-line interface for Blue.

use anyhow::Result;
use blue_core::daemon::{run_daemon, DaemonClient, DaemonDb, DaemonPaths, DaemonState};
use blue_core::realm::RealmService;
use blue_core::tracker::IssueTracker;
use blue_core::ProjectState;
use clap::{Parser, Subcommand};
use serde_json::json;

// ============================================================================
// RFC 0049: Synchronous Guard Command
// ============================================================================
//
// The guard command runs BEFORE tokio runtime initialization to avoid hanging
// issues when invoked from Claude Code hooks. Pre-init gates should not depend
// on post-init infrastructure.

/// Check if this is a guard command and handle it synchronously.
/// Returns Some(exit_code) if handled, None to continue to tokio::main.
fn maybe_handle_guard_sync() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();

    // Quick check: is this a guard command?
    if args.len() >= 2 && args[1] == "guard" {
        // Parse --path=VALUE
        let path = args
            .iter()
            .find(|a| a.starts_with("--path="))
            .map(|a| &a[7..]);

        if let Some(path) = path {
            return Some(run_guard_sync(path));
        }
    }
    None
}

/// Synchronous guard implementation - no tokio, no tracing, just the check.
fn run_guard_sync(path_str: &str) -> i32 {
    use std::path::Path;

    // Check bypass environment variable
    if std::env::var("BLUE_BYPASS_WORKTREE").is_ok() {
        // Note: We skip audit logging in sync mode for simplicity
        return 0; // Allow
    }

    let path = Path::new(path_str);

    // Fast allowlist check
    if is_in_allowlist_sync(path) {
        return 0; // Allow
    }

    // Get cwd
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("guard: failed to get current directory");
            return 1;
        }
    };

    // Check worktree status
    let git_path = cwd.join(".git");

    if git_path.is_file() {
        // This is a worktree (linked worktree has .git as a file)
        if let Ok(content) = std::fs::read_to_string(&git_path) {
            if content.starts_with("gitdir:") {
                let dir_name = cwd.file_name().and_then(|n| n.to_str()).unwrap_or("");

                let parent_is_worktrees = cwd
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|s| s == "worktrees")
                    .unwrap_or(false);

                let is_rfc = dir_name.starts_with("rfc-")
                    || dir_name.starts_with("feature-")
                    || parent_is_worktrees;

                if is_rfc {
                    let abs_path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        cwd.join(path)
                    };
                    if abs_path.starts_with(&cwd) {
                        return 0; // Allow writes in RFC worktree
                    }
                }
            }
        }
        eprintln!(
            "guard: blocked write to {} (not in RFC worktree scope)",
            path.display()
        );
        return 1;
    } else if git_path.is_dir() {
        // Main repository - check branch
        if let Ok(output) = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&cwd)
            .output()
        {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let is_rfc = branch.starts_with("feature/")
                || branch.starts_with("rfc/")
                || branch.starts_with("rfc-");

            if is_rfc {
                return 0; // Allow - on RFC branch
            }
        }

        // Not on RFC branch - check if source code
        if is_source_code_path_sync(path) {
            eprintln!(
                "guard: blocked write to {} (no active worktree)",
                path.display()
            );
            eprintln!("hint: Create a worktree with 'blue worktree create <rfc-title>' first");
            return 1;
        }
        return 0; // Allow non-source-code files
    }

    // No .git - allow (not a git repo)
    0
}

/// Synchronous allowlist check (RFC 0049)
fn is_in_allowlist_sync(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();

    let allowlist = [
        ".blue/docs/",
        ".claude/",
        "/tmp/",
        ".gitignore",
        ".blue/audit/",
    ];

    for pattern in &allowlist {
        if path_str.contains(pattern) {
            return true;
        }
    }

    // Root-level markdown (not in crates/ or src/)
    if path_str.ends_with(".md") && !path_str.contains("crates/") && !path_str.contains("src/") {
        return true;
    }

    // Dialogue temp files
    if path_str.contains("/tmp/blue-dialogue/") {
        return true;
    }

    false
}

/// Synchronous source code path check (RFC 0049)
fn is_source_code_path_sync(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();

    let source_patterns = ["src/", "crates/", "apps/", "lib/", "packages/", "tests/"];
    for pattern in &source_patterns {
        if path_str.contains(pattern) {
            return true;
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let code_extensions = [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h",
        ];
        if code_extensions.contains(&ext) {
            return true;
        }
    }

    false
}

// ============================================================================
// End RFC 0049
// ============================================================================

#[derive(Parser)]
#[command(name = "blue")]
#[command(about = "Welcome home. A development philosophy and toolset.")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Welcome home - initialize Blue in this directory
    Init {
        /// Reinitialize even if .blue/ already exists
        #[arg(long)]
        force: bool,
    },

    /// Get project status
    Status,

    /// What's next?
    Next,

    /// RFC commands
    Rfc {
        #[command(subcommand)]
        command: RfcCommands,
    },

    /// Worktree commands
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },

    /// Create a PR
    Pr {
        #[command(subcommand)]
        command: PrCommands,
    },

    /// Check standards
    Lint,

    /// Come home from alignment/coherence
    Migrate {
        /// Source system
        #[arg(long)]
        from: String,
    },

    /// Run as MCP server
    Mcp {
        /// Enable debug logging to /tmp/blue-mcp-debug.log
        #[arg(long)]
        debug: bool,
    },

    /// Daemon commands
    Daemon {
        #[command(subcommand)]
        command: Option<DaemonCommands>,
    },

    /// Realm commands (cross-repo coordination)
    Realm {
        #[command(subcommand)]
        command: RealmCommands,
    },

    /// Session commands (work coordination)
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Semantic index commands (RFC 0010)
    Index {
        #[command(subcommand)]
        command: IndexCommands,
    },

    /// Search the semantic index
    Search {
        /// Search query
        query: String,

        /// Search symbols only
        #[arg(long)]
        symbols: bool,

        /// Maximum results
        #[arg(long, short, default_value = "10")]
        limit: usize,
    },

    /// Show impact of changing a file
    Impact {
        /// File path to analyze
        file: String,
    },

    /// Context injection visibility (RFC 0016)
    Context {
        #[command(subcommand)]
        command: Option<ContextCommands>,
    },

    /// Guard: Check if file writes are allowed (RFC 0038 PreToolUse hook)
    Guard {
        /// Path to check
        #[arg(long)]
        path: String,

        /// Tool that triggered the check (for audit logging)
        #[arg(long)]
        tool: Option<String>,
    },

    /// Session heartbeat (silent, used by hooks)
    #[command(name = "session-heartbeat")]
    SessionHeartbeat,

    /// Session end (silent, used by hooks)
    #[command(name = "session-end")]
    SessionEnd,

    /// Install Blue for Claude Code (RFC 0052)
    Install {
        /// Only install hooks
        #[arg(long)]
        hooks_only: bool,

        /// Only install skills
        #[arg(long)]
        skills_only: bool,

        /// Only configure MCP server
        #[arg(long)]
        mcp_only: bool,

        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// Remove Blue from Claude Code (RFC 0052)
    Uninstall,

    /// Check Blue installation health (RFC 0052)
    Doctor,

    // ==================== RFC 0057: CLI Parity ====================
    /// Dialogue commands (alignment dialogues)
    Dialogue {
        #[command(subcommand)]
        command: DialogueCommands,
    },

    /// ADR commands (Architecture Decision Records)
    Adr {
        #[command(subcommand)]
        command: AdrCommands,
    },

    /// Spike commands (time-boxed investigations)
    Spike {
        #[command(subcommand)]
        command: SpikeCommands,
    },

    /// Audit commands
    Audit {
        #[command(subcommand)]
        command: AuditCommands,
    },

    /// PRD commands (Product Requirements Documents)
    Prd {
        #[command(subcommand)]
        command: PrdCommands,
    },

    /// Reminder commands
    Reminder {
        #[command(subcommand)]
        command: ReminderCommands,
    },

    /// Delete a document
    Delete {
        /// Document type (rfc, spike, adr, decision, audit, prd, dialogue, postmortem, runbook)
        #[arg(long)]
        doc_type: String,
        /// Document title
        title: String,
        /// Force delete without confirmation
        #[arg(long)]
        force: bool,
        /// Permanently delete (skip trash)
        #[arg(long)]
        permanent: bool,
    },
    /// Restore a deleted document
    Restore {
        /// Document type
        #[arg(long)]
        doc_type: String,
        /// Document title
        title: String,
    },
    /// List deleted documents
    DeletedList {
        /// Filter by document type
        #[arg(long)]
        doc_type: Option<String>,
    },
    /// Purge old deleted documents
    PurgeDeleted {
        /// Days threshold (default 7)
        #[arg(long, default_value = "7")]
        days: u64,
    },

    /// Release management
    Release {
        #[command(subcommand)]
        command: ReleaseCommands,
    },
    /// Post-mortem commands
    Postmortem {
        #[command(subcommand)]
        command: PostmortemCommands,
    },
    /// Runbook commands
    Runbook {
        #[command(subcommand)]
        command: RunbookCommands,
    },
    /// Decision commands
    #[command(name = "decision")]
    Decision {
        #[command(subcommand)]
        command: DecisionCommands,
    },
    /// Staging environment commands
    Staging {
        #[command(subcommand)]
        command: StagingCommands,
    },
    /// Environment commands
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },
    /// Interactive onboarding guide
    Guide {
        /// Action (start, resume, next, skip, reset, status)
        #[arg(default_value = "start")]
        action: String,
        /// Choice for guided steps
        #[arg(long)]
        choice: Option<String>,
    },
    /// Project health check
    HealthCheck,
    /// Sync database with filesystem
    #[command(name = "sync-db")]
    SyncDb,
    /// Notifications
    Notifications {
        #[command(subcommand)]
        command: NotificationCommands,
    },

    /// Jira integration commands (RFC 0063)
    Jira {
        #[command(subcommand)]
        command: JiraCommands,
    },

    /// Org commands — manage orgs and repos (RFC 0067)
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },

    /// Clone a repo into the org-mapped directory (RFC 0067)
    Clone {
        /// Git URL or repo name (if --org is provided)
        url: String,

        /// Org name (for cloning by name instead of URL)
        #[arg(long)]
        org: Option<String>,

        /// Register in a realm after cloning
        #[arg(long)]
        realm: Option<String>,
    },

    /// Sync RFCs to Jira (RFC 0063 — git is authority, Jira is projection)
    Sync {
        /// Jira domain (e.g., myorg.atlassian.net)
        #[arg(long)]
        domain: String,

        /// Jira project key (e.g., BLUE)
        #[arg(long)]
        project: String,

        /// Preview only — don't create/update Jira issues or modify files
        #[arg(long)]
        dry_run: bool,

        /// Drift policy: overwrite, warn (default), or block
        #[arg(long, default_value = "warn")]
        drift_policy: String,
    },
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start the daemon (foreground)
    Start,

    /// Check daemon status
    Status,

    /// Stop the daemon
    Stop,
}

#[derive(Subcommand)]
enum RealmCommands {
    /// Show realm status
    Status,

    /// Sync with realm repository
    Sync {
        /// Force sync even if no changes detected
        #[arg(long)]
        force: bool,
    },

    /// Check realm for CI validation
    Check {
        /// Specific realm to check (default: all)
        #[arg(long)]
        realm: Option<String>,

        /// Exit with error code on warnings
        #[arg(long)]
        strict: bool,
    },

    /// Worktree management for multi-repo RFC work
    Worktree {
        #[command(subcommand)]
        command: RealmWorktreeCommands,
    },

    /// PR workflow for cross-repo changes
    Pr {
        #[command(subcommand)]
        command: RealmPrCommands,
    },

    /// Realm admin commands
    Admin {
        #[command(subcommand)]
        command: RealmAdminCommands,
    },
}

#[derive(Subcommand)]
enum RealmPrCommands {
    /// Show PR status for an RFC across repos
    Status {
        /// RFC name
        #[arg(long)]
        rfc: String,
    },

    /// Prepare changes for PR (commit uncommitted changes)
    Prepare {
        /// RFC name
        #[arg(long)]
        rfc: String,

        /// Commit message
        #[arg(long, short)]
        message: Option<String>,
    },
}

#[derive(Subcommand)]
enum RealmWorktreeCommands {
    /// Create worktrees for an RFC across repos
    Create {
        /// RFC name (becomes branch name)
        #[arg(long)]
        rfc: String,

        /// Specific repos (default: all in realm)
        #[arg(long, value_delimiter = ',')]
        repos: Option<Vec<String>>,
    },

    /// List active worktrees
    List,

    /// Remove worktrees for an RFC
    Remove {
        /// RFC name
        #[arg(long)]
        rfc: String,
    },
}

#[derive(Subcommand)]
enum RealmAdminCommands {
    /// Initialize a new realm
    Init {
        /// Realm name
        #[arg(long)]
        name: String,

        /// Forgejo URL (optional, uses local git if not provided)
        #[arg(long)]
        forgejo: Option<String>,
    },

    /// Join an existing realm
    Join {
        /// Realm name
        name: String,

        /// Repo name (defaults to current directory name)
        #[arg(long)]
        repo: Option<String>,
    },

    /// Create a domain in a realm
    Domain {
        /// Realm name
        #[arg(long)]
        realm: String,

        /// Domain name
        #[arg(long)]
        name: String,

        /// Member repos (comma-separated)
        #[arg(long, value_delimiter = ',')]
        repos: Vec<String>,
    },

    /// Create a contract in a domain
    Contract {
        /// Realm name
        #[arg(long)]
        realm: String,

        /// Domain name
        #[arg(long)]
        domain: String,

        /// Contract name
        #[arg(long)]
        name: String,

        /// Owner repo (the repo that can modify this contract)
        #[arg(long)]
        owner: String,
    },

    /// Create a binding for a repo in a domain
    Binding {
        /// Realm name
        #[arg(long)]
        realm: String,

        /// Domain name
        #[arg(long)]
        domain: String,

        /// Repo name
        #[arg(long)]
        repo: String,

        /// Role: provider, consumer, or both
        #[arg(long, default_value = "consumer")]
        role: String,
    },
}

#[derive(Subcommand)]
enum SessionCommands {
    /// Start a work session
    Start {
        /// RFC being worked on
        #[arg(long)]
        rfc: Option<String>,
    },

    /// List active sessions
    List,

    /// Stop current session
    Stop,

    /// Show session status
    Status,

    /// Record session heartbeat (used by hooks)
    Heartbeat,
}

#[derive(Subcommand)]
enum RfcCommands {
    /// Create a new RFC
    Create {
        /// RFC title
        title: String,

        /// Problem statement
        #[arg(long)]
        problem: Option<String>,

        /// Source spike (links RFC to spike)
        #[arg(long)]
        source_spike: Option<String>,
    },
    /// List all RFCs
    List {
        /// Filter by status (draft, accepted, in-progress, implemented)
        #[arg(long)]
        status: Option<String>,
    },
    /// Get RFC details
    Get {
        /// RFC title
        title: String,
    },
    /// Update RFC status
    Status {
        /// RFC title
        title: String,

        /// New status (draft, accepted, in-progress, implemented, superseded)
        #[arg(long)]
        set: String,
    },
    /// Create a plan for an RFC
    Plan {
        /// RFC title
        title: String,

        /// Tasks (can be specified multiple times)
        #[arg(long, num_args = 1..)]
        task: Vec<String>,
    },
    /// Mark RFC as complete
    Complete {
        /// RFC title
        title: String,
    },
}

#[derive(Subcommand)]
enum WorktreeCommands {
    /// Create a worktree for an RFC
    Create {
        /// RFC title
        title: String,
    },
    /// List worktrees
    List,
    /// Remove a worktree
    Remove {
        /// RFC title
        title: String,
    },
}

#[derive(Subcommand)]
enum PrCommands {
    /// Create a PR
    Create {
        /// PR title
        #[arg(long)]
        title: String,
    },
    /// Verify PR test plan
    Verify {
        /// PR number
        #[arg(long)]
        pr_number: Option<u64>,
    },
    /// Check a test item as verified
    CheckItem {
        /// PR number
        #[arg(long)]
        pr_number: u64,
        /// Item text
        #[arg(long)]
        item: String,
        /// Verified by
        #[arg(long)]
        verified_by: Option<String>,
    },
    /// Check PR approvals
    CheckApprovals {
        /// PR number
        #[arg(long)]
        pr_number: Option<u64>,
    },
    /// Merge PR
    Merge {
        /// PR number
        #[arg(long)]
        pr_number: u64,
        /// Use squash merge
        #[arg(long, default_value = "true")]
        squash: bool,
    },
}

#[derive(Subcommand)]
enum ContextCommands {
    /// Show full manifest with injection status
    Show {
        /// Show complete audit trail with timestamps and hashes
        #[arg(long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum IndexCommands {
    /// Index all files in the realm (bootstrap)
    All {
        /// Specific directory to index
        path: Option<String>,

        /// AI model for indexing (default: qwen2.5:3b)
        #[arg(long)]
        model: Option<String>,
    },

    /// Index staged files (for pre-commit hook)
    Diff {
        /// AI model for indexing
        #[arg(long)]
        model: Option<String>,
    },

    /// Index a specific file
    File {
        /// File path
        path: String,

        /// AI model for indexing
        #[arg(long)]
        model: Option<String>,
    },

    /// Refresh stale index entries
    Refresh {
        /// AI model for indexing
        #[arg(long)]
        model: Option<String>,
    },

    /// Install git pre-commit hook
    InstallHook,

    /// Show index status
    Status,
}

// ==================== RFC 0057: CLI Parity Command Enums ====================

#[derive(Subcommand)]
enum DialogueCommands {
    /// Create a new dialogue
    Create {
        /// Dialogue title
        title: String,

        /// Enable alignment mode with expert panel
        #[arg(long)]
        alignment: bool,

        /// Panel size for alignment mode
        #[arg(long)]
        panel_size: Option<usize>,

        /// Path to expert pool JSON file (required for alignment)
        #[arg(long)]
        expert_pool: Option<String>,

        /// Linked RFC title
        #[arg(long)]
        rfc: Option<String>,

        /// Source files for expert grounding (can be repeated)
        #[arg(long)]
        source: Vec<String>,
    },
    /// Get dialogue details
    Get {
        /// Dialogue title or ID
        title: String,
    },
    /// List all dialogues
    List,
    /// Export dialogue to JSON
    Export {
        /// Dialogue ID
        dialogue_id: String,

        /// Output path (optional)
        #[arg(long)]
        output: Option<String>,
    },
    /// Get a fully-substituted round prompt for an agent
    RoundPrompt {
        /// Output directory for the dialogue
        #[arg(long)]
        output_dir: String,

        /// Agent name (e.g., "Muffin")
        #[arg(long)]
        agent_name: String,

        /// Agent emoji (e.g., "🧁")
        #[arg(long)]
        agent_emoji: String,

        /// Agent role (e.g., "Data Modeling Specialist")
        #[arg(long)]
        agent_role: String,

        /// Round number
        #[arg(long)]
        round: u64,

        /// Source files for expert grounding (can be repeated)
        #[arg(long)]
        source: Vec<String>,

        /// Expert source for graduated rotation (retained, pool, created)
        #[arg(long)]
        expert_source: Option<String>,

        /// Focus for created experts
        #[arg(long)]
        focus: Option<String>,
    },
    /// Evolve the expert panel between rounds
    EvolvePanel {
        /// Output directory for the dialogue
        #[arg(long)]
        output_dir: String,

        /// Round number
        #[arg(long)]
        round: u64,

        /// Panel specification as JSON array
        #[arg(long)]
        panel: String,
    },
    /// Lint a dialogue file
    Lint {
        /// File path
        file: String,
    },
    /// Extract dialogue from JSONL
    Extract {
        /// File path or task ID
        source: String,
    },
    /// Save dialogue
    Save {
        /// Dialogue title
        title: String,
        /// Source file path or task ID
        #[arg(long)]
        source: String,
        /// Summary
        #[arg(long)]
        summary: Option<String>,
        /// Linked RFC title
        #[arg(long)]
        rfc: Option<String>,
    },
    /// Get round context for convergence tracking
    RoundContext {
        /// Dialogue ID
        #[arg(long)]
        dialogue_id: String,
        /// Round number
        #[arg(long)]
        round: u64,
    },
    /// Create a new expert mid-dialogue
    ExpertCreate {
        /// Dialogue ID
        #[arg(long)]
        dialogue_id: String,
        /// Expert slug
        #[arg(long)]
        expert_slug: String,
        /// Expert role
        #[arg(long)]
        role: String,
        /// Expert tier
        #[arg(long)]
        tier: Option<String>,
        /// Reason for creation
        #[arg(long)]
        creation_reason: Option<String>,
        /// First round number
        #[arg(long)]
        first_round: Option<u64>,
    },
    /// Register round data
    RoundRegister {
        /// Dialogue ID
        #[arg(long)]
        dialogue_id: String,
        /// Round number
        #[arg(long)]
        round: u64,
        /// Round data as JSON
        #[arg(long)]
        data: String,
    },
    /// Register verdict
    VerdictRegister {
        /// Dialogue ID
        #[arg(long)]
        dialogue_id: String,
        /// Verdict data as JSON
        #[arg(long)]
        data: String,
    },
    /// Verify round files exist
    RoundVerify {
        /// Output directory for the dialogue
        #[arg(long)]
        output_dir: String,

        /// Round number
        #[arg(long)]
        round: u64,

        /// Agent names as JSON array (e.g., '["Muffin","Cupcake"]')
        #[arg(long)]
        agents: String,
    },
    /// Sample a new panel from the expert pool
    SamplePanel {
        /// Dialogue title
        title: String,

        /// Panel size
        #[arg(long)]
        panel_size: Option<usize>,

        /// Expert roles to retain (can be repeated)
        #[arg(long)]
        retain: Vec<String>,

        /// Expert roles to exclude (can be repeated)
        #[arg(long)]
        exclude: Vec<String>,
    },
}

#[derive(Subcommand)]
enum AdrCommands {
    /// Create a new ADR
    Create {
        /// ADR title
        title: String,
    },
    /// Get ADR details
    Get {
        /// ADR title
        title: String,
    },
    /// List all ADRs
    List,
    /// Update ADR status
    Status {
        /// ADR title
        title: String,

        /// New status (proposed, accepted, deprecated, superseded)
        status: String,
    },
}

#[derive(Subcommand)]
enum SpikeCommands {
    /// Create a new spike
    Create {
        /// Spike title
        title: String,

        /// Time budget in hours
        #[arg(long, default_value = "4")]
        budget: u32,
    },
    /// Get spike details
    Get {
        /// Spike title
        title: String,
    },
    /// List all spikes
    List,
    /// Complete a spike
    Complete {
        /// Spike title
        title: String,

        /// Outcome (success, partial, failure)
        #[arg(long)]
        outcome: String,
    },
}

#[derive(Subcommand)]
enum AuditCommands {
    /// Create a new audit document
    Create {
        /// Audit title
        title: String,
    },
    /// Get audit details
    Get {
        /// Audit title
        title: String,
    },
    /// List all audits
    List,
}

#[derive(Subcommand)]
enum PrdCommands {
    /// Create a new PRD
    Create {
        /// PRD title
        title: String,
    },
    /// Get PRD details
    Get {
        /// PRD title
        title: String,
    },
    /// List all PRDs
    List,
}

#[derive(Subcommand)]
enum ReminderCommands {
    /// Create a new reminder
    Create {
        /// Reminder message
        message: String,

        /// When to remind (e.g., "tomorrow", "2024-03-15")
        #[arg(long)]
        when: String,
    },
    /// List all reminders
    List,
    /// Snooze a reminder
    Snooze {
        /// Reminder ID
        id: i64,

        /// Snooze until (e.g., "1h", "tomorrow")
        #[arg(long)]
        until: String,
    },
    /// Dismiss a reminder
    Dismiss {
        /// Reminder ID
        id: i64,
    },
}

#[derive(Subcommand)]
enum JiraCommands {
    /// Guided setup: walks you through connecting Blue to Jira
    Setup,

    /// Pre-flight validation: check credentials, API access, project
    Doctor {
        /// Jira domain (e.g., myorg.atlassian.net)
        #[arg(long)]
        domain: Option<String>,
    },

    /// Credential management
    Auth {
        #[command(subcommand)]
        command: JiraAuthCommands,
    },

    /// Import issues from Jira as RFC stubs
    Import {
        /// Jira project key (e.g., BLUE)
        #[arg(long)]
        project: String,

        /// Jira domain (e.g., myorg.atlassian.net)
        #[arg(long)]
        domain: String,

        /// Preview only — don't write files
        #[arg(long)]
        dry_run: bool,
    },

    /// Show cross-repo Jira state overview (RFC 0063 Phase 4)
    Status {
        /// Jira domain (e.g., myorg.atlassian.net)
        #[arg(long)]
        domain: String,

        /// Jira project key (e.g., BLUE)
        #[arg(long)]
        project: String,
    },
}

#[derive(Subcommand)]
enum JiraAuthCommands {
    /// Store Jira credentials
    Login {
        /// Jira domain (e.g., myorg.atlassian.net)
        #[arg(long)]
        domain: String,

        /// Atlassian account email
        #[arg(long)]
        email: Option<String>,

        /// API token (reads from stdin if not provided)
        #[arg(long)]
        token: Option<String>,

        /// Store in TOML file instead of keychain
        #[arg(long)]
        toml: bool,
    },

    /// Check credential status
    Status {
        /// Jira domain (if omitted, checks all stored domains)
        #[arg(long)]
        domain: Option<String>,
    },
}

#[derive(Subcommand)]
enum OrgCommands {
    /// List registered orgs and their repos
    List,

    /// Register a new org
    Add {
        /// Org name (e.g., superviber, muffin-labs)
        name: String,

        /// Git provider
        #[arg(long, default_value = "github")]
        provider: String,

        /// Host for Forgejo/self-hosted (e.g., git.example.com)
        #[arg(long)]
        host: Option<String>,
    },

    /// Remove an org
    Remove {
        /// Org name
        name: String,
    },

    /// Scan an org directory for repos
    Scan {
        /// Org name (or "all" to scan all)
        name: String,
    },

    /// Show status: org → repo mapping
    Status,

    /// Show/set blue home directory
    Home {
        /// New home path (if omitted, shows current)
        path: Option<String>,
    },

    /// Migrate repos from flat layout to org-mapped directories
    Migrate {
        /// Directory to scan (default: blue home)
        #[arg(long)]
        from: Option<String>,

        /// Actually move (default: dry-run)
        #[arg(long)]
        execute: bool,
    },
}

// ==================== RFC 0072: New Command Enums ====================

#[derive(Subcommand)]
enum ReleaseCommands {
    /// Create a new release
    Create {
        /// Version (auto-detected if omitted)
        version: Option<String>,
    },
}

#[derive(Subcommand)]
enum PostmortemCommands {
    /// Create a post-mortem
    Create {
        /// Title
        title: String,
        /// Severity (P1-P4)
        #[arg(long)]
        severity: String,
        /// Summary
        #[arg(long)]
        summary: String,
        /// Root cause
        #[arg(long)]
        root_cause: Option<String>,
        /// Duration
        #[arg(long)]
        duration: Option<String>,
        /// Impact
        #[arg(long)]
        impact: Option<String>,
    },
    /// Convert post-mortem action to RFC
    ActionToRfc {
        /// Post-mortem title
        #[arg(long)]
        postmortem: String,
        /// Action description
        #[arg(long)]
        action: String,
        /// RFC title (auto-generated if omitted)
        #[arg(long)]
        rfc_title: Option<String>,
    },
}

#[derive(Subcommand)]
enum RunbookCommands {
    /// Create a runbook
    Create {
        /// Runbook title
        title: String,
        /// Source RFC
        #[arg(long)]
        source_rfc: Option<String>,
        /// Service name
        #[arg(long)]
        service: Option<String>,
        /// Owner
        #[arg(long)]
        owner: Option<String>,
    },
    /// Update a runbook
    Update {
        /// Runbook title
        title: String,
        /// Add operation
        #[arg(long)]
        add_operation: Option<String>,
        /// Add troubleshooting step
        #[arg(long)]
        add_troubleshooting: Option<String>,
    },
    /// Look up runbook by action
    Lookup {
        /// Action to look up
        action: String,
    },
    /// List all registered actions
    Actions,
}

#[derive(Subcommand)]
enum DecisionCommands {
    /// Create a decision record
    Create {
        /// Decision title
        title: String,
        /// The decision
        #[arg(long)]
        decision: String,
        /// Rationale
        #[arg(long)]
        rationale: Option<String>,
    },
}

#[derive(Subcommand)]
enum StagingCommands {
    /// Lock a staging resource
    Lock {
        /// Resource name
        resource: String,
        /// Locked by
        #[arg(long)]
        locked_by: String,
        /// Agent ID
        #[arg(long)]
        agent_id: Option<String>,
        /// Duration in minutes
        #[arg(long, default_value = "60")]
        duration: u64,
    },
    /// Unlock a staging resource
    Unlock {
        /// Resource name
        resource: String,
        /// Locked by
        #[arg(long)]
        locked_by: String,
    },
    /// Show staging status
    Status {
        /// Resource name
        resource: Option<String>,
    },
    /// Clean up expired locks
    Cleanup,
    /// List deployments
    Deployments {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Check for expired
        #[arg(long)]
        check_expired: bool,
    },
    /// Create staging environment
    Create {
        /// Stack name
        #[arg(long)]
        stack: String,
        /// Dry run
        #[arg(long)]
        dry_run: bool,
        /// TTL in hours
        #[arg(long, default_value = "24")]
        ttl_hours: u64,
    },
    /// Destroy staging environment
    Destroy {
        /// Environment name
        name: String,
        /// Dry run
        #[arg(long)]
        dry_run: bool,
    },
    /// Estimate staging costs
    Cost {
        /// Duration in hours
        #[arg(long, default_value = "24")]
        duration_hours: u64,
    },
}

#[derive(Subcommand)]
enum EnvCommands {
    /// Detect external dependencies
    Detect,
    /// Generate isolated environment config
    Mock {
        /// Agent ID
        #[arg(long)]
        agent_id: Option<String>,
        /// Worktree path
        #[arg(long)]
        worktree_path: Option<String>,
    },
}

#[derive(Subcommand)]
enum NotificationCommands {
    /// List notifications
    List {
        /// Filter by state (pending, seen, expired, all)
        #[arg(long, default_value = "pending")]
        state: String,
    },
}

/// Entry point - handles guard synchronously before tokio (RFC 0049)
fn main() {
    // RFC 0049: Handle guard command synchronously before tokio runtime
    if let Some(exit_code) = maybe_handle_guard_sync() {
        std::process::exit(exit_code);
    }

    // Normal path: run tokio runtime
    if let Err(e) = tokio_main() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[tokio::main]
async fn tokio_main() -> Result<()> {
    let cli = Cli::parse();

    // RFC 0020: MCP debug mode logs to file at DEBUG level
    let is_mcp_debug = matches!(&cli.command, Some(Commands::Mcp { debug: true }));
    if is_mcp_debug {
        let log_file = std::fs::File::create("/tmp/blue-mcp-debug.log")?;
        tracing_subscriber::fmt()
            .with_writer(log_file)
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into()),
            )
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into()),
            )
            .init();
    }

    match cli.command {
        None => {
            println!("{}", blue_core::voice::welcome());
        }
        Some(Commands::Status) => match get_project_state() {
            Ok(state) => {
                let args = serde_json::json!({});
                match blue_core::handlers::status::handle_status(&state, &args) {
                    Ok(result) => {
                        let project = result
                            .get("project")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let active = result
                            .get("active")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let ready = result
                            .get("ready")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let stalled = result
                            .get("stalled")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let drafts = result
                            .get("drafts")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let hint = result.get("hint").and_then(|v| v.as_str()).unwrap_or("");

                        println!("Project: {}", project);
                        println!("Active:  {} RFC(s)", active);
                        println!("Ready:   {} RFC(s)", ready);
                        if stalled > 0 {
                            println!("Stalled: {} RFC(s)", stalled);
                        }
                        println!("Drafts:  {} RFC(s)", drafts);
                        println!();
                        println!("{}", hint);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(_) => {
                println!("{}", blue_core::voice::welcome());
                println!();
                println!("Run 'blue init' to get started.");
            }
        },
        Some(Commands::Init { force }) => {
            let cwd = std::env::current_dir()?;
            let blue_dir = cwd.join(".blue");

            // Check if already initialized
            if blue_dir.exists() && !force {
                println!("Blue already initialized in this directory.");
                println!("  {}", blue_dir.display());
                println!();
                println!("Use --force to reinitialize.");
                return Ok(());
            }

            // detect_blue auto-creates .blue/ per RFC 0003
            let home = blue_core::detect_blue(&cwd)
                .map_err(|e| anyhow::anyhow!("Failed to initialize: {}", e))?;

            // Load state to ensure database is created with schema
            let project = home
                .project_name
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let _state = blue_core::ProjectState::load(home.clone(), &project)
                .map_err(|e| anyhow::anyhow!("Failed to create database: {}", e))?;

            println!("{}", blue_core::voice::welcome());
            println!();
            println!("Initialized Blue:");
            println!("  Root:     {}", home.root.display());
            println!("  Database: {}", home.db_path.display());
            println!("  Docs:     {}", home.docs_path.display());

            // RFC 0067: Detect org from git remote and auto-register
            if let Some((org_name, repo_name, provider)) =
                blue_core::detect_org_from_repo(&home.root)
            {
                println!("  Org:      {}/{} [{}]", org_name, repo_name, provider);

                let mut config = blue_core::BlueGlobalConfig::load();
                if config.find_org(&org_name).is_none() {
                    config.add_org(match provider {
                        blue_core::Provider::Github => blue_core::Org::github(&org_name),
                        blue_core::Provider::Forgejo => {
                            blue_core::Org::forgejo(&org_name, "")
                        }
                    });
                    let _ = config.save();
                    println!("  Registered org: {}", org_name);
                }
            }

            // RFC 0067: Offer realm registration
            let realms_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".blue")
                .join("realms");
            let realm_service = blue_core::realm::RealmService::new(realms_path);
            if let Ok(realms) = realm_service.list_realms() {
                if !realms.is_empty() {
                    println!();
                    println!("Available realms: {}", realms.join(", "));
                    eprint!("Join a realm? (name or Enter to skip): ");
                    let mut realm_input = String::new();
                    std::io::stdin().read_line(&mut realm_input).unwrap();
                    let realm_input = realm_input.trim();
                    if !realm_input.is_empty() && realms.contains(&realm_input.to_string()) {
                        let repo_name = home
                            .project_name
                            .as_deref()
                            .unwrap_or("unknown");
                        match realm_service.join_realm(realm_input, repo_name, &home.root) {
                            Ok(()) => println!("Joined realm: {}", realm_input),
                            Err(e) => eprintln!("Failed to join realm: {}", e),
                        }
                    }
                }
            }

            println!();
            println!("Next steps:");
            println!("  blue rfc create \"My First RFC\"");
            println!("  blue status");
        }
        Some(Commands::Next) => {
            let state = get_project_state()?;
            let args = serde_json::json!({});
            match blue_core::handlers::status::handle_next(&state, &args) {
                Ok(result) => {
                    if let Some(recs) = result.get("recommendations").and_then(|v| v.as_array()) {
                        for rec in recs {
                            if let Some(s) = rec.as_str() {
                                println!("{}", s);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Mcp { .. }) => {
            eprintln!("MCP server removed (RFC 0072). Use `blue <command>` directly.");
            std::process::exit(1);
        }
        Some(Commands::Daemon { command }) => {
            handle_daemon_command(command).await?;
        }
        Some(Commands::Realm { command }) => {
            handle_realm_command(command).await?;
        }
        Some(Commands::Session { command }) => {
            handle_session_command(command).await?;
        }
        Some(Commands::Rfc { command }) => {
            handle_rfc_command(command).await?;
        }
        Some(Commands::Worktree { command }) => {
            handle_local_worktree_command(command).await?;
        }
        Some(Commands::Pr { command }) => {
            let state = get_project_state()?;
            match command {
                PrCommands::Create { title } => {
                    println!("Creating PR: {}", title);
                }
                PrCommands::Verify { pr_number } => {
                    let args = json!({ "pr_number": pr_number });
                    match blue_core::handlers::pr::handle_verify(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                PrCommands::CheckItem { pr_number, item, verified_by } => {
                    let args = json!({ "pr_number": pr_number, "item": item, "verified_by": verified_by });
                    match blue_core::handlers::pr::handle_check_item(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                PrCommands::CheckApprovals { pr_number } => {
                    let args = json!({ "pr_number": pr_number });
                    match blue_core::handlers::pr::handle_check_approvals(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                PrCommands::Merge { pr_number, squash } => {
                    let args = json!({ "pr_number": pr_number, "squash": squash });
                    match blue_core::handlers::pr::handle_merge(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        Some(Commands::Lint) => {
            println!("Checking standards.\n");

            // RFC 0063: Check staged files for Jira credential patterns
            let mut warnings_count = 0;

            // Get staged files from git
            let staged = std::process::Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .output();

            if let Ok(output) = staged {
                let files = String::from_utf8_lossy(&output.stdout);
                for file_path in files.lines() {
                    if file_path.is_empty() {
                        continue;
                    }
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        let warnings =
                            blue_core::tracker::lint::check_for_jira_credentials(&content, file_path);
                        for w in &warnings {
                            println!(
                                "  [{}] {}:{} — {}",
                                w.severity, w.file, w.line, w.message
                            );
                            warnings_count += 1;
                        }
                    }
                }
            }

            // Also scan working directory for common credential files
            let suspect_files = [
                ".env",
                "jira-credentials.toml",
                ".jira-cli/config.yml",
            ];
            for suspect in &suspect_files {
                if let Ok(content) = std::fs::read_to_string(suspect) {
                    let warnings =
                        blue_core::tracker::lint::check_for_jira_credentials(&content, suspect);
                    for w in &warnings {
                        println!(
                            "  [{}] {}:{} — {}",
                            w.severity, w.file, w.line, w.message
                        );
                        warnings_count += 1;
                    }
                }
            }

            if warnings_count > 0 {
                println!("\n{} credential warning(s) found.", warnings_count);
                std::process::exit(1);
            } else {
                println!("  [ok] No credential leaks detected.");
            }
        }
        Some(Commands::Migrate { from }) => {
            println!("Coming home from {}.", from);
        }
        Some(Commands::Index { command }) => {
            handle_index_command(command).await?;
        }
        Some(Commands::Search {
            query,
            symbols,
            limit,
        }) => {
            handle_search_command(&query, symbols, limit).await?;
        }
        Some(Commands::Impact { file }) => {
            handle_impact_command(&file).await?;
        }
        Some(Commands::Context { command }) => {
            handle_context_command(command).await?;
        }
        Some(Commands::Guard { path, tool }) => {
            handle_guard_command(&path, tool.as_deref()).await?;
        }
        Some(Commands::SessionHeartbeat) => {
            // Silent heartbeat - touch session file if it exists
            let cwd = std::env::current_dir()?;
            let session_file = cwd.join(".blue").join("session");
            if session_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    let _ = std::fs::write(&session_file, content);
                }
            }
        }
        Some(Commands::SessionEnd) => {
            // Silent session end - remove session file if it exists
            let cwd = std::env::current_dir()?;
            let session_file = cwd.join(".blue").join("session");
            if session_file.exists() {
                let _ = std::fs::remove_file(&session_file);
            }
        }
        Some(Commands::Install {
            hooks_only,
            skills_only,
            mcp_only,
            force,
        }) => {
            handle_install_command(hooks_only, skills_only, mcp_only, force).await?;
        }
        Some(Commands::Uninstall) => {
            handle_uninstall_command().await?;
        }
        Some(Commands::Doctor) => {
            handle_doctor_command().await?;
        }
        // RFC 0057: CLI Parity commands
        Some(Commands::Dialogue { command }) => {
            handle_dialogue_command(command).await?;
        }
        Some(Commands::Adr { command }) => {
            handle_adr_command(command).await?;
        }
        Some(Commands::Spike { command }) => {
            handle_spike_command(command).await?;
        }
        Some(Commands::Audit { command }) => {
            handle_audit_command(command).await?;
        }
        Some(Commands::Prd { command }) => {
            handle_prd_command(command).await?;
        }
        Some(Commands::Reminder { command }) => {
            handle_reminder_command(command).await?;
        }
        // RFC 0072: Document deletion commands
        Some(Commands::Delete { doc_type, title, force, permanent }) => {
            let mut state = get_project_state()?;
            let dt = blue_core::DocType::parse(&doc_type)
                .ok_or_else(|| anyhow::anyhow!("Unknown doc type: {}", doc_type))?;
            match blue_core::handlers::delete::handle_delete(&mut state, dt, &title, force, permanent) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        Some(Commands::Restore { doc_type, title }) => {
            let mut state = get_project_state()?;
            let dt = blue_core::DocType::parse(&doc_type)
                .ok_or_else(|| anyhow::anyhow!("Unknown doc type: {}", doc_type))?;
            match blue_core::handlers::delete::handle_restore(&mut state, dt, &title) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        Some(Commands::DeletedList { doc_type }) => {
            let state = get_project_state()?;
            let dt = doc_type.as_deref().and_then(blue_core::DocType::parse);
            match blue_core::handlers::delete::handle_list_deleted(&state, dt) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        Some(Commands::PurgeDeleted { days }) => {
            let mut state = get_project_state()?;
            match blue_core::handlers::delete::handle_purge_deleted(&mut state, days as i64) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        // RFC 0072: Release commands
        Some(Commands::Release { command }) => {
            let state = get_project_state()?;
            match command {
                ReleaseCommands::Create { version } => {
                    let args = json!({ "version": version });
                    match blue_core::handlers::release::handle_create(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Post-mortem commands
        Some(Commands::Postmortem { command }) => {
            let mut state = get_project_state()?;
            match command {
                PostmortemCommands::Create { title, severity, summary, root_cause, duration, impact } => {
                    let args = json!({
                        "title": title,
                        "severity": severity,
                        "summary": summary,
                        "root_cause": root_cause,
                        "duration": duration,
                        "impact": impact,
                    });
                    match blue_core::handlers::postmortem::handle_create(&mut state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                PostmortemCommands::ActionToRfc { postmortem, action, rfc_title } => {
                    let args = json!({
                        "postmortem_title": postmortem,
                        "action": action,
                        "rfc_title": rfc_title,
                    });
                    match blue_core::handlers::postmortem::handle_action_to_rfc(&mut state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Runbook commands
        Some(Commands::Runbook { command }) => {
            let mut state = get_project_state()?;
            match command {
                RunbookCommands::Create { title, source_rfc, service, owner } => {
                    let args = json!({
                        "title": title,
                        "source_rfc": source_rfc,
                        "service": service,
                        "owner": owner,
                    });
                    match blue_core::handlers::runbook::handle_create(&mut state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                RunbookCommands::Update { title, add_operation, add_troubleshooting } => {
                    let args = json!({
                        "title": title,
                        "add_operation": add_operation,
                        "add_troubleshooting": add_troubleshooting,
                    });
                    match blue_core::handlers::runbook::handle_update(&mut state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                RunbookCommands::Lookup { action } => {
                    let args = json!({ "action": action });
                    match blue_core::handlers::runbook::handle_lookup(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                RunbookCommands::Actions => {
                    match blue_core::handlers::runbook::handle_actions(&state) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Decision commands
        Some(Commands::Decision { command }) => {
            let state = get_project_state()?;
            match command {
                DecisionCommands::Create { title, decision, rationale } => {
                    let args = json!({
                        "title": title,
                        "decision": decision,
                        "rationale": rationale,
                    });
                    match blue_core::handlers::decision::handle_create(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Staging commands
        Some(Commands::Staging { command }) => {
            match command {
                StagingCommands::Lock { resource, locked_by, agent_id, duration } => {
                    let state = get_project_state()?;
                    let args = json!({
                        "resource": resource,
                        "locked_by": locked_by,
                        "agent_id": agent_id,
                        "duration_minutes": duration,
                    });
                    match blue_core::handlers::staging::handle_lock(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Unlock { resource, locked_by } => {
                    let state = get_project_state()?;
                    let args = json!({ "resource": resource, "locked_by": locked_by });
                    match blue_core::handlers::staging::handle_unlock(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Status { resource } => {
                    let state = get_project_state()?;
                    let args = json!({ "resource": resource });
                    match blue_core::handlers::staging::handle_status(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Cleanup => {
                    let state = get_project_state()?;
                    let args = json!({});
                    match blue_core::handlers::staging::handle_cleanup(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Deployments { status, check_expired } => {
                    let state = get_project_state()?;
                    let args = json!({ "status": status, "check_expired": check_expired });
                    match blue_core::handlers::staging::handle_deployments(&state, &args) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Create { stack, dry_run, ttl_hours } => {
                    let cwd = std::env::current_dir()?;
                    let args = json!({ "stack": stack, "dry_run": dry_run, "ttl_hours": ttl_hours });
                    match blue_core::handlers::staging::handle_create(&args, &cwd) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Destroy { name, dry_run } => {
                    let cwd = std::env::current_dir()?;
                    let args = json!({ "name": name, "dry_run": dry_run });
                    match blue_core::handlers::staging::handle_destroy(&args, &cwd) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                StagingCommands::Cost { duration_hours } => {
                    let cwd = std::env::current_dir()?;
                    let args = json!({ "duration_hours": duration_hours });
                    match blue_core::handlers::staging::handle_cost(&args, &cwd) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Environment commands
        Some(Commands::Env { command }) => {
            let cwd = std::env::current_dir()?;
            match command {
                EnvCommands::Detect => {
                    let args = json!({});
                    match blue_core::handlers::env::handle_detect(&args, &cwd) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
                EnvCommands::Mock { agent_id, worktree_path } => {
                    let args = json!({ "agent_id": agent_id, "worktree_path": worktree_path });
                    match blue_core::handlers::env::handle_mock(&args, &cwd) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        // RFC 0072: Guide command
        Some(Commands::Guide { action, choice }) => {
            let cwd = std::env::current_dir()?;
            let blue_path = cwd.join(".blue");
            let args = json!({ "action": action, "choice": choice });
            match blue_core::handlers::guide::handle_guide(&args, &blue_path) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        // RFC 0072: Health check
        Some(Commands::HealthCheck) => {
            let state = get_project_state()?;
            let args = json!({});
            match blue_core::handlers::status::handle_status(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        // RFC 0072: Sync database with filesystem
        Some(Commands::SyncDb) => {
            let cwd = std::env::current_dir()?;
            println!("Syncing database with filesystem...");
            let args = json!({});
            match blue_core::handlers::lint::handle_lint(&args, &cwd) {
                Ok(result) => {
                    println!("Sync complete.");
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        // RFC 0072: Notifications
        Some(Commands::Notifications { command }) => {
            match command {
                NotificationCommands::List { state } => {
                    let cwd = std::env::current_dir()?;
                    let filter = if state == "all" { None } else { Some(state.as_str()) };
                    match blue_core::handlers::realm::handle_notifications_list(Some(&cwd), filter) {
                        Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                        Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
                    }
                }
            }
        }
        Some(Commands::Jira { command }) => {
            handle_jira_command(command).await?;
        }
        Some(Commands::Org { command }) => {
            handle_org_command(command)?;
        }
        Some(Commands::Clone { url, org, realm }) => {
            handle_clone_command(&url, org.as_deref(), realm.as_deref())?;
        }
        Some(Commands::Sync {
            domain,
            project,
            dry_run,
            drift_policy,
        }) => {
            let domain = domain.clone();
            let project = project.clone();
            let drift_policy = drift_policy.clone();
            let dry_run = dry_run;
            tokio::task::spawn_blocking(move || {
                handle_sync_command(&domain, &project, dry_run, &drift_policy)
            })
            .await??;
        }
    }

    Ok(())
}

async fn handle_daemon_command(command: Option<DaemonCommands>) -> Result<()> {
    match command {
        None | Some(DaemonCommands::Start) => {
            // Start daemon in foreground
            let paths = DaemonPaths::new()?;
            paths.ensure_dirs()?;

            let db = DaemonDb::open(&paths.database)?;
            let state = DaemonState::new(db, paths);

            println!("Starting Blue daemon on localhost:7865...");
            run_daemon(state).await?;
        }
        Some(DaemonCommands::Status) => {
            let client = DaemonClient::new();
            match client.health().await {
                Ok(health) => {
                    println!("Daemon running. Version: {}", health.version);

                    // Show active sessions
                    if let Ok(sessions) = client.list_sessions().await {
                        if !sessions.is_empty() {
                            println!("\nActive sessions:");
                            for session in sessions {
                                println!("  {} ({}) - {}", session.repo, session.realm, session.id);
                            }
                        }
                    }

                    // Show tracked realms
                    if let Ok(realms) = client.list_realms().await {
                        if !realms.is_empty() {
                            println!("\nTracked realms:");
                            for realm in realms {
                                println!("  {} - {}", realm.name, realm.forgejo_url);
                            }
                        }
                    }
                }
                Err(_) => {
                    println!("Daemon not running.");
                }
            }
        }
        Some(DaemonCommands::Stop) => {
            // TODO: Implement graceful shutdown
            println!("Stopping daemon not yet implemented.");
        }
    }
    Ok(())
}

async fn handle_realm_command(command: RealmCommands) -> Result<()> {
    let client = DaemonClient::new();

    // Ensure daemon is running for all realm commands
    client.ensure_running().await?;

    match command {
        RealmCommands::Status => {
            let paths = DaemonPaths::new()?;
            let service = RealmService::new(paths.realms.clone());
            let realm_names = service.list_realms()?;

            if realm_names.is_empty() {
                println!("No realms configured. Run 'blue realm admin init' to create one.");
                return Ok(());
            }

            let sessions = client.list_sessions().await.unwrap_or_default();
            let notifications = client.list_notifications().await.unwrap_or_default();

            for realm_name in &realm_names {
                // Load detailed realm info
                match service.load_realm_details(realm_name) {
                    Ok(details) => {
                        println!("Realm: {}", details.info.name);
                        println!("  Path: {}", details.info.path.display());
                        println!("  Version: {}", details.info.config.version);

                        // Repos
                        if !details.repos.is_empty() {
                            println!("\n  Repos:");
                            for repo in &details.repos {
                                let path_info = repo.path.as_deref().unwrap_or("remote");
                                println!("    {} ({})", repo.name, path_info);
                            }
                        }

                        // Domains
                        if !details.domains.is_empty() {
                            println!("\n  Domains:");
                            for domain_detail in &details.domains {
                                let d = &domain_detail.domain;
                                println!("    {} ({} members)", d.name, d.members.len());

                                // Contracts in domain
                                for contract in &domain_detail.contracts {
                                    println!(
                                        "      Contract: {} v{} (owner: {})",
                                        contract.name, contract.version, contract.owner
                                    );
                                }

                                // Bindings in domain
                                for binding in &domain_detail.bindings {
                                    let exports = binding.exports.len();
                                    let imports = binding.imports.len();
                                    println!(
                                        "      Binding: {} ({:?}, {} exports, {} imports)",
                                        binding.repo, binding.role, exports, imports
                                    );
                                }
                            }
                        }

                        // Sessions in this realm
                        let realm_sessions: Vec<_> =
                            sessions.iter().filter(|s| s.realm == *realm_name).collect();
                        if !realm_sessions.is_empty() {
                            println!("\n  Active sessions:");
                            for s in realm_sessions {
                                let rfc = s.active_rfc.as_deref().unwrap_or("idle");
                                println!("    {} - {}", s.repo, rfc);
                            }
                        }

                        // Notifications in this realm
                        let realm_notifs: Vec<_> = notifications
                            .iter()
                            .filter(|n| n.realm == *realm_name)
                            .collect();
                        if !realm_notifs.is_empty() {
                            println!("\n  Notifications:");
                            for n in realm_notifs {
                                println!(
                                    "    [{:?}] {} updated {} in {}",
                                    n.change_type, n.from_repo, n.contract, n.domain
                                );
                            }
                        }
                    }
                    Err(e) => {
                        println!("Realm: {} (error: {})", realm_name, e);
                    }
                }
                println!();
            }
        }
        RealmCommands::Sync { force } => {
            let paths = DaemonPaths::new()?;
            let service = RealmService::new(paths.realms.clone());
            let realm_names = service.list_realms()?;

            if realm_names.is_empty() {
                println!("No realms configured.");
                return Ok(());
            }

            for realm_name in &realm_names {
                // First show status
                match service.realm_sync_status(realm_name) {
                    Ok(status) if status.has_changes() => {
                        println!("Realm '{}' has pending changes:", realm_name);
                        for f in &status.new_files {
                            println!("  + {}", f);
                        }
                        for f in &status.modified_files {
                            println!("  ~ {}", f);
                        }
                        for f in &status.deleted_files {
                            println!("  - {}", f);
                        }
                    }
                    Ok(_) => {
                        println!("Realm '{}' is clean.", realm_name);
                    }
                    Err(e) => {
                        println!("Realm '{}': error getting status: {}", realm_name, e);
                        continue;
                    }
                }

                // Sync
                println!("Syncing realm '{}'...", realm_name);
                match service.sync_realm(realm_name, force) {
                    Ok(result) => {
                        println!("  {}", result.message);
                        if let Some(commit) = result.last_commit {
                            println!("  Latest: {}", commit);
                        }
                    }
                    Err(e) => {
                        println!("  Error: {}", e);
                    }
                }
            }
        }
        RealmCommands::Check { realm, strict } => {
            let paths = DaemonPaths::new()?;
            let service = RealmService::new(paths.realms.clone());

            let realm_names = match realm {
                Some(name) => vec![name],
                None => service.list_realms()?,
            };

            if realm_names.is_empty() {
                println!("No realms configured.");
                return Ok(());
            }

            let mut has_errors = false;
            let mut has_warnings = false;

            for realm_name in &realm_names {
                println!("Checking realm '{}'...", realm_name);

                match service.check_realm(realm_name) {
                    Ok(result) => {
                        if result.is_ok() && !result.has_warnings() {
                            println!("  All checks passed.");
                        }

                        for warning in &result.warnings {
                            has_warnings = true;
                            println!("  WARNING [{}]: {}", warning.domain, warning.message);
                        }

                        for error in &result.errors {
                            has_errors = true;
                            println!("  ERROR [{}]: {}", error.domain, error.message);
                        }
                    }
                    Err(e) => {
                        has_errors = true;
                        println!("  Error checking realm: {}", e);
                    }
                }
            }

            if has_errors || (strict && has_warnings) {
                std::process::exit(1);
            }
        }
        RealmCommands::Worktree { command } => {
            handle_worktree_command(command).await?;
        }
        RealmCommands::Pr { command } => {
            handle_realm_pr_command(command).await?;
        }
        RealmCommands::Admin { command } => {
            handle_realm_admin_command(command, &client).await?;
        }
    }
    Ok(())
}

async fn handle_worktree_command(command: RealmWorktreeCommands) -> Result<()> {
    use blue_core::realm::LocalRepoConfig;

    let paths = DaemonPaths::new()?;
    let service = RealmService::new(paths.realms.clone());

    match command {
        RealmWorktreeCommands::Create { rfc, repos } => {
            // Get current directory and check for .blue/config.yaml
            let cwd = std::env::current_dir()?;
            let config_path = cwd.join(".blue").join("config.yaml");

            if !config_path.exists() {
                println!("This repo is not part of a realm.");
                println!("Run 'blue realm admin join <realm>' first.");
                return Ok(());
            }

            let local_config = LocalRepoConfig::load(&config_path)?;
            let realm_name = &local_config.realm.name;

            // Get repos to create worktrees for
            let details = service.load_realm_details(realm_name)?;
            let repo_names: Vec<String> = match repos {
                Some(r) => r,
                None => details.repos.iter().map(|r| r.name.clone()).collect(),
            };

            if repo_names.is_empty() {
                println!("No repos found in realm '{}'.", realm_name);
                return Ok(());
            }

            println!(
                "Creating worktrees for RFC '{}' in realm '{}'...",
                rfc, realm_name
            );

            for repo in &details.repos {
                if !repo_names.contains(&repo.name) {
                    continue;
                }

                // RFC 0067: Use org-relative resolution with fallback chain
                let repo_path = match service.resolve_repo_path(repo) {
                    Some(p) => p,
                    None => {
                        println!("  {} - skipped (no local path)", repo.name);
                        continue;
                    }
                };

                match service.create_worktree(realm_name, &repo.name, &rfc, &repo_path) {
                    Ok(info) => {
                        if info.already_existed {
                            println!(
                                "  {} - already exists at {}",
                                info.repo,
                                info.path.display()
                            );
                        } else {
                            println!("  {} - created at {}", info.repo, info.path.display());
                        }
                    }
                    Err(e) => {
                        println!("  {} - error: {}", repo.name, e);
                    }
                }
            }
        }

        RealmWorktreeCommands::List => {
            let realm_names = service.list_realms()?;

            if realm_names.is_empty() {
                println!("No realms configured.");
                return Ok(());
            }

            let mut found_any = false;
            for realm_name in &realm_names {
                let worktrees = service.list_worktrees(realm_name)?;
                if !worktrees.is_empty() {
                    found_any = true;
                    println!("Realm '{}' worktrees:", realm_name);
                    for wt in worktrees {
                        println!("  {} ({}) - {}", wt.rfc, wt.repo, wt.path.display());
                    }
                }
            }

            if !found_any {
                println!("No active worktrees.");
            }
        }

        RealmWorktreeCommands::Remove { rfc } => {
            // Get current realm from config
            let cwd = std::env::current_dir()?;
            let config_path = cwd.join(".blue").join("config.yaml");

            let realm_name = if config_path.exists() {
                let local_config = LocalRepoConfig::load(&config_path)?;
                local_config.realm.name
            } else {
                // Try to find any realm with this RFC worktree
                let realm_names = service.list_realms()?;
                let mut found_realm = None;
                for name in &realm_names {
                    let worktrees = service.list_worktrees(name)?;
                    if worktrees.iter().any(|wt| wt.rfc == rfc) {
                        found_realm = Some(name.clone());
                        break;
                    }
                }
                match found_realm {
                    Some(name) => name,
                    None => {
                        println!("No worktrees found for RFC '{}'.", rfc);
                        return Ok(());
                    }
                }
            };

            println!(
                "Removing worktrees for RFC '{}' in realm '{}'...",
                rfc, realm_name
            );

            match service.remove_worktrees(&realm_name, &rfc) {
                Ok(removed) => {
                    if removed.is_empty() {
                        println!("  No worktrees found.");
                    } else {
                        for repo in removed {
                            println!("  {} - removed", repo);
                        }
                    }
                }
                Err(e) => {
                    println!("  Error: {}", e);
                }
            }
        }
    }

    Ok(())
}

async fn handle_realm_pr_command(command: RealmPrCommands) -> Result<()> {
    use blue_core::realm::LocalRepoConfig;

    let paths = DaemonPaths::new()?;
    let service = RealmService::new(paths.realms.clone());

    // Get realm from current directory config or find from worktrees
    let get_realm_name = |rfc: &str| -> Result<String> {
        let cwd = std::env::current_dir()?;
        let config_path = cwd.join(".blue").join("config.yaml");

        if config_path.exists() {
            let local_config = LocalRepoConfig::load(&config_path)?;
            return Ok(local_config.realm.name);
        }

        // Try to find realm from worktrees
        let realm_names = service.list_realms()?;
        for name in &realm_names {
            let worktrees = service.list_worktrees(name)?;
            if worktrees.iter().any(|wt| wt.rfc == rfc) {
                return Ok(name.clone());
            }
        }

        anyhow::bail!("No realm found for RFC '{}'", rfc);
    };

    match command {
        RealmPrCommands::Status { rfc } => {
            let realm_name = get_realm_name(&rfc)?;
            let statuses = service.pr_status(&realm_name, &rfc)?;

            if statuses.is_empty() {
                println!(
                    "No worktrees found for RFC '{}' in realm '{}'.",
                    rfc, realm_name
                );
                println!("Run 'blue realm worktree create --rfc {}' first.", rfc);
                return Ok(());
            }

            println!("PR status for RFC '{}' in realm '{}':\n", rfc, realm_name);

            for status in &statuses {
                let icon = if status.has_uncommitted { "!" } else { "✓" };
                println!(
                    "{} {} (branch: {}, {} commits ahead)",
                    icon, status.repo, status.branch, status.commits_ahead
                );
                println!("    Path: {}", status.path.display());

                if status.has_uncommitted {
                    println!("    Uncommitted changes:");
                    for file in &status.modified_files {
                        println!("      - {}", file);
                    }
                }
            }

            // Summary
            let uncommitted_count = statuses.iter().filter(|s| s.has_uncommitted).count();
            let total_commits: usize = statuses.iter().map(|s| s.commits_ahead).sum();

            println!("\nSummary:");
            println!("  {} repos with worktrees", statuses.len());
            println!("  {} repos with uncommitted changes", uncommitted_count);
            println!("  {} total commits ahead of main", total_commits);

            if uncommitted_count > 0 {
                println!(
                    "\nRun 'blue realm pr prepare --rfc {}' to commit changes.",
                    rfc
                );
            }
        }

        RealmPrCommands::Prepare { rfc, message } => {
            let realm_name = get_realm_name(&rfc)?;
            let msg = message.as_deref();

            println!(
                "Preparing PR for RFC '{}' in realm '{}'...\n",
                rfc, realm_name
            );

            let results = service.pr_prepare(&realm_name, &rfc, msg)?;

            if results.is_empty() {
                println!("No worktrees found for RFC '{}'.", rfc);
                return Ok(());
            }

            for (repo, committed) in &results {
                if *committed {
                    println!("  {} - changes committed", repo);
                } else {
                    println!("  {} - no changes to commit", repo);
                }
            }

            let committed_count = results.iter().filter(|(_, c)| *c).count();
            println!("\n{} repos had changes committed.", committed_count);
        }
    }

    Ok(())
}

async fn handle_realm_admin_command(
    command: RealmAdminCommands,
    _client: &DaemonClient,
) -> Result<()> {
    let paths = DaemonPaths::new()?;
    paths.ensure_dirs()?;
    let service = RealmService::new(paths.realms.clone());

    match command {
        RealmAdminCommands::Init { name, forgejo } => {
            // Create realm locally
            let info = service.init_realm(&name)?;

            // Register with daemon
            let realm = service.to_daemon_realm(&info);

            // For now, directly update the daemon's database
            // In the future, this would go through the daemon API
            let db = DaemonDb::open(&paths.database)?;
            db.upsert_realm(&realm)?;

            println!("Created realm '{}'", name);
            println!("  Path: {}", info.path.display());
            if let Some(url) = forgejo {
                println!("  Forgejo: {} (push deferred - remote down)", url);
            } else {
                println!("  Mode: local git");
            }
            println!(
                "\nNext: Run 'blue realm admin join {}' in your repos.",
                name
            );
        }

        RealmAdminCommands::Join { name, repo } => {
            // Get current directory
            let cwd = std::env::current_dir()?;

            // Determine repo name
            let repo_name = repo.unwrap_or_else(|| {
                cwd.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

            // Join the realm
            service.join_realm(&name, &repo_name, &cwd)?;

            println!("Joined realm '{}'", name);
            println!("  Repo: {}", repo_name);
            println!("  Config: {}/.blue/config.yaml", cwd.display());
        }

        RealmAdminCommands::Domain { realm, name, repos } => {
            // Create domain
            service.create_domain(&realm, &name, &repos)?;

            println!("Created domain '{}' in realm '{}'", name, realm);
            println!("  Members: {}", repos.join(", "));
            println!("\nNext: Create contracts and bindings for this domain.");
        }

        RealmAdminCommands::Contract {
            realm,
            domain,
            name,
            owner,
        } => {
            service.create_contract(&realm, &domain, &name, &owner)?;

            println!("Created contract '{}' in domain '{}'", name, domain);
            println!("  Owner: {}", owner);
            println!("  Version: 1.0.0");
            println!("\nNext: Create bindings to export/import this contract.");
        }

        RealmAdminCommands::Binding {
            realm,
            domain,
            repo,
            role,
        } => {
            use blue_core::realm::BindingRole;

            let binding_role = match role.to_lowercase().as_str() {
                "provider" => BindingRole::Provider,
                "consumer" => BindingRole::Consumer,
                "both" => BindingRole::Both,
                _ => {
                    println!("Invalid role '{}'. Use: provider, consumer, or both.", role);
                    return Ok(());
                }
            };

            service.create_binding(&realm, &domain, &repo, binding_role)?;

            println!("Created binding for '{}' in domain '{}'", repo, domain);
            println!("  Role: {:?}", binding_role);
            println!("\nNext: Run 'blue realm check' to validate the configuration.");
        }
    }

    Ok(())
}

async fn handle_session_command(command: SessionCommands) -> Result<()> {
    use blue_core::daemon::CreateSessionRequest;
    use blue_core::realm::LocalRepoConfig;

    let client = DaemonClient::new();
    client.ensure_running().await?;

    match command {
        SessionCommands::Start { rfc } => {
            // Get current directory and check for .blue/config.yaml
            let cwd = std::env::current_dir()?;
            let config_path = cwd.join(".blue").join("config.yaml");

            if !config_path.exists() {
                println!("This repo is not part of a realm.");
                println!("Run 'blue realm admin join <realm>' first.");
                return Ok(());
            }

            // Load local config to get realm and repo info
            let local_config = LocalRepoConfig::load(&config_path)?;

            // Generate session ID
            let session_id = format!(
                "{}-{}-{}",
                local_config.repo,
                std::process::id(),
                chrono::Utc::now().timestamp()
            );

            // Create session
            let req = CreateSessionRequest {
                id: session_id.clone(),
                repo: local_config.repo.clone(),
                realm: local_config.realm.name.clone(),
                client_id: Some(format!("cli-{}", std::process::id())),
                active_rfc: rfc.clone(),
                active_domains: Vec::new(),
                exports_modified: Vec::new(),
                imports_watching: Vec::new(),
            };

            let session = client.create_session(req).await?;
            println!("Session started: {}", session.id);
            println!("  Repo: {}", session.repo);
            println!("  Realm: {}", session.realm);
            if let Some(rfc) = &session.active_rfc {
                println!("  RFC: {}", rfc);
            }

            // Save session ID to .blue/session
            let session_file = cwd.join(".blue").join("session");
            std::fs::write(&session_file, &session.id)?;
            println!("\nSession ID saved to .blue/session");
        }

        SessionCommands::List => {
            let sessions = client.list_sessions().await?;

            if sessions.is_empty() {
                println!("No active sessions.");
            } else {
                println!("Active sessions:");
                for s in sessions {
                    let rfc = s.active_rfc.as_deref().unwrap_or("idle");
                    println!("  {} ({}/{}) - {}", s.id, s.realm, s.repo, rfc);
                }
            }
        }

        SessionCommands::Stop => {
            // Try to read session ID from .blue/session
            let cwd = std::env::current_dir()?;
            let session_file = cwd.join(".blue").join("session");

            if !session_file.exists() {
                println!("No active session in this repo.");
                return Ok(());
            }

            let session_id = std::fs::read_to_string(&session_file)?;
            let session_id = session_id.trim();

            client.remove_session(session_id).await?;
            std::fs::remove_file(&session_file)?;

            println!("Session stopped: {}", session_id);
        }

        SessionCommands::Status => {
            // Check for local session
            let cwd = std::env::current_dir()?;
            let session_file = cwd.join(".blue").join("session");

            if session_file.exists() {
                let session_id = std::fs::read_to_string(&session_file)?;
                let session_id = session_id.trim();
                println!("Current session: {}", session_id);
            } else {
                println!("No active session in this repo.");
            }

            // List all sessions
            let sessions = client.list_sessions().await?;
            if !sessions.is_empty() {
                println!("\nAll active sessions:");
                for s in sessions {
                    let rfc = s.active_rfc.as_deref().unwrap_or("idle");
                    println!("  {} ({}/{}) - {}", s.id, s.realm, s.repo, rfc);
                }
            }

            // Check for notifications
            let notifications = client.list_notifications().await?;
            if !notifications.is_empty() {
                println!("\nPending notifications:");
                for n in notifications {
                    println!(
                        "  [{:?}] {} updated {} in {}",
                        n.change_type, n.from_repo, n.contract, n.domain
                    );
                }
            }
        }

        SessionCommands::Heartbeat => {
            // Silent heartbeat - just touch the session file to update activity
            let cwd = std::env::current_dir()?;
            let session_file = cwd.join(".blue").join("session");

            if session_file.exists() {
                // Touch file by reading and writing back (updates mtime)
                if let Ok(content) = std::fs::read_to_string(&session_file) {
                    let _ = std::fs::write(&session_file, content);
                }
            }
            // Silent success - no output for hooks
        }
    }

    Ok(())
}

// ==================== Semantic Index Commands (RFC 0010) ====================

async fn handle_index_command(command: IndexCommands) -> Result<()> {
    // Run the blocking indexer operations in a separate thread
    // to avoid runtime conflicts with reqwest::blocking::Client
    tokio::task::spawn_blocking(move || handle_index_command_blocking(command)).await??;
    Ok(())
}

fn handle_index_command_blocking(command: IndexCommands) -> Result<()> {
    use blue_core::store::DocumentStore;
    use blue_core::is_indexable_file;
    use std::path::Path;

    // Get the .blue database path
    let cwd = std::env::current_dir()?;
    let db_path = cwd.join(".blue").join("blue.db");

    if !db_path.exists() {
        println!("No .blue directory found. Run 'blue init' first.");
        return Ok(());
    }

    let store = DocumentStore::open(&db_path)?;

    match command {
        IndexCommands::All { path, model: _ } => {
            let target_path = path.as_deref().unwrap_or(".");

            // Collect all indexable files
            let files = collect_indexable_files(Path::new(target_path))?;
            println!("Found {} indexable files in '{}'", files.len(), target_path);
            println!("\nEmbedded LLM indexing has been removed (RFC 0071).");
            println!("Use 'blue index status' to check existing index state.");
        }

        IndexCommands::Diff { model: _ } => {
            // Get staged files
            let output = std::process::Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .output()?;

            let staged_files: Vec<String> = std::str::from_utf8(&output.stdout)?
                .lines()
                .filter(|l| !l.is_empty())
                .filter(|l| is_indexable_file(Path::new(l)))
                .map(|s| s.to_string())
                .collect();

            if staged_files.is_empty() {
                println!("No indexable staged files.");
                return Ok(());
            }

            println!("{} indexable staged file(s) detected.", staged_files.len());
            println!("Embedded LLM indexing has been removed (RFC 0071).");
        }

        IndexCommands::File { path, model: _ } => {
            let file_path = Path::new(&path);

            if !file_path.exists() {
                println!("File not found: {}", path);
                return Ok(());
            }

            println!("Embedded LLM indexing has been removed (RFC 0071).");
            println!("File '{}' exists and is indexable: {}", path, is_indexable_file(file_path));
        }

        IndexCommands::Refresh { model: _ } => {
            let realm = "default";

            let (file_count, symbol_count) = store.get_index_stats(realm)?;
            println!(
                "Current index: {} files, {} symbols",
                file_count, symbol_count
            );

            if file_count == 0 {
                println!("Index is empty.");
                return Ok(());
            }

            // Get all indexed files and check which are stale
            let indexed_files = store.list_file_index(realm, None)?;
            let mut stale_count = 0;

            for entry in &indexed_files {
                let path = Path::new(&entry.file_path);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let current_hash = hash_file_content(&content);
                        if current_hash != entry.file_hash {
                            stale_count += 1;
                        }
                    }
                }
            }

            if stale_count == 0 {
                println!("All indexed files are up to date.");
            } else {
                println!("Found {} stale file(s).", stale_count);
                println!("Embedded LLM re-indexing has been removed (RFC 0071).");
            }
        }

        IndexCommands::InstallHook => {
            let hook_path = cwd.join(".git").join("hooks").join("pre-commit");

            if !cwd.join(".git").exists() {
                println!("Not a git repository.");
                return Ok(());
            }

            let hook_content = r#"#!/bin/sh
# Blue semantic index pre-commit hook
# Indexes staged files before commit

blue index diff 2>/dev/null || true
"#;

            std::fs::write(&hook_path, hook_content)?;

            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&hook_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&hook_path, perms)?;
            }

            println!("Installed pre-commit hook at {}", hook_path.display());
            println!("Staged files will be indexed on each commit.");
        }

        IndexCommands::Status => {
            let realm = "default";

            let (file_count, symbol_count) = store.get_index_stats(realm)?;

            println!("Index status:");
            println!("  Indexed files: {}", file_count);
            println!("  Indexed symbols: {}", symbol_count);

            if file_count == 0 {
                println!("\nIndex is empty. Run 'blue index all' to bootstrap.");
            }
        }
    }

    Ok(())
}

/// Collect all indexable files in a directory
fn collect_indexable_files(dir: &std::path::Path) -> Result<Vec<String>> {
    use blue_core::{is_indexable_file, should_skip_dir};
    use std::fs;

    let mut files = Vec::new();

    fn walk_dir(dir: &std::path::Path, files: &mut Vec<String>) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if path.is_dir() {
                if !should_skip_dir(name) {
                    walk_dir(&path, files)?;
                }
            } else if is_indexable_file(&path) {
                if let Some(s) = path.to_str() {
                    files.push(s.to_string());
                }
            }
        }
        Ok(())
    }

    walk_dir(dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// Hash file content for staleness detection
fn hash_file_content(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

async fn handle_search_command(query: &str, symbols_only: bool, limit: usize) -> Result<()> {
    use blue_core::store::DocumentStore;

    let cwd = std::env::current_dir()?;
    let db_path = cwd.join(".blue").join("blue.db");

    if !db_path.exists() {
        println!("No .blue directory found. Run 'blue init' first.");
        return Ok(());
    }

    let store = DocumentStore::open(&db_path)?;
    let realm = "default";

    if symbols_only {
        let results = store.search_symbols(realm, query, limit)?;

        if results.is_empty() {
            println!("No symbols found matching '{}'.", query);
            return Ok(());
        }

        println!("Symbols matching '{}':\n", query);
        for (symbol, file) in results {
            let lines = match (symbol.start_line, symbol.end_line) {
                (Some(s), Some(e)) => format!(":{}-{}", s, e),
                (Some(s), None) => format!(":{}", s),
                _ => String::new(),
            };
            println!(
                "  {} ({}) - {}{}",
                symbol.name, symbol.kind, file.file_path, lines
            );
            if let Some(desc) = &symbol.description {
                println!("    {}", desc);
            }
        }
    } else {
        let results = store.search_file_index(realm, query, limit)?;

        if results.is_empty() {
            println!("No files found matching '{}'.", query);
            return Ok(());
        }

        println!("Files matching '{}':\n", query);
        for result in results {
            println!("  {}", result.file_entry.file_path);
            if let Some(summary) = &result.file_entry.summary {
                println!("    {}", summary);
            }
        }
    }

    Ok(())
}

async fn handle_impact_command(file: &str) -> Result<()> {
    use blue_core::store::DocumentStore;

    let cwd = std::env::current_dir()?;
    let db_path = cwd.join(".blue").join("blue.db");

    if !db_path.exists() {
        println!("No .blue directory found. Run 'blue init' first.");
        return Ok(());
    }

    let store = DocumentStore::open(&db_path)?;
    let realm = "default";

    // Get file entry
    let file_entry = store.get_file_index(realm, realm, file)?;

    match file_entry {
        Some(entry) => {
            println!("Impact analysis for: {}\n", file);

            if let Some(summary) = &entry.summary {
                println!("Summary: {}\n", summary);
            }

            if let Some(relationships) = &entry.relationships {
                println!("Relationships:\n{}\n", relationships);
            }

            // Get symbols
            if let Some(id) = entry.id {
                let symbols = store.get_file_symbols(id)?;
                if !symbols.is_empty() {
                    println!("Symbols ({}):", symbols.len());
                    for sym in symbols {
                        let lines = match (sym.start_line, sym.end_line) {
                            (Some(s), Some(e)) => format!("lines {}-{}", s, e),
                            (Some(s), None) => format!("line {}", s),
                            _ => String::new(),
                        };
                        println!("  {} ({}) {}", sym.name, sym.kind, lines);
                    }
                }
            }

            // Search for files that reference this file
            println!("\nSearching for files that reference this file...");
            let filename = std::path::Path::new(file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);

            let references = store.search_file_index(realm, filename, 20)?;
            let references: Vec<_> = references
                .into_iter()
                .filter(|r| r.file_entry.file_path != file)
                .collect();

            if references.is_empty() {
                println!("No files found referencing this file.");
            } else {
                println!("\nFiles that may reference '{}':", file);
                for r in references {
                    println!("  {}", r.file_entry.file_path);
                }
            }
        }
        None => {
            println!("File '{}' is not indexed.", file);
            println!("Run 'blue index file {}' to index it.", file);
        }
    }

    Ok(())
}

// ==================== Context Commands (RFC 0016) ====================

async fn handle_context_command(command: Option<ContextCommands>) -> Result<()> {
    use blue_core::ContextManifest;

    let cwd = std::env::current_dir()?;
    let blue_dir = cwd.join(".blue");

    if !blue_dir.exists() {
        println!("No .blue directory found. Run 'blue init' first.");
        return Ok(());
    }

    let manifest = ContextManifest::load_or_default(&cwd)?;

    match command {
        None => {
            // Quick summary (default)
            let resolution = manifest.resolve(&cwd)?;
            print_context_summary(&resolution);
        }
        Some(ContextCommands::Show { verbose }) => {
            // Full manifest view
            let resolution = manifest.resolve(&cwd)?;

            if verbose {
                print_context_verbose(&manifest, &resolution);
            } else {
                print_context_show(&manifest, &resolution);
            }
        }
    }

    Ok(())
}

fn print_context_summary(resolution: &blue_core::ManifestResolution) {
    fn format_tokens(tokens: usize) -> String {
        if tokens >= 1000 {
            format!("{:.1}k", tokens as f64 / 1000.0)
        } else {
            format!("{}", tokens)
        }
    }

    println!(
        "Identity: {} sources ({} tokens) | Workflow: {} sources ({} tokens)",
        resolution.identity.source_count,
        format_tokens(resolution.identity.token_count),
        resolution.workflow.source_count,
        format_tokens(resolution.workflow.token_count),
    );
}

fn print_context_show(
    manifest: &blue_core::ContextManifest,
    resolution: &blue_core::ManifestResolution,
) {
    println!("Context Manifest (v{})", manifest.version);
    println!();

    // Identity tier
    println!("Identity Tier (always injected)");
    println!("  Budget: {} tokens", manifest.identity.max_tokens);
    println!("  Actual: {} tokens", resolution.identity.token_count);
    for source in &resolution.identity.sources {
        let label = source.label.as_deref().unwrap_or("");
        let status = if source.file_count > 0 { "✓" } else { "○" };
        println!(
            "  {} {} ({} files, {} tokens)",
            status, source.uri, source.file_count, source.tokens
        );
        if !label.is_empty() {
            println!("      {}", label);
        }
    }
    println!();

    // Workflow tier
    println!("Workflow Tier (activity-triggered)");
    println!("  Budget: {} tokens", manifest.workflow.max_tokens);
    println!("  Actual: {} tokens", resolution.workflow.token_count);
    for source in &resolution.workflow.sources {
        let label = source.label.as_deref().unwrap_or("");
        let status = if source.file_count > 0 { "✓" } else { "○" };
        println!(
            "  {} {} ({} files, {} tokens)",
            status, source.uri, source.file_count, source.tokens
        );
        if !label.is_empty() {
            println!("      {}", label);
        }
    }

    // Triggers
    if !manifest.workflow.refresh_triggers.is_empty() {
        println!("  Triggers:");
        for trigger in &manifest.workflow.refresh_triggers {
            let name = match trigger {
                blue_core::RefreshTrigger::OnRfcChange => "on_rfc_change".to_string(),
                blue_core::RefreshTrigger::EveryNTurns(n) => format!("every_{}_turns", n),
                blue_core::RefreshTrigger::OnToolCall(tool) => format!("on_tool_call({})", tool),
            };
            println!("    - {}", name);
        }
    }
    println!();

    // Reference tier
    println!("Reference Tier (on-demand via MCP)");
    println!("  Budget: {} tokens", manifest.reference.max_tokens);
    println!("  Staleness: {} days", manifest.reference.staleness_days);
    if let Some(graph) = &manifest.reference.graph {
        println!("  Graph: {}", graph);
    }
    println!();

    // Plugins
    if !manifest.plugins.is_empty() {
        println!("Plugins:");
        for plugin in &manifest.plugins {
            println!("  - {}", plugin.uri);
            if !plugin.provides.is_empty() {
                println!("    Provides: {}", plugin.provides.join(", "));
            }
        }
    }
}

fn print_context_verbose(
    manifest: &blue_core::ContextManifest,
    resolution: &blue_core::ManifestResolution,
) {
    // Print the regular show output first
    print_context_show(manifest, resolution);

    // Add verbose details
    println!("=== Audit Details ===");
    println!();

    if let Some(generated) = &manifest.generated_at {
        println!("Generated: {}", generated);
    }
    if let Some(commit) = &manifest.source_commit {
        println!("Source commit: {}", commit);
    }

    println!();
    println!("Content Hashes:");
    for source in &resolution.identity.sources {
        println!("  {} -> {}", source.uri, source.content_hash);
    }
    for source in &resolution.workflow.sources {
        println!("  {} -> {}", source.uri, source.content_hash);
    }

    // Try to show recent injection history from the database
    let cwd = std::env::current_dir().ok();
    if let Some(cwd) = cwd {
        let db_path = cwd.join(".blue").join("blue.db");
        if db_path.exists() {
            if let Ok(store) = blue_core::DocumentStore::open(&db_path) {
                if let Ok(recent) = store.get_recent_injections(10) {
                    if !recent.is_empty() {
                        println!();
                        println!("Recent Injections:");
                        for inj in recent {
                            println!(
                                "  {} | {} | {} | {} tokens",
                                inj.timestamp,
                                inj.tier,
                                inj.source_uri,
                                inj.token_count.unwrap_or(0)
                            );
                        }
                    }
                }
            }
        }
    }
}

// ==================== Guard Command (RFC 0038) ====================

/// Check if file write is allowed based on worktree and allowlist rules.
///
/// Exit codes:
/// - 0: Allow the write
/// - 1: Block the write (not in valid worktree and not in allowlist)
async fn handle_guard_command(path: &str, tool: Option<&str>) -> Result<()> {
    use std::path::Path;

    // Check for bypass environment variable
    if std::env::var("BLUE_BYPASS_WORKTREE").is_ok() {
        // Log bypass for audit
        log_guard_bypass(path, tool, "BLUE_BYPASS_WORKTREE env set");
        return Ok(()); // Exit 0 = allow
    }

    let path = Path::new(path);

    // Check allowlist patterns first (fast path)
    if is_in_allowlist(path) {
        return Ok(()); // Exit 0 = allow
    }

    // Get current working directory
    let cwd = std::env::current_dir()?;

    // Check if we're in a git worktree
    let worktree_info = get_worktree_info(&cwd)?;

    match worktree_info {
        Some(info) => {
            // We're in a worktree - check if it's associated with an RFC
            if info.is_rfc_worktree {
                // Check if the path is inside this worktree
                let abs_path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    cwd.join(path)
                };

                if abs_path.starts_with(&info.worktree_path) {
                    return Ok(()); // Exit 0 = allow writes in RFC worktree
                }
            }
            // Not in allowlist and not in RFC worktree scope
            eprintln!(
                "guard: blocked write to {} (not in RFC worktree scope)",
                path.display()
            );
            std::process::exit(1);
        }
        None => {
            // Not in a worktree - check if there's an active RFC that might apply
            // For now, block writes to source code outside worktrees
            if is_source_code_path(path) {
                eprintln!(
                    "guard: blocked write to {} (no active worktree)",
                    path.display()
                );
                eprintln!("hint: Create a worktree with 'blue worktree create <rfc-title>' first");
                std::process::exit(1);
            }
            // Non-source-code files are allowed
            Ok(())
        }
    }
}

/// Allowlist patterns for files that can always be written
fn is_in_allowlist(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();

    // Always-allowed patterns
    let allowlist = [
        ".blue/docs/",  // Blue documentation
        ".claude/",     // Claude configuration
        "/tmp/",        // Temp files
        "*.md",         // Markdown at root (but not in crates/)
        ".gitignore",   // Git config
        ".blue/audit/", // Audit logs
    ];

    for pattern in &allowlist {
        if pattern.starts_with("*.") {
            // Extension pattern - check only root level
            let ext = &pattern[1..];
            if path_str.ends_with(ext)
                && !path_str.contains("crates/")
                && !path_str.contains("src/")
            {
                return true;
            }
        } else if path_str.contains(pattern) {
            return true;
        }
    }

    // Check for dialogue temp files
    if path_str.contains("/tmp/blue-dialogue/") {
        return true;
    }

    false
}

/// Check if a path looks like source code
fn is_source_code_path(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();

    // Source code indicators
    let source_patterns = ["src/", "crates/", "apps/", "lib/", "packages/", "tests/"];

    for pattern in &source_patterns {
        if path_str.contains(pattern) {
            return true;
        }
    }

    // Check file extensions
    if let Some(ext) = path.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) {
        let code_extensions = [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h",
        ];
        if code_extensions.contains(&ext) {
            return true;
        }
    }

    false
}

struct WorktreeInfo {
    worktree_path: std::path::PathBuf,
    is_rfc_worktree: bool,
}

/// Get information about the current git worktree
fn get_worktree_info(cwd: &std::path::Path) -> Result<Option<WorktreeInfo>> {
    // Check if we're in a git worktree by looking at .git file
    let git_path = cwd.join(".git");

    if git_path.is_file() {
        // This is a worktree (linked worktree has .git as a file)
        let content = std::fs::read_to_string(&git_path)?;
        if content.starts_with("gitdir:") {
            // Parse the worktree path
            let worktree_path = cwd.to_path_buf();

            // Check if this looks like an RFC worktree
            // RFC worktrees are typically named feature/<rfc-slug> or rfc/<rfc-slug>
            let dir_name = cwd
                .file_name()
                .and_then(|n: &std::ffi::OsStr| n.to_str())
                .unwrap_or("");

            let parent_is_worktrees = cwd
                .parent()
                .and_then(|p: &std::path::Path| p.file_name())
                .and_then(|n: &std::ffi::OsStr| n.to_str())
                .map(|s: &str| s == "worktrees")
                .unwrap_or(false);

            let is_rfc = dir_name.starts_with("rfc-")
                || dir_name.starts_with("feature-")
                || parent_is_worktrees;

            return Ok(Some(WorktreeInfo {
                worktree_path,
                is_rfc_worktree: is_rfc,
            }));
        }
    } else if git_path.is_dir() {
        // Main repository - check if we're on an RFC branch
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(cwd)
            .output();

        if let Ok(output) = output {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let is_rfc = branch.starts_with("feature/")
                || branch.starts_with("rfc/")
                || branch.starts_with("rfc-");

            return Ok(Some(WorktreeInfo {
                worktree_path: cwd.to_path_buf(),
                is_rfc_worktree: is_rfc,
            }));
        }
    }

    Ok(None)
}

/// Log a guard bypass for audit trail
fn log_guard_bypass(path: &str, tool: Option<&str>, reason: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => return,
    };

    let audit_dir = cwd.join(".blue").join("audit");
    if std::fs::create_dir_all(&audit_dir).is_err() {
        return;
    }

    let log_path = audit_dir.join("guard-bypass.log");
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let tool_str = tool.unwrap_or("unknown");
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

    let entry = format!(
        "{} | {} | {} | {} | {}\n",
        timestamp, user, tool_str, path, reason
    );

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let _ = file.write_all(entry.as_bytes());
    }
}

// ============================================================================
// RFC 0052: Blue Install Command
// ============================================================================

const SESSION_START_HOOK: &str = r#"#!/bin/bash
# Managed by: blue install
# Blue SessionStart hook - sets up PATH for Claude Code

if [ -n "$CLAUDE_ENV_FILE" ] && [ -n "$CLAUDE_PROJECT_DIR" ]; then
  echo "export PATH=\"\$CLAUDE_PROJECT_DIR/target/release:\$PATH\"" >> "$CLAUDE_ENV_FILE"
fi

exit 0
"#;

const GUARD_WRITE_HOOK: &str = r#"#!/bin/bash
# Managed by: blue install
# Blue PreToolUse hook - enforces RFC 0038 worktree protection

# Read stdin with bash timeout (portable, no GNU timeout needed)
INPUT=""
while IFS= read -t 2 -r line; do
    INPUT="${INPUT}${line}"
done

if [ -z "$INPUT" ]; then
    exit 0
fi

FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null || echo "")

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

blue guard --path="$FILE_PATH"
"#;

async fn handle_install_command(
    hooks_only: bool,
    skills_only: bool,
    _mcp_only: bool,
    force: bool,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    println!("Installing Blue for Claude Code...\n");

    let install_all = !hooks_only && !skills_only;

    // Install hooks
    if install_all || hooks_only {
        println!("Hooks:");
        install_hooks(&cwd, force)?;
    }

    // Install skills
    if install_all || skills_only {
        println!("\nSkills:");
        install_skills(&cwd, &home)?;
    }

    // RFC 0072: Clean up legacy MCP config if present
    if install_all {
        cleanup_legacy_mcp(&home)?;
    }

    println!("\nBlue installed. Restart Claude Code to activate.");
    Ok(())
}

fn install_hooks(project_dir: &std::path::Path, force: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let hooks_dir = project_dir.join(".claude").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    // Write session-start.sh
    let session_start_path = hooks_dir.join("session-start.sh");
    if !session_start_path.exists() || force {
        std::fs::write(&session_start_path, SESSION_START_HOOK)?;
        std::fs::set_permissions(&session_start_path, std::fs::Permissions::from_mode(0o755))?;
        println!("  ✓ .claude/hooks/session-start.sh");
    } else {
        println!("  - .claude/hooks/session-start.sh (exists, use --force to overwrite)");
    }

    // Write guard-write.sh
    let guard_write_path = hooks_dir.join("guard-write.sh");
    if !guard_write_path.exists() || force {
        std::fs::write(&guard_write_path, GUARD_WRITE_HOOK)?;
        std::fs::set_permissions(&guard_write_path, std::fs::Permissions::from_mode(0o755))?;
        println!("  ✓ .claude/hooks/guard-write.sh");
    } else {
        println!("  - .claude/hooks/guard-write.sh (exists, use --force to overwrite)");
    }

    // Update settings.json
    let settings_path = project_dir.join(".claude").join("settings.json");
    let settings = merge_hook_settings(&settings_path)?;
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    println!("  ✓ .claude/settings.json (merged)");

    Ok(())
}

fn merge_hook_settings(settings_path: &std::path::Path) -> Result<serde_json::Value> {
    use serde_json::json;

    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(settings_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = json!({});
    }

    // Add SessionStart hook
    settings["hooks"]["SessionStart"] = json!([
        {
            "hooks": [
                {
                    "type": "command",
                    "command": ".claude/hooks/session-start.sh"
                }
            ]
        }
    ]);

    // Add PreToolUse hook
    settings["hooks"]["PreToolUse"] = json!([
        {
            "matcher": "Write|Edit|MultiEdit",
            "hooks": [
                {
                    "type": "command",
                    "command": ".claude/hooks/guard-write.sh"
                }
            ]
        }
    ]);

    Ok(settings)
}

fn install_skills(project_dir: &std::path::Path, home: &std::path::Path) -> Result<()> {
    let skills_dir = project_dir.join("skills");
    let target_dir = home.join(".claude").join("skills");

    std::fs::create_dir_all(&target_dir)?;

    if !skills_dir.exists() {
        println!("  - No skills directory found");
        return Ok(());
    }

    for entry in std::fs::read_dir(&skills_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let skill_name = entry.file_name();
            let link_path = target_dir.join(&skill_name);

            // Remove existing symlink if present
            if link_path.exists() || link_path.symlink_metadata().is_ok() {
                std::fs::remove_file(&link_path).ok();
            }

            // Create symlink
            std::os::unix::fs::symlink(&path, &link_path)?;
            println!(
                "  ✓ ~/.claude/skills/{} -> {}",
                skill_name.to_string_lossy(),
                path.display()
            );
        }
    }

    Ok(())
}

/// RFC 0072: Remove stale MCP server config from ~/.claude.json
fn cleanup_legacy_mcp(home: &std::path::Path) -> Result<()> {
    let config_path = home.join(".claude.json");

    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = config.get_mut("mcpServers") {
                    if let Some(obj) = servers.as_object_mut() {
                        if obj.remove("blue").is_some() {
                            std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                            println!("\nLegacy MCP:");
                            println!("  ✓ Removed stale blue MCP server from ~/.claude.json (RFC 0072)");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_uninstall_command() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    println!("Removing Blue from Claude Code...\n");

    // Remove hooks
    println!("Hooks:");
    uninstall_hooks(&cwd)?;

    // Remove skills
    println!("\nSkills:");
    uninstall_skills(&cwd, &home)?;

    // RFC 0072: Clean up any legacy MCP config
    cleanup_legacy_mcp(&home)?;

    println!("\nBlue uninstalled.");
    Ok(())
}

fn uninstall_hooks(project_dir: &std::path::Path) -> Result<()> {
    let hooks_dir = project_dir.join(".claude").join("hooks");

    // Remove hook scripts
    let session_start = hooks_dir.join("session-start.sh");
    if session_start.exists() {
        // Check if managed by blue
        if let Ok(content) = std::fs::read_to_string(&session_start) {
            if content.contains("Managed by: blue install") {
                std::fs::remove_file(&session_start)?;
                println!("  ✓ Removed .claude/hooks/session-start.sh");
            } else {
                println!("  - .claude/hooks/session-start.sh (not managed by blue, skipped)");
            }
        }
    }

    let guard_write = hooks_dir.join("guard-write.sh");
    if guard_write.exists() {
        if let Ok(content) = std::fs::read_to_string(&guard_write) {
            if content.contains("Managed by: blue install") {
                std::fs::remove_file(&guard_write)?;
                println!("  ✓ Removed .claude/hooks/guard-write.sh");
            } else {
                println!("  - .claude/hooks/guard-write.sh (not managed by blue, skipped)");
            }
        }
    }

    // Clean settings.json
    let settings_path = project_dir.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(hooks) = settings.get_mut("hooks") {
                    if let Some(obj) = hooks.as_object_mut() {
                        obj.remove("SessionStart");
                        obj.remove("PreToolUse");
                    }
                }
                std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
                println!("  ✓ Cleaned .claude/settings.json");
            }
        }
    }

    Ok(())
}

fn uninstall_skills(project_dir: &std::path::Path, home: &std::path::Path) -> Result<()> {
    let skills_dir = project_dir.join("skills");
    let target_dir = home.join(".claude").join("skills");

    if !skills_dir.exists() {
        println!("  - No skills to remove");
        return Ok(());
    }

    for entry in std::fs::read_dir(&skills_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let skill_name = entry.file_name();
            let link_path = target_dir.join(&skill_name);

            if link_path.symlink_metadata().is_ok() {
                std::fs::remove_file(&link_path)?;
                println!(
                    "  ✓ Removed ~/.claude/skills/{}",
                    skill_name.to_string_lossy()
                );
            }
        }
    }

    Ok(())
}


async fn handle_doctor_command() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let cwd = std::env::current_dir()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    println!("Blue Installation Health Check\n");

    let mut issues = 0;

    // Check binary
    println!("Binary:");
    let binary_path = which::which("blue").ok();
    if let Some(ref path) = binary_path {
        println!("  ✓ blue found at {}", path.display());

        // RFC 0060: macOS-specific signature and liveness checks
        #[cfg(target_os = "macos")]
        {
            // Check for stale provenance xattr
            let xattr_output = std::process::Command::new("xattr")
                .arg("-l")
                .arg(path)
                .output();

            if let Ok(output) = xattr_output {
                let attrs = String::from_utf8_lossy(&output.stdout);
                if attrs.contains("com.apple.provenance") {
                    println!("  ⚠ com.apple.provenance xattr present (may cause hangs)");
                    println!(
                        "    hint: xattr -cr {} && codesign --force --sign - {}",
                        path.display(),
                        path.display()
                    );
                    issues += 1;
                }
            }

            // Check code signature validity
            let codesign_output = std::process::Command::new("codesign")
                .args(["--verify", "--verbose"])
                .arg(path)
                .output();

            if let Ok(output) = codesign_output {
                if output.status.success() {
                    println!("  ✓ Code signature valid");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if stderr.contains("invalid signature") || stderr.contains("modified") {
                        println!("  ✗ Code signature invalid or stale");
                        println!("    hint: codesign --force --sign - {}", path.display());
                        issues += 1;
                    } else if stderr.contains("not signed") {
                        println!("  - Binary not signed (may be fine)");
                    }
                }
            }

            // Liveness check with timeout
            use std::time::Duration;
            let liveness = std::process::Command::new(path)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    // Wait up to 3 seconds
                    let start = std::time::Instant::now();
                    loop {
                        match child.try_wait() {
                            Ok(Some(status)) => return Ok(status.success()),
                            Ok(None) => {
                                if start.elapsed() > Duration::from_secs(3) {
                                    let _ = child.kill();
                                    return Ok(false);
                                }
                                std::thread::sleep(Duration::from_millis(50));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                });

            match liveness {
                Ok(true) => println!("  ✓ Binary responds within timeout"),
                Ok(false) => {
                    println!("  ✗ Binary hangs (dyld signature issue)");
                    println!("    hint: cargo install --path apps/blue-cli --force");
                    issues += 1;
                }
                Err(e) => println!("  ⚠ Could not run liveness check: {}", e),
            }
        }
    } else {
        println!("  ✗ blue not found in PATH");
        issues += 1;
    }

    // Check hooks
    println!("\nHooks:");
    let hooks_dir = cwd.join(".claude").join("hooks");

    let session_start = hooks_dir.join("session-start.sh");
    if session_start.exists() {
        let is_executable = std::fs::metadata(&session_start)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if is_executable {
            println!("  ✓ session-start.sh (installed, executable)");
        } else {
            println!("  ✗ session-start.sh (not executable)");
            issues += 1;
        }
    } else {
        println!("  ✗ session-start.sh missing");
        issues += 1;
    }

    let guard_write = hooks_dir.join("guard-write.sh");
    if guard_write.exists() {
        let is_executable = std::fs::metadata(&guard_write)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        if is_executable {
            println!("  ✓ guard-write.sh (installed, executable)");
        } else {
            println!("  ✗ guard-write.sh (not executable)");
            issues += 1;
        }
    } else {
        println!("  ✗ guard-write.sh missing");
        issues += 1;
    }

    let settings_path = cwd.join(".claude").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if content.contains("SessionStart") && content.contains("PreToolUse") {
                println!("  ✓ settings.json configured");
            } else {
                println!("  ✗ settings.json missing hook configuration");
                issues += 1;
            }
        }
    } else {
        println!("  ✗ settings.json missing");
        issues += 1;
    }

    // Check skills
    println!("\nSkills:");
    let skills_dir = cwd.join("skills");
    let target_dir = home.join(".claude").join("skills");

    if skills_dir.exists() {
        for entry in std::fs::read_dir(&skills_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let skill_name = entry.file_name();
                let link_path = target_dir.join(&skill_name);

                if link_path.symlink_metadata().is_ok() {
                    // Check if symlink points to correct target
                    if let Ok(target) = std::fs::read_link(&link_path) {
                        if target == entry.path() {
                            println!("  ✓ {} (symlink valid)", skill_name.to_string_lossy());
                        } else {
                            println!(
                                "  ✗ {} (symlink points to wrong target)",
                                skill_name.to_string_lossy()
                            );
                            issues += 1;
                        }
                    }
                } else {
                    println!("  ✗ {} (symlink missing)", skill_name.to_string_lossy());
                    issues += 1;
                }
            }
        }
    } else {
        println!("  - No skills directory");
    }

    // RFC 0072: Check for stale MCP config
    let config_path = home.join(".claude.json");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = config.get("mcpServers") {
                    if servers.get("blue").is_some() {
                        println!("\nLegacy:");
                        println!("  ⚠ Stale blue MCP server in ~/.claude.json (RFC 0072)");
                        println!("    hint: Run `blue install` to clean up");
                        issues += 1;
                    }
                }
            }
        }
    }

    // Summary
    println!();
    if issues == 0 {
        println!("All checks passed.");
    } else {
        println!("{} issue(s) found. Run `blue install` to fix.", issues);
    }

    Ok(())
}

// ==================== RFC 0057: CLI Parity Handlers ====================

/// Get or create project state for CLI commands
fn get_project_state() -> Result<ProjectState> {
    let cwd = std::env::current_dir()?;
    let home =
        blue_core::detect_blue(&cwd).map_err(|e| anyhow::anyhow!("Not a Blue project: {}", e))?;
    let project = home
        .project_name
        .clone()
        .unwrap_or_else(|| "default".to_string());
    ProjectState::load(home, &project)
        .map_err(|e| anyhow::anyhow!("Failed to load project state: {}", e))
}

/// Handle dialogue subcommands
async fn handle_dialogue_command(command: DialogueCommands) -> Result<()> {
    let mut state = get_project_state()?;

    match command {
        DialogueCommands::Create {
            title,
            alignment,
            panel_size,
            expert_pool,
            rfc,
            source,
        } => {
            // Load expert pool from JSON file if provided
            let pool_value: Option<serde_json::Value> = if let Some(ref path) = expert_pool {
                let content = std::fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Failed to read expert pool file: {}", e))?;
                Some(serde_json::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Invalid expert pool JSON: {}", e))?)
            } else {
                None
            };

            let mut args = json!({
                "title": title,
                "alignment": alignment,
                "panel_size": panel_size,
            });
            if let Some(pool) = pool_value {
                args["expert_pool"] = pool;
            }
            if let Some(ref rfc_title) = rfc {
                args["rfc_title"] = json!(rfc_title);
            }
            if !source.is_empty() {
                args["sources"] = json!(source);
            }
            match blue_core::handlers::dialogue::handle_create(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                    if let Some(file) = result
                        .get("dialogue")
                        .and_then(|d| d.get("file"))
                        .and_then(|v| v.as_str())
                    {
                        println!("File: {}", file);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::Get { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::dialogue::handle_get(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::List => {
            let args = json!({});
            match blue_core::handlers::dialogue::handle_list(&state, &args) {
                Ok(result) => {
                    if let Some(dialogues) = result.get("dialogues").and_then(|v| v.as_array()) {
                        if dialogues.is_empty() {
                            println!("No dialogues found.");
                        } else {
                            for d in dialogues {
                                let title = d.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                                let status =
                                    d.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                                println!("  {} [{}]", title, status);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::Export {
            dialogue_id,
            output,
        } => {
            let mut args = json!({ "dialogue_id": dialogue_id });
            if let Some(path) = output {
                args["output_path"] = json!(path);
            }
            match blue_core::handlers::dialogue::handle_export(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::RoundPrompt {
            output_dir,
            agent_name,
            agent_emoji,
            agent_role,
            round,
            source,
            expert_source,
            focus,
        } => {
            let mut args = json!({
                "output_dir": output_dir,
                "agent_name": agent_name,
                "agent_emoji": agent_emoji,
                "agent_role": agent_role,
                "round": round,
            });
            if !source.is_empty() {
                args["sources"] = json!(source);
            }
            if let Some(ref es) = expert_source {
                args["expert_source"] = json!(es);
            }
            if let Some(ref f) = focus {
                args["focus"] = json!(f);
            }
            match blue_core::handlers::dialogue::handle_round_prompt(&args) {
                Ok(result) => {
                    // Print the prompt to stdout (primary output)
                    if let Some(prompt) = result.get("prompt").and_then(|v| v.as_str()) {
                        println!("{}", prompt);
                    }
                    // Print metadata to stderr so it doesn't interfere with prompt capture
                    if let Some(pf) = result.get("prompt_file").and_then(|v| v.as_str()) {
                        eprintln!("Prompt file: {}", pf);
                    }
                    if let Some(of) = result.get("output_file").and_then(|v| v.as_str()) {
                        eprintln!("Output file: {}", of);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::EvolvePanel {
            output_dir,
            round,
            panel,
        } => {
            let panel_value: serde_json::Value = serde_json::from_str(&panel)
                .map_err(|e| anyhow::anyhow!("Invalid panel JSON: {}", e))?;
            let args = json!({
                "output_dir": output_dir,
                "round": round,
                "panel": panel_value,
            });
            match blue_core::handlers::dialogue::handle_evolve_panel(&args) {
                Ok(result) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::Lint { file } => {
            let args = json!({ "file_path": file });
            match blue_core::handlers::dialogue_lint::handle_dialogue_lint(&args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::Extract { source } => {
            // Try as file path first, then as task ID
            let args = if std::path::Path::new(&source).exists() {
                json!({ "file_path": source })
            } else {
                json!({ "task_id": source })
            };
            match blue_core::handlers::dialogue::handle_extract_dialogue(&args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::Save { title, source, summary, rfc } => {
            let args = if std::path::Path::new(&source).exists() {
                json!({ "title": title, "file_path": source, "summary": summary, "rfc_title": rfc })
            } else {
                json!({ "title": title, "task_id": source, "summary": summary, "rfc_title": rfc })
            };
            match blue_core::handlers::dialogue::handle_save(&mut state, &args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::RoundContext { dialogue_id, round } => {
            let args = json!({ "dialogue_id": dialogue_id, "round": round });
            match blue_core::handlers::dialogue::handle_round_context(&state, &args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::ExpertCreate { dialogue_id, expert_slug, role, tier, creation_reason, first_round } => {
            let args = json!({
                "dialogue_id": dialogue_id,
                "expert_slug": expert_slug,
                "role": role,
                "tier": tier,
                "creation_reason": creation_reason,
                "first_round": first_round,
            });
            match blue_core::handlers::dialogue::handle_expert_create(&state, &args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::RoundRegister { dialogue_id, round, data } => {
            let data_value: serde_json::Value = serde_json::from_str(&data)
                .map_err(|e| anyhow::anyhow!("Invalid round data JSON: {}", e))?;
            let mut args = json!({ "dialogue_id": dialogue_id, "round": round });
            // Merge data fields into args
            if let Some(obj) = data_value.as_object() {
                for (k, v) in obj {
                    args[k] = v.clone();
                }
            }
            match blue_core::handlers::dialogue::handle_round_register(&state, &args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::VerdictRegister { dialogue_id, data } => {
            let data_value: serde_json::Value = serde_json::from_str(&data)
                .map_err(|e| anyhow::anyhow!("Invalid verdict data JSON: {}", e))?;
            let mut args = json!({ "dialogue_id": dialogue_id });
            if let Some(obj) = data_value.as_object() {
                for (k, v) in obj {
                    args[k] = v.clone();
                }
            }
            match blue_core::handlers::dialogue::handle_verdict_register(&state, &args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => { eprintln!("Error: {}", e); std::process::exit(1); }
            }
        }
        DialogueCommands::RoundVerify {
            output_dir,
            round,
            agents,
        } => {
            let agents_value: serde_json::Value = serde_json::from_str(&agents)
                .map_err(|e| anyhow::anyhow!("Invalid agents JSON: {}", e))?;
            let args = json!({
                "output_dir": output_dir,
                "round": round,
                "agents": agents_value,
            });
            match blue_core::handlers::dialogue::handle_round_verify(&args) {
                Ok(result) => println!("{}", serde_json::to_string_pretty(&result)?),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        DialogueCommands::SamplePanel {
            title,
            panel_size,
            retain,
            exclude,
        } => {
            let mut args = json!({ "dialogue_title": title });
            if let Some(size) = panel_size {
                args["panel_size"] = json!(size);
            }
            if !retain.is_empty() {
                args["retain"] = json!(retain);
            }
            if !exclude.is_empty() {
                args["exclude"] = json!(exclude);
            }
            match blue_core::handlers::dialogue::handle_sample_panel(&args) {
                Ok(result) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&result).unwrap_or_default()
                    );
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Handle ADR subcommands
async fn handle_adr_command(command: AdrCommands) -> Result<()> {
    let mut state = get_project_state()?;

    match command {
        AdrCommands::Create { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::adr::handle_create(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AdrCommands::Get { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::adr::handle_get(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AdrCommands::List => match blue_core::handlers::adr::handle_list(&state) {
            Ok(result) => {
                if let Some(adrs) = result.get("adrs").and_then(|v| v.as_array()) {
                    if adrs.is_empty() {
                        println!("No ADRs found.");
                    } else {
                        for a in adrs {
                            let number = a.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                            let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                            let status = a.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                            println!("  {:04} {} [{}]", number, title, status);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        AdrCommands::Status {
            title,
            status: _status,
        } => {
            // Note: ADR status changes require editing the file directly
            println!("To change ADR status, edit the ADR file directly.");
            println!("Looking for ADR '{}'...", title);
            let args = json!({ "title": title });
            if let Ok(result) = blue_core::handlers::adr::handle_get(&state, &args) {
                if let Some(file) = result.get("file_path").and_then(|v| v.as_str()) {
                    println!("File: {}", file);
                }
            }
        }
    }
    Ok(())
}

/// Handle spike subcommands
async fn handle_spike_command(command: SpikeCommands) -> Result<()> {
    let mut state = get_project_state()?;

    match command {
        SpikeCommands::Create { title, budget } => {
            let args = json!({ "title": title, "budget_hours": budget });
            match blue_core::handlers::spike::handle_create(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        SpikeCommands::Get { title } => {
            // Spike get/list not yet implemented - check .blue/docs/spikes/
            println!(
                "Spike details for '{}' - check .blue/docs/spikes/ directory",
                title
            );
            println!("hint: Use `ls .blue/docs/spikes/` to see available spikes");
        }
        SpikeCommands::List => {
            // Spike list not yet implemented - show directory hint
            println!("Listing spikes from .blue/docs/spikes/");
            let spike_dir = std::path::Path::new(".blue/docs/spikes");
            if spike_dir.exists() {
                for entry in std::fs::read_dir(spike_dir)? {
                    let entry = entry?;
                    let name = entry.file_name();
                    println!("  {}", name.to_string_lossy());
                }
            } else {
                println!("No spikes directory found.");
            }
        }
        SpikeCommands::Complete { title, outcome } => {
            let args = json!({ "title": title, "outcome": outcome });
            match blue_core::handlers::spike::handle_complete(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Handle audit subcommands
async fn handle_audit_command(command: AuditCommands) -> Result<()> {
    let state = get_project_state()?;

    match command {
        AuditCommands::Create { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::audit_doc::handle_create(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AuditCommands::Get { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::audit_doc::handle_get(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        AuditCommands::List => match blue_core::handlers::audit_doc::handle_list(&state) {
            Ok(result) => {
                if let Some(audits) = result.get("audits").and_then(|v| v.as_array()) {
                    if audits.is_empty() {
                        println!("No audits found.");
                    } else {
                        for a in audits {
                            let title = a.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                            let status = a.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                            println!("  {} [{}]", title, status);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    }
    Ok(())
}

/// Handle PRD subcommands
async fn handle_prd_command(command: PrdCommands) -> Result<()> {
    let state = get_project_state()?;

    match command {
        PrdCommands::Create { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::prd::handle_create(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        PrdCommands::Get { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::prd::handle_get(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        PrdCommands::List => {
            let args = json!({});
            match blue_core::handlers::prd::handle_list(&state, &args) {
                Ok(result) => {
                    if let Some(prds) = result.get("prds").and_then(|v| v.as_array()) {
                        if prds.is_empty() {
                            println!("No PRDs found.");
                        } else {
                            for p in prds {
                                let title = p.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                                let status =
                                    p.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                                println!("  {} [{}]", title, status);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Handle reminder subcommands
async fn handle_reminder_command(command: ReminderCommands) -> Result<()> {
    let state = get_project_state()?;

    match command {
        ReminderCommands::Create { message, when } => {
            let args = json!({ "message": message, "when": when });
            match blue_core::handlers::reminder::handle_create(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ReminderCommands::List => {
            let args = json!({});
            match blue_core::handlers::reminder::handle_list(&state, &args) {
                Ok(result) => {
                    if let Some(reminders) = result.get("reminders").and_then(|v| v.as_array()) {
                        if reminders.is_empty() {
                            println!("No reminders.");
                        } else {
                            for r in reminders {
                                let id = r.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let msg = r.get("message").and_then(|v| v.as_str()).unwrap_or("?");
                                let due = r.get("due_at").and_then(|v| v.as_str()).unwrap_or("?");
                                println!("  [{}] {} (due: {})", id, msg, due);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ReminderCommands::Snooze { id, until } => {
            let args = json!({ "id": id, "until": until });
            match blue_core::handlers::reminder::handle_snooze(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ReminderCommands::Dismiss { id } => {
            let args = json!({ "id": id });
            match blue_core::handlers::reminder::handle_clear(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Handle RFC subcommands (RFC 0061)
async fn handle_rfc_command(command: RfcCommands) -> Result<()> {
    let mut state = get_project_state()?;

    match command {
        RfcCommands::Create {
            title,
            problem,
            source_spike,
        } => {
            let mut args = json!({ "title": title });
            if let Some(p) = problem {
                args["problem"] = json!(p);
            }
            if let Some(s) = source_spike {
                args["source_spike"] = json!(s);
            }
            match blue_core::handlers::rfc::handle_create(&mut state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                    if let Some(file) = result.get("file").and_then(|v| v.as_str()) {
                        println!("File: {}", file);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RfcCommands::List { status } => {
            let args = match status {
                Some(s) => json!({ "status": s }),
                None => json!({}),
            };
            match blue_core::handlers::rfc::handle_list(&state, &args) {
                Ok(result) => {
                    if let Some(rfcs) = result.get("rfcs").and_then(|v| v.as_array()) {
                        if rfcs.is_empty() {
                            println!("No RFCs found.");
                        } else {
                            for r in rfcs {
                                let number = r.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
                                let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                                let status =
                                    r.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                                println!("  {:04} {} [{}]", number, title, status);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RfcCommands::Get { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::rfc::handle_get(&state, &args) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RfcCommands::Status { title, set } => {
            let args = json!({ "title": title, "status": set });
            match blue_core::handlers::rfc::handle_update_status(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RfcCommands::Plan { title, task } => {
            let args = json!({ "title": title, "tasks": task });
            match blue_core::handlers::rfc::handle_plan(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                    if let Some(file) = result.get("plan_file").and_then(|v| v.as_str()) {
                        println!("Plan file: {}", file);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RfcCommands::Complete { title } => {
            let args = json!({ "title": title });
            match blue_core::handlers::rfc::handle_complete(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Handle local worktree subcommands (RFC 0061)
async fn handle_local_worktree_command(command: WorktreeCommands) -> Result<()> {
    let state = get_project_state()?;

    match command {
        WorktreeCommands::Create { title } => {
            let args = json!({ "rfc_title": title });
            match blue_core::handlers::worktree::handle_create(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                    if let Some(path) = result.get("worktree_path").and_then(|v| v.as_str()) {
                        println!("Worktree: {}", path);
                    }
                    if let Some(branch) = result.get("branch").and_then(|v| v.as_str()) {
                        println!("Branch: {}", branch);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        WorktreeCommands::List => match blue_core::handlers::worktree::handle_list(&state) {
            Ok(result) => {
                if let Some(worktrees) = result.get("worktrees").and_then(|v| v.as_array()) {
                    if worktrees.is_empty() {
                        println!("No worktrees found.");
                    } else {
                        for w in worktrees {
                            let name = w.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let branch = w.get("branch").and_then(|v| v.as_str()).unwrap_or("?");
                            let path = w.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                            println!("  {} ({}) -> {}", name, branch, path);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        WorktreeCommands::Remove { title } => {
            let args = json!({ "name": title });
            match blue_core::handlers::worktree::handle_remove(&state, &args) {
                Ok(result) => {
                    if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
                        println!("{}", msg);
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

async fn handle_jira_command(command: JiraCommands) -> Result<()> {
    // Tracker uses reqwest::blocking — must run outside tokio runtime
    tokio::task::spawn_blocking(move || handle_jira_command_blocking(command)).await??;
    Ok(())
}

fn handle_jira_command_blocking(command: JiraCommands) -> Result<()> {
    match command {
        JiraCommands::Setup => {
            jira_setup_wizard()?;
        }
        JiraCommands::Doctor { domain } => {
            let domain = domain.unwrap_or_else(|| {
                std::env::var("BLUE_JIRA_TEST_DOMAIN").unwrap_or_else(|_| {
                    eprintln!("No domain specified. Use --domain or set BLUE_JIRA_TEST_DOMAIN");
                    std::process::exit(1);
                })
            });

            println!("Jira Doctor: {}\n", domain);

            // 1. Check credentials
            let store = blue_core::tracker::CredentialStore::new(&domain);
            let tier = store.resolve_tier();
            match tier {
                Some(t) => println!("  [ok] Credentials found (source: {})", t),
                None => {
                    println!("  [FAIL] No credentials found");
                    println!("         Run: blue jira auth login --domain {}", domain);
                    std::process::exit(1);
                }
            }

            // 2. Check API access
            let creds = store.get_credentials()?;
            let tracker = blue_core::tracker::JiraCloudTracker::new(
                domain.clone(),
                creds.email.clone(),
                creds.token,
            );

            match tracker.auth_status() {
                Ok(status) => {
                    println!(
                        "  [ok] Authenticated as {}",
                        status.user.unwrap_or_else(|| "unknown".to_string())
                    );
                }
                Err(e) => {
                    println!("  [FAIL] Auth failed: {}", e);
                    std::process::exit(1);
                }
            }

            // 3. Check project access
            match tracker.list_projects() {
                Ok(projects) => {
                    println!("  [ok] {} project(s) accessible", projects.len());
                    for p in &projects {
                        println!("       - {} ({})", p.key, p.name);
                    }
                }
                Err(e) => {
                    println!("  [FAIL] Project list failed: {}", e);
                    std::process::exit(1);
                }
            }

            println!("\nAll checks passed.");
        }

        JiraCommands::Auth { command } => match command {
            JiraAuthCommands::Login {
                domain,
                email,
                token,
                toml,
            } => {
                let email = email.unwrap_or_else(|| {
                    eprint!("Email: ");
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf.trim().to_string()
                });

                let token = token.unwrap_or_else(|| {
                    eprint!("API Token: ");
                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).unwrap();
                    buf.trim().to_string()
                });

                // Verify before storing
                let tracker = blue_core::tracker::JiraCloudTracker::new(
                    domain.clone(),
                    email.clone(),
                    token.clone(),
                );

                match tracker.auth_status() {
                    Ok(status) => {
                        println!(
                            "Verified: {}",
                            status.user.unwrap_or_else(|| "authenticated".to_string())
                        );
                    }
                    Err(e) => {
                        eprintln!("Auth failed: {}. Credentials not stored.", e);
                        std::process::exit(1);
                    }
                }

                let creds = blue_core::tracker::TrackerCredentials { email, token };
                let store = blue_core::tracker::CredentialStore::new(&domain);

                if toml {
                    store.store_toml(&creds)?;
                    println!("Stored in TOML (~/.config/blue/jira-credentials.toml)");
                } else {
                    match store.store_keychain(&creds) {
                        Ok(()) => println!("Stored in OS keychain"),
                        Err(e) => {
                            eprintln!("Keychain failed ({}), falling back to TOML", e);
                            store.store_toml(&creds)?;
                            println!("Stored in TOML file");
                        }
                    }
                }
            }

            JiraAuthCommands::Status { domain } => {
                if let Some(domain) = domain {
                    let store = blue_core::tracker::CredentialStore::new(&domain);
                    match store.resolve_tier() {
                        Some(tier) => {
                            let creds = store.get_credentials()?;
                            let tracker = blue_core::tracker::JiraCloudTracker::new(
                                domain.clone(),
                                creds.email,
                                creds.token,
                            );
                            match tracker.auth_status() {
                                Ok(status) => println!(
                                    "{}: valid (source: {}, user: {})",
                                    domain,
                                    tier,
                                    status.user.unwrap_or_default()
                                ),
                                Err(_) => {
                                    println!("{}: expired/invalid (source: {})", domain, tier)
                                }
                            }
                        }
                        None => println!("{}: no credentials", domain),
                    }
                } else {
                    println!("Specify --domain to check a specific domain");
                }
            }
        },

        JiraCommands::Import {
            project,
            domain,
            dry_run,
        } => {
            let store = blue_core::tracker::CredentialStore::new(&domain);
            let creds = store.get_credentials()?;
            let tracker =
                blue_core::tracker::JiraCloudTracker::new(domain.clone(), creds.email, creds.token);

            println!("Scanning {} @ {}...\n", project, domain);

            let scan = blue_core::tracker::ImportScan::run(&tracker, &project, &domain)?;
            println!("{}", scan.render_report());

            if !dry_run {
                // Phase 2: Write RFC stubs and epic YAML to .blue/docs/rfcs/
                let state = get_project_state()?;
                let rfcs_dir = state.home.docs_path.join("rfcs");
                std::fs::create_dir_all(&rfcs_dir)
                    .map_err(|e| anyhow::anyhow!("Failed to create rfcs dir: {}", e))?;

                let mut written = 0;
                for (i, rfc_stub) in scan.rfcs.iter().enumerate() {
                    let slug = rfc_stub
                        .title
                        .to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join("-")
                        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
                    let filename = format!("imported-{:03}-{}.draft.md", i + 1, slug);
                    let path = rfcs_dir.join(&filename);
                    if !path.exists() {
                        let content = blue_core::tracker::import::render_rfc_stub(rfc_stub);
                        std::fs::write(&path, &content)
                            .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", filename, e))?;
                        println!("  Created: {}", path.display());
                        written += 1;
                    }
                }

                // Write epic YAML files
                let epics_dir = state.home.blue_dir.join("jira").join("epics");
                std::fs::create_dir_all(&epics_dir)
                    .map_err(|e| anyhow::anyhow!("Failed to create epics dir: {}", e))?;

                for epic in &scan.epics {
                    let yaml_content = blue_core::tracker::import::render_epic_yaml(epic);
                    let slug = epic
                        .title
                        .to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join("-")
                        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");
                    let path = epics_dir.join(format!("{}.yaml", slug));
                    if !path.exists() {
                        std::fs::write(&path, &yaml_content)
                            .map_err(|e| anyhow::anyhow!("Failed to write epic: {}", e))?;
                        println!("  Created: {}", path.display());
                    }
                }

                println!(
                    "\nImported {} RFC stubs and {} epics.",
                    written,
                    scan.epics.len()
                );
            }
        }

        JiraCommands::Status { domain, project } => {
            let store = blue_core::tracker::CredentialStore::new(&domain);
            let creds = store.get_credentials()?;
            let tracker =
                blue_core::tracker::JiraCloudTracker::new(domain.clone(), creds.email, creds.token);

            // Scan local RFCs for Jira bindings
            let state = get_project_state()?;
            let rfcs_dir = state.home.docs_path.join("rfcs");

            println!("Jira Status: {} / {}\n", domain, project);

            if !rfcs_dir.exists() {
                println!("No RFCs directory found.");
                return Ok(());
            }

            let mut synced = 0;
            let mut unsynced = 0;
            let mut drift = 0;

            for entry in std::fs::read_dir(&rfcs_dir)
                .map_err(|e| anyhow::anyhow!("Failed to read rfcs dir: {}", e))?
            {
                let entry =
                    entry.map_err(|e| anyhow::anyhow!("Failed to read entry: {}", e))?;
                let path = entry.path();
                if path.extension().map_or(true, |ext| ext != "md") {
                    continue;
                }

                let content = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))?;
                let binding = blue_core::tracker::sync::parse_jira_binding(&content);
                let rfc_title = blue_core::tracker::sync::parse_rfc_title(&content)
                    .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());
                let rfc_status = blue_core::tracker::sync::parse_rfc_status(&content);

                if let Some(ref task_key) = binding.task_key {
                    // Check Jira status
                    match tracker.get_issue(task_key) {
                        Ok(issue) => {
                            let expected_jira_status = rfc_status
                                .as_deref()
                                .and_then(blue_core::tracker::sync::rfc_status_to_jira);

                            let status_match = expected_jira_status
                                .map(|expected| issue.status.name == expected)
                                .unwrap_or(true);

                            if status_match {
                                println!(
                                    "  [synced]  {} → {} ({})",
                                    rfc_title, task_key, issue.status.name
                                );
                                synced += 1;
                            } else {
                                println!(
                                    "  [drift]   {} → {} (jira: {}, expected: {})",
                                    rfc_title,
                                    task_key,
                                    issue.status.name,
                                    expected_jira_status.unwrap_or("?")
                                );
                                drift += 1;
                            }
                        }
                        Err(e) => {
                            println!("  [error]   {} → {} ({})", rfc_title, task_key, e);
                            drift += 1;
                        }
                    }
                } else {
                    println!("  [unsync]  {}", rfc_title);
                    unsynced += 1;
                }
            }

            println!(
                "\nSummary: {} synced, {} drifted, {} unsynced",
                synced, drift, unsynced
            );
        }
    }
    Ok(())
}

// ==================== RFC 0063: Sync Command ====================

fn handle_sync_command(
    domain: &str,
    project: &str,
    dry_run: bool,
    drift_policy_str: &str,
) -> Result<()> {
    let drift_policy = match drift_policy_str {
        "overwrite" => blue_core::tracker::sync::DriftPolicy::Overwrite,
        "block" => blue_core::tracker::sync::DriftPolicy::Block,
        _ => blue_core::tracker::sync::DriftPolicy::Warn,
    };

    let store = blue_core::tracker::CredentialStore::new(domain);
    let creds = store.get_credentials()?;
    let tracker =
        blue_core::tracker::JiraCloudTracker::new(domain.to_string(), creds.email, creds.token);

    let config = blue_core::tracker::sync::SyncConfig {
        domain: domain.to_string(),
        project_key: project.to_string(),
        drift_policy,
    };

    // Auto-detect PM repo mode: epics/ at cwd with either domain.yaml or jira.toml
    let cwd = std::env::current_dir().unwrap_or_default();
    let has_epics = cwd.join("epics").exists();
    let has_domain_yaml = cwd.join("domain.yaml").exists();
    let has_jira_toml = cwd.join("jira.toml").exists();
    let is_pm_repo = has_epics && (has_domain_yaml || has_jira_toml);

    let report = if is_pm_repo {
        // PM repo mode: sync epics/ with YAML front matter (RFC 0068)
        if dry_run {
            println!("PM Sync (dry run): {} / {}\n", domain, project);
        } else {
            println!("Syncing PM repo to Jira: {} / {}\n", domain, project);
        }

        // Try to read config overrides from domain.yaml or jira.toml
        let (actual_domain, actual_project) = if let Ok(pm_domain) =
            blue_core::pm::domain::PmDomain::load(&cwd.join("domain.yaml"))
        {
            (
                pm_domain.domain.unwrap_or_else(|| domain.to_string()),
                pm_domain.project_key.unwrap_or_else(|| project.to_string()),
            )
        } else if let Ok(jira_toml_content) = std::fs::read_to_string(cwd.join("jira.toml")) {
            // Simple line-based parsing of jira.toml for domain and project_key
            let mut d = domain.to_string();
            let mut p = project.to_string();
            for line in jira_toml_content.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("domain").and_then(|s| s.trim_start().strip_prefix('=')).map(|s| s.trim().trim_matches('"')) {
                    d = val.to_string();
                } else if let Some(val) = line.strip_prefix("project_key").and_then(|s| s.trim_start().strip_prefix('=')).map(|s| s.trim().trim_matches('"')) {
                    p = val.to_string();
                }
            }
            (d, p)
        } else {
            (domain.to_string(), project.to_string())
        };

        if actual_domain.as_str() != domain || actual_project.as_str() != project {
            println!(
                "  (using local config: {} / {})\n",
                actual_domain, actual_project
            );
        }

        let store = blue_core::tracker::CredentialStore::new(&actual_domain);
        let creds = store.get_credentials()?;
        let tracker = blue_core::tracker::JiraCloudTracker::new(
            actual_domain.clone(),
            creds.email,
            creds.token,
        );

        // Pre-sync: verify project exists, try to create if not
        ensure_project_exists(&tracker, &actual_project, &actual_domain)?;

        let config = blue_core::tracker::sync::SyncConfig {
            domain: actual_domain,
            project_key: actual_project,
            drift_policy,
        };
        blue_core::pm::sync::run_pm_sync(&tracker, &config, &cwd, dry_run)?
    } else {
        // RFC mode: sync .blue/docs/rfcs/ with markdown table front matter (RFC 0063)
        ensure_project_exists(&tracker, project, domain)?;

        let state = get_project_state()?;
        let rfcs_dir = state.home.docs_path.join("rfcs");

        if dry_run {
            println!("Sync (dry run): {} / {}\n", domain, project);
        } else {
            println!("Syncing RFCs to Jira: {} / {}\n", domain, project);
        }

        blue_core::tracker::sync::run_sync(&tracker, &config, &rfcs_dir, dry_run)?
    };

    for result in &report.results {
        let status = match &result.action {
            blue_core::tracker::sync::SyncAction::Created => "created",
            blue_core::tracker::sync::SyncAction::Transitioned => "transitioned",
            blue_core::tracker::sync::SyncAction::UpToDate => "up-to-date",
            blue_core::tracker::sync::SyncAction::Skipped => "skipped",
            blue_core::tracker::sync::SyncAction::Error => "ERROR",
        };

        let key_info = result
            .jira_key
            .as_deref()
            .map(|k| format!(" → {}", k))
            .unwrap_or_default();

        println!("  [{}] {}{}", status, result.rfc_title, key_info);

        if let Some(ref err) = result.error {
            println!("         {}", err);
        }
    }

    println!(
        "\nSummary: {} created, {} transitioned, {} up-to-date, {} errors",
        report.created, report.transitioned, report.up_to_date, report.errors
    );

    if !report.drift.is_empty() {
        println!("\nDrift detected:");
        for d in &report.drift {
            println!(
                "  {} ({}): {} — local: {}, jira: {}",
                d.rfc_title, d.jira_key, d.field, d.local_value, d.jira_value
            );
        }
    }

    Ok(())
}

/// Pre-sync: verify the Jira project exists. If not, try to create it.
/// On permission failure, print helpful instructions and bail.
fn ensure_project_exists(
    tracker: &blue_core::tracker::JiraCloudTracker,
    project_key: &str,
    domain: &str,
) -> Result<()> {
    use blue_core::tracker::IssueTracker;

    match tracker.project_exists(project_key) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(e) => {
            eprintln!("Warning: Could not check project: {}", e);
            return Ok(()); // proceed anyway, will fail on issue creation
        }
    }

    println!(
        "Project '{}' not found in Jira. Attempting to create...",
        project_key
    );

    let opts = blue_core::tracker::CreateProjectOpts {
        key: project_key.to_string(),
        name: project_key.to_string(),
        project_type: "software".to_string(),
        lead_account_id: None, // will use current user
    };

    match tracker.create_project(opts) {
        Ok(project) => {
            println!(
                "Created Jira project: {} ({})\n",
                project.name, project.key
            );
            Ok(())
        }
        Err(e) => {
            let api_status = match &e {
                blue_core::tracker::TrackerError::Api { status, .. } => Some(*status),
                _ => None,
            };

            if api_status == Some(403) || api_status == Some(401) {
                eprintln!("Could not create project (insufficient permissions).\n");
                eprintln!("Please create the project manually:");
                eprintln!(
                    "  1. Go to https://{}/secure/admin/AddProject!default.jspa",
                    domain
                );
                eprintln!("  2. Project name: {}", project_key);
                eprintln!("  3. Project key: {}", project_key);
                eprintln!("  4. Project type: Scrum or Kanban");
                eprintln!("  5. Then re-run: blue sync --domain {} --project {}", domain, project_key);
                anyhow::bail!("Project '{}' does not exist and could not be created", project_key);
            }

            // Other error — might be network, parse, etc.
            anyhow::bail!("Failed to create project '{}': {}", project_key, e);
        }
    }
}

// ==================== RFC 0067: Org Commands ====================

fn handle_org_command(command: OrgCommands) -> Result<()> {
    match command {
        OrgCommands::List => {
            let config = blue_core::BlueGlobalConfig::load();
            println!("Blue home: {}\n", config.home.path);

            if config.orgs.is_empty() {
                println!("No orgs registered.");
                println!("  blue org add superviber");
                println!("  blue org add myorg --provider forgejo --host git.example.com");
                return Ok(());
            }

            for org in &config.orgs {
                let host_info = org
                    .host
                    .as_deref()
                    .map(|h| format!(" ({})", h))
                    .unwrap_or_default();
                println!("{}  [{}]{}", org.name, org.provider, host_info);

                let repos = config.scan_org(&org.name);
                if repos.is_empty() {
                    println!("  (no repos found)");
                } else {
                    for repo in &repos {
                        println!("  {}", repo);
                    }
                }
                println!();
            }
        }

        OrgCommands::Add {
            name,
            provider,
            host,
        } => {
            let mut config = blue_core::BlueGlobalConfig::load();

            let org = match provider.as_str() {
                "forgejo" => {
                    let host = host.unwrap_or_else(|| {
                        eprint!("Forgejo host: ");
                        let mut buf = String::new();
                        std::io::stdin().read_line(&mut buf).unwrap();
                        buf.trim().to_string()
                    });
                    blue_core::Org::forgejo(&name, &host)
                }
                _ => blue_core::Org::github(&name),
            };

            config.add_org(org);
            config.save()?;
            println!("Registered org: {}", name);

            // Create the org directory if it doesn't exist
            let org_dir = config.home_path().join(&name);
            if !org_dir.exists() {
                std::fs::create_dir_all(&org_dir)?;
                println!("Created: {}", org_dir.display());
            }
        }

        OrgCommands::Remove { name } => {
            let mut config = blue_core::BlueGlobalConfig::load();
            if config.remove_org(&name) {
                config.save()?;
                println!("Removed org: {}", name);
            } else {
                eprintln!("Org not found: {}", name);
            }
        }

        OrgCommands::Scan { name } => {
            let config = blue_core::BlueGlobalConfig::load();

            let orgs_to_scan: Vec<&blue_core::Org> = if name == "all" {
                config.orgs.iter().collect()
            } else {
                match config.find_org(&name) {
                    Some(org) => vec![org],
                    None => {
                        eprintln!("Org not registered: {}. Run: blue org add {}", name, name);
                        return Ok(());
                    }
                }
            };

            for org in orgs_to_scan {
                let repos = config.scan_org(&org.name);
                println!("{}/  ({} repos)", org.name, repos.len());
                for repo in &repos {
                    println!("  {}", repo);
                }
            }
        }

        OrgCommands::Status => {
            let config = blue_core::BlueGlobalConfig::load();
            let home = config.home_path();

            println!("Blue home: {}", home.display());
            println!("Config:    {}\n", blue_core::org_config_path().display());

            if config.orgs.is_empty() {
                println!("No orgs registered. Run: blue org add <name>");
                return Ok(());
            }

            let mut total_repos = 0;
            for org in &config.orgs {
                let repos = config.scan_org(&org.name);
                let org_dir = home.join(&org.name);
                let exists = org_dir.exists();
                let status = if !exists {
                    " (directory missing)"
                } else if repos.is_empty() {
                    " (empty)"
                } else {
                    ""
                };
                println!(
                    "{}  [{}]{}  — {} repos",
                    org.name,
                    org.provider,
                    status,
                    repos.len()
                );
                total_repos += repos.len();
            }
            println!(
                "\n{} orgs, {} repos total",
                config.orgs.len(),
                total_repos
            );
        }

        OrgCommands::Migrate { from, execute } => {
            let config = blue_core::BlueGlobalConfig::load();
            let scan_dir = from
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| config.home_path());
            let home = config.home_path();

            println!("Scanning {} for git repos...\n", scan_dir.display());
            let moves = blue_core::scan_for_migration(&scan_dir, &home);

            if moves.is_empty() {
                println!("No migrations needed. All repos are already in org directories.");
                return Ok(());
            }

            // Group by org for display
            let mut current_org = String::new();
            for mv in &moves {
                if mv.org != current_org {
                    if !current_org.is_empty() {
                        println!();
                    }
                    println!("{}/ ", mv.org);
                    current_org = mv.org.clone();
                }
                let from_short = mv
                    .from
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                if from_short == mv.repo_name {
                    println!("  {} → {}", mv.from.display(), mv.to.display());
                } else {
                    println!(
                        "  {} → {} (renamed from {})",
                        mv.from.display(),
                        mv.to.display(),
                        from_short
                    );
                }
            }

            // Detect collisions
            let mut targets: std::collections::HashMap<&std::path::Path, Vec<&str>> =
                std::collections::HashMap::new();
            for mv in &moves {
                targets
                    .entry(mv.to.as_path())
                    .or_default()
                    .push(&mv.repo_dir_name);
            }
            let collisions: Vec<_> = targets
                .iter()
                .filter(|(_, sources)| sources.len() > 1)
                .collect();
            if !collisions.is_empty() {
                println!("\nCollisions detected:");
                for (target, sources) in &collisions {
                    println!(
                        "  {} ← {}",
                        target.display(),
                        sources.join(", ")
                    );
                }
                println!("Resolve these before running with --execute.");
            }

            println!("\n{} repos to move", moves.len());

            if !execute {
                println!("\nDry run. Add --execute to move files.");
                return Ok(());
            }

            println!();
            let mut success = 0;
            let mut failed = 0;
            for mv in &moves {
                match blue_core::execute_move(mv) {
                    Ok(()) => {
                        println!("  Moved: {}/{}", mv.org, mv.repo_name);
                        success += 1;

                        // Auto-register org if needed
                        let mut config = blue_core::BlueGlobalConfig::load();
                        if config.find_org(&mv.org).is_none() {
                            if let Some((_, _, provider)) =
                                blue_core::detect_org_from_repo(&mv.to)
                            {
                                config.add_org(match provider {
                                    blue_core::Provider::Github => {
                                        blue_core::Org::github(&mv.org)
                                    }
                                    blue_core::Provider::Forgejo => {
                                        blue_core::Org::forgejo(&mv.org, "")
                                    }
                                });
                                let _ = config.save();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  Failed: {}/{} — {}", mv.org, mv.repo_name, e);
                        failed += 1;
                    }
                }
            }

            println!("\nDone. {} moved, {} failed.", success, failed);
        }

        OrgCommands::Home { path } => {
            let mut config = blue_core::BlueGlobalConfig::load();
            match path {
                Some(new_path) => {
                    let expanded = if new_path.starts_with('~') {
                        let home = dirs::home_dir().unwrap_or_default();
                        home.join(new_path.trim_start_matches("~/"))
                            .display()
                            .to_string()
                    } else {
                        std::path::Path::new(&new_path)
                            .canonicalize()
                            .unwrap_or_else(|_| std::path::PathBuf::from(&new_path))
                            .display()
                            .to_string()
                    };
                    config.home.path = expanded.clone();
                    config.save()?;
                    println!("Blue home set to: {}", expanded);
                }
                None => {
                    println!("{}", config.home.path);
                }
            }
        }
    }
    Ok(())
}

fn handle_clone_command(url: &str, org_name: Option<&str>, realm: Option<&str>) -> Result<()> {
    let config = blue_core::BlueGlobalConfig::load();

    let target = if let Some(org_name) = org_name {
        // Clone by org + name
        blue_core::clone_repo_by_name(&config, org_name, url)?
    } else {
        // Clone by URL
        blue_core::clone_repo(&config, url)?
    };

    println!("Cloned to: {}", target.display());

    // Auto-detect org and register if needed
    if let Some((org_name, _repo_name, provider)) = blue_core::detect_org_from_repo(&target) {
        let mut config = blue_core::BlueGlobalConfig::load();
        if config.find_org(&org_name).is_none() {
            let org = match provider {
                blue_core::Provider::Github => blue_core::Org::github(&org_name),
                blue_core::Provider::Forgejo => blue_core::Org::forgejo(&org_name, ""),
            };
            config.add_org(org);
            config.save()?;
            println!("Registered org: {}", org_name);
        }
    }

    // Initialize .blue/ in the cloned repo
    let home = blue_core::detect_blue(&target)
        .map_err(|e| anyhow::anyhow!("Failed to init .blue/: {}", e))?;
    println!("Initialized Blue in {}", home.root.display());

    // Optionally join a realm
    if let Some(realm_name) = realm {
        let realms_path = dirs::home_dir()
            .unwrap_or_default()
            .join(".blue")
            .join("realms");
        let service = blue_core::realm::RealmService::new(realms_path);
        let repo_name = target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        service.join_realm(realm_name, repo_name, &target)?;
        println!("Joined realm: {}", realm_name);
    }

    Ok(())
}

fn jira_setup_wizard() -> Result<()> {
    println!("=== Blue × Jira Cloud Setup ===\n");

    // Step 1: Domain
    println!("Step 1: Your Jira domain");
    println!("  This is the \"yourorg\" part of yourorg.atlassian.net\n");
    eprint!("Jira domain (e.g. myorg.atlassian.net): ");
    let mut domain = String::new();
    std::io::stdin().read_line(&mut domain).unwrap();
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        eprintln!("Domain cannot be empty.");
        std::process::exit(1);
    }

    // Step 2: API token instructions
    println!("\nStep 2: Create an API token");
    println!("  1. Go to: https://id.atlassian.com/manage-profile/security/api-tokens");
    println!("  2. Click \"Create API token\"");
    println!("  3. Label it \"blue\" (or anything you like)");
    println!("  4. Copy the token — you won't see it again\n");

    eprint!("Press Enter when you have your token ready...");
    let mut pause = String::new();
    std::io::stdin().read_line(&mut pause).unwrap();

    // Step 3: Credentials
    println!("Step 3: Enter your credentials\n");
    eprint!("Email (the one you log into Jira with): ");
    let mut email = String::new();
    std::io::stdin().read_line(&mut email).unwrap();
    let email = email.trim().to_string();

    eprint!("API Token: ");
    let mut token = String::new();
    std::io::stdin().read_line(&mut token).unwrap();
    let token = token.trim().to_string();

    if email.is_empty() || token.is_empty() {
        eprintln!("Email and token are both required.");
        std::process::exit(1);
    }

    // Step 4: Verify
    println!("\nVerifying credentials against {}...", domain);
    let tracker =
        blue_core::tracker::JiraCloudTracker::new(domain.clone(), email.clone(), token.clone());

    match tracker.auth_status() {
        Ok(status) => {
            let user = status.user.unwrap_or_else(|| "authenticated".to_string());
            println!("  Authenticated as: {}", user);
        }
        Err(e) => {
            eprintln!("  Auth failed: {}", e);
            eprintln!("\nDouble-check your email and token, then try again.");
            std::process::exit(1);
        }
    }

    // Step 5: Store
    println!("\nStoring credentials...");
    let creds = blue_core::tracker::TrackerCredentials { email, token };
    let store = blue_core::tracker::CredentialStore::new(&domain);

    match store.store_keychain(&creds) {
        Ok(()) => println!("  Saved to OS keychain"),
        Err(e) => {
            eprintln!("  Keychain unavailable ({}), using TOML fallback", e);
            store.store_toml(&creds)?;
            println!("  Saved to ~/.config/blue/jira-credentials.toml");
        }
    }

    // Step 6: Doctor check
    println!("\nRunning doctor...\n");
    println!("  Credential source: {}", store.resolve_tier().unwrap_or("none"));

    match tracker.list_projects() {
        Ok(projects) => {
            println!("  Projects accessible: {}", projects.len());
            for p in projects.iter().take(5) {
                println!("    - {} ({})", p.name, p.key);
            }
            if projects.len() > 5 {
                println!("    ... and {} more", projects.len() - 5);
            }
        }
        Err(e) => {
            eprintln!("  Could not list projects: {}", e);
        }
    }

    println!("\nSetup complete. You can now use:");
    println!("  blue jira doctor              — check connection health");
    println!("  blue jira import --dry-run    — preview Jira → RFC import");
    println!("  blue jira auth status         — check stored credentials");

    Ok(())
}
