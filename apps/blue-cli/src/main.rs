//! Blue CLI - Welcome home
//!
//! Command-line interface for Blue.

use clap::{Parser, Subcommand};
use anyhow::Result;
use blue_core::daemon::{DaemonClient, DaemonDb, DaemonPaths, DaemonState, run_daemon};
use blue_core::realm::RealmService;

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
        let path = args.iter()
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
                let dir_name = cwd.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                let parent_is_worktrees = cwd.parent()
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
        eprintln!("guard: blocked write to {} (not in RFC worktree scope)", path.display());
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
            eprintln!("guard: blocked write to {} (no active worktree)", path.display());
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
        let code_extensions = ["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h"];
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
    Init,

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

    /// Launch Goose AI agent with Blue extension
    Agent {
        /// Model to use (default: claude-sonnet-4-20250514)
        #[arg(long, short)]
        model: Option<String>,

        /// Additional Goose arguments
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
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
    },
    /// Create a plan for an RFC
    Plan {
        /// RFC title
        title: String,
    },
    /// Get RFC details
    Get {
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
        None | Some(Commands::Status) => {
            println!("{}", blue_core::voice::welcome());
        }
        Some(Commands::Init) => {
            println!("{}", blue_core::voice::welcome());
            // TODO: Initialize .blue directory
        }
        Some(Commands::Next) => {
            println!("Looking at what's ready. One moment.");
            // TODO: Implement next
        }
        Some(Commands::Mcp { .. }) => {
            blue_mcp::run().await?;
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
        Some(Commands::Rfc { command }) => match command {
            RfcCommands::Create { title } => {
                println!("{}", blue_core::voice::success(
                    &format!("Created RFC '{}'", title),
                    Some("Want me to help fill in the details?"),
                ));
            }
            RfcCommands::Plan { title } => {
                println!("{}", blue_core::voice::ask(
                    &format!("Ready to plan '{}'", title),
                    "What are the tasks",
                ));
            }
            RfcCommands::Get { title } => {
                println!("Looking for '{}'.", title);
            }
        },
        Some(Commands::Worktree { command }) => match command {
            WorktreeCommands::Create { title } => {
                println!("Creating worktree for '{}'.", title);
            }
            WorktreeCommands::List => {
                println!("Listing worktrees.");
            }
            WorktreeCommands::Remove { title } => {
                println!("Removing worktree for '{}'.", title);
            }
        },
        Some(Commands::Pr { command }) => match command {
            PrCommands::Create { title } => {
                println!("Creating PR: {}", title);
            }
        },
        Some(Commands::Lint) => {
            println!("Checking standards.");
        }
        Some(Commands::Migrate { from }) => {
            println!("Coming home from {}.", from);
        }
        Some(Commands::Agent { model, args }) => {
            handle_agent_command(model, args).await?;
        }
        Some(Commands::Index { command }) => {
            handle_index_command(command).await?;
        }
        Some(Commands::Search { query, symbols, limit }) => {
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
        Some(Commands::Install { hooks_only, skills_only, mcp_only, force }) => {
            handle_install_command(hooks_only, skills_only, mcp_only, force).await?;
        }
        Some(Commands::Uninstall) => {
            handle_uninstall_command().await?;
        }
        Some(Commands::Doctor) => {
            handle_doctor_command().await?;
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
                        let realm_sessions: Vec<_> = sessions
                            .iter()
                            .filter(|s| s.realm == *realm_name)
                            .collect();
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

            println!("Creating worktrees for RFC '{}' in realm '{}'...", rfc, realm_name);

            for repo in &details.repos {
                if !repo_names.contains(&repo.name) {
                    continue;
                }

                let repo_path = match &repo.path {
                    Some(p) => std::path::PathBuf::from(p),
                    None => {
                        println!("  {} - skipped (no local path)", repo.name);
                        continue;
                    }
                };

                match service.create_worktree(realm_name, &repo.name, &rfc, &repo_path) {
                    Ok(info) => {
                        if info.already_existed {
                            println!("  {} - already exists at {}", info.repo, info.path.display());
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

            println!("Removing worktrees for RFC '{}' in realm '{}'...", rfc, realm_name);

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
                println!("No worktrees found for RFC '{}' in realm '{}'.", rfc, realm_name);
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
                println!("\nRun 'blue realm pr prepare --rfc {}' to commit changes.", rfc);
            }
        }

        RealmPrCommands::Prepare { rfc, message } => {
            let realm_name = get_realm_name(&rfc)?;
            let msg = message.as_deref();

            println!("Preparing PR for RFC '{}' in realm '{}'...\n", rfc, realm_name);

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

async fn handle_realm_admin_command(command: RealmAdminCommands, _client: &DaemonClient) -> Result<()> {
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
            println!("\nNext: Run 'blue realm admin join {}' in your repos.", name);
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

        RealmAdminCommands::Contract { realm, domain, name, owner } => {
            service.create_contract(&realm, &domain, &name, &owner)?;

            println!("Created contract '{}' in domain '{}'", name, domain);
            println!("  Owner: {}", owner);
            println!("  Version: 1.0.0");
            println!("\nNext: Create bindings to export/import this contract.");
        }

        RealmAdminCommands::Binding { realm, domain, repo, role } => {
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
                    println!(
                        "  {} ({}/{}) - {}",
                        s.id, s.realm, s.repo, rfc
                    );
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

async fn handle_agent_command(model: Option<String>, extra_args: Vec<String>) -> Result<()> {
    use std::process::Command;

    // Find Goose binary: bundled first, then system
    let goose_path = find_goose_binary()?;

    // Check if Ollama is running and get available models
    let ollama_model = if model.is_none() {
        detect_ollama_model().await
    } else {
        None
    };

    // Get the path to the blue binary
    let blue_binary = std::env::current_exe()?;

    // Build the extension command
    let extension_cmd = format!("{} mcp", blue_binary.display());

    println!("Starting Goose with Blue extension...");
    println!("  Goose: {}", goose_path.display());
    println!("  Extension: {}", extension_cmd);

    // Configure Goose for the model
    let (provider, model_name) = if let Some(m) = &model {
        // User specified a model - could be "provider/model" format
        if m.contains('/') {
            let parts: Vec<&str> = m.splitn(2, '/').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            // Assume ollama if no provider specified
            ("ollama".to_string(), m.clone())
        }
    } else if let Some(m) = ollama_model {
        ("ollama".to_string(), m)
    } else {
        // Check if goose is already configured
        let config_path = dirs::config_dir()
            .map(|d| d.join("goose").join("config.yaml"));

        if let Some(path) = &config_path {
            if path.exists() {
                let content = std::fs::read_to_string(path).unwrap_or_default();
                if content.contains("GOOSE_PROVIDER") {
                    println!("  Using existing Goose config");
                    ("".to_string(), "".to_string())
                } else {
                    anyhow::bail!(
                        "No model available. Either:\n  \
                         1. Start Ollama with a model: ollama run qwen2.5:7b\n  \
                         2. Specify a model: blue agent --model ollama/qwen2.5:7b\n  \
                         3. Configure Goose: goose configure"
                    );
                }
            } else {
                anyhow::bail!(
                    "No model available. Either:\n  \
                     1. Start Ollama with a model: ollama run qwen2.5:7b\n  \
                     2. Specify a model: blue agent --model ollama/qwen2.5:7b\n  \
                     3. Configure Goose: goose configure"
                );
            }
        } else {
            anyhow::bail!("Could not determine config directory");
        }
    };

    // Build goose command
    let mut cmd = Command::new(&goose_path);
    cmd.arg("session");
    cmd.arg("--with-extension").arg(&extension_cmd);

    // Configure via environment variables (more reliable than config file)
    if !provider.is_empty() {
        cmd.env("GOOSE_PROVIDER", &provider);
        cmd.env("GOOSE_MODEL", &model_name);
        println!("  Provider: {}", provider);
        println!("  Model: {}", model_name);

        if provider == "ollama" {
            cmd.env("OLLAMA_HOST", "http://localhost:11434");
        }
    }

    // Add any extra arguments
    for arg in extra_args {
        cmd.arg(arg);
    }

    // Execute goose, replacing current process
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        // exec() only returns if there was an error
        anyhow::bail!("Failed to exec goose: {}", err);
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, spawn and wait
        let status = cmd.status()?;
        if !status.success() {
            anyhow::bail!("Goose exited with status: {}", status);
        }
        Ok(())
    }
}


fn find_goose_binary() -> Result<std::path::PathBuf> {
    use std::path::PathBuf;

    let binary_name = if cfg!(windows) { "goose.exe" } else { "goose" };

    // 1. Check Blue's data directory (~/.local/share/blue/bin/goose)
    if let Some(data_dir) = dirs::data_dir() {
        let blue_bin = data_dir.join("blue").join("bin").join(binary_name);
        if blue_bin.exists() && is_block_goose(&blue_bin) {
            return Ok(blue_bin);
        }
    }

    // 2. Check for bundled binary next to blue executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join(binary_name);
            if bundled.exists() && is_block_goose(&bundled) {
                return Ok(bundled);
            }
        }
    }

    // 3. Check compile-time bundled path (dev builds)
    if let Some(path) = option_env!("GOOSE_BINARY_PATH") {
        let bundled = PathBuf::from(path);
        if bundled.exists() && is_block_goose(&bundled) {
            return Ok(bundled);
        }
    }

    // 4. Not found - download it
    println!("Goose not found. Downloading...");
    download_goose_runtime()
}

fn is_block_goose(path: &std::path::Path) -> bool {
    // Check if it's Block's Goose (AI agent), not pressly/goose (DB migration)
    if let Ok(output) = std::process::Command::new(path).arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Block's goose outputs version without "DRIVER" references
        // and has "session" subcommand
        !version.contains("DRIVER") && !stderr.contains("DRIVER")
    } else {
        false
    }
}

fn download_goose_runtime() -> Result<std::path::PathBuf> {
    const GOOSE_VERSION: &str = "1.21.1";

    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
    let bin_dir = data_dir.join("blue").join("bin");
    std::fs::create_dir_all(&bin_dir)?;

    let binary_name = if cfg!(windows) { "goose.exe" } else { "goose" };
    let dest = bin_dir.join(binary_name);

    // Determine download URL based on platform
    let (url, is_zip) = get_goose_download_url(GOOSE_VERSION)?;

    println!("  Downloading from: {}", url);

    // Download to temp file
    let temp_dir = tempfile::tempdir()?;
    let archive_path = temp_dir.path().join("goose-archive");

    let status = std::process::Command::new("curl")
        .args(["-L", "-o"])
        .arg(&archive_path)
        .arg(&url)
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to download Goose");
    }

    // Extract
    if is_zip {
        let status = std::process::Command::new("unzip")
            .args(["-o"])
            .arg(&archive_path)
            .arg("-d")
            .arg(temp_dir.path())
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to extract Goose zip");
        }
    } else {
        let status = std::process::Command::new("tar")
            .args(["-xjf"])
            .arg(&archive_path)
            .arg("-C")
            .arg(temp_dir.path())
            .status()?;
        if !status.success() {
            anyhow::bail!("Failed to extract Goose archive");
        }
    }

    // Find the goose binary in extracted files
    let extracted = find_file_recursive(temp_dir.path(), binary_name)?;

    // Copy to destination
    std::fs::copy(&extracted, &dest)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }

    println!("  Installed to: {}", dest.display());
    Ok(dest)
}

fn get_goose_download_url(version: &str) -> Result<(String, bool)> {
    let base = format!(
        "https://github.com/block/goose/releases/download/v{}",
        version
    );

    let (arch, os) = (std::env::consts::ARCH, std::env::consts::OS);

    let (file, is_zip) = match (arch, os) {
        ("aarch64", "macos") => ("goose-aarch64-apple-darwin.tar.bz2", false),
        ("x86_64", "macos") => ("goose-x86_64-apple-darwin.tar.bz2", false),
        ("x86_64", "linux") => ("goose-x86_64-unknown-linux-gnu.tar.bz2", false),
        ("aarch64", "linux") => ("goose-aarch64-unknown-linux-gnu.tar.bz2", false),
        ("x86_64", "windows") => ("goose-x86_64-pc-windows-gnu.zip", true),
        _ => anyhow::bail!("Unsupported platform: {} {}", arch, os),
    };

    Ok((format!("{}/{}", base, file), is_zip))
}

fn find_file_recursive(dir: &std::path::Path, name: &str) -> Result<std::path::PathBuf> {
    // Check direct path
    let direct = dir.join(name);
    if direct.exists() {
        return Ok(direct);
    }

    // Search subdirectories
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(found) = find_file_recursive(&path, name) {
                return Ok(found);
            }
        } else if path.file_name().map(|n| n == name).unwrap_or(false) {
            return Ok(path);
        }
    }

    anyhow::bail!("Binary {} not found in {:?}", name, dir)
}

async fn detect_ollama_model() -> Option<String> {
    // Check if Ollama is running
    let client = reqwest::Client::new();
    let resp = client
        .get("http://localhost:11434/api/tags")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct OllamaModels {
        models: Vec<OllamaModel>,
    }

    #[derive(serde::Deserialize)]
    struct OllamaModel {
        name: String,
        size: u64,
    }

    let models: OllamaModels = resp.json().await.ok()?;

    if models.models.is_empty() {
        return None;
    }

    // Prefer larger models (likely better for agentic work)
    // Sort by size descending and pick first
    let mut sorted = models.models;
    sorted.sort_by(|a, b| b.size.cmp(&a.size));

    let best = &sorted[0];
    println!("  Detected Ollama with {} model(s)", sorted.len());

    Some(best.name.clone())
}

// ==================== Semantic Index Commands (RFC 0010) ====================

async fn handle_index_command(command: IndexCommands) -> Result<()> {
    // Run the blocking indexer operations in a separate thread
    // to avoid runtime conflicts with reqwest::blocking::Client
    tokio::task::spawn_blocking(move || {
        handle_index_command_blocking(command)
    }).await??;
    Ok(())
}

fn handle_index_command_blocking(command: IndexCommands) -> Result<()> {
    use blue_core::store::DocumentStore;
    use blue_core::{Indexer, IndexerConfig, is_indexable_file, LocalLlmConfig};
    use blue_ollama::OllamaLlm;
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
        IndexCommands::All { path, model } => {
            let target_path = path.as_deref().unwrap_or(".");
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

            // Collect all indexable files
            let files = collect_indexable_files(Path::new(target_path))?;
            println!("Found {} indexable files in '{}'", files.len(), target_path);

            if files.is_empty() {
                println!("No files to index.");
                return Ok(());
            }

            // Try to connect to Ollama
            let llm_config = LocalLlmConfig {
                model: model_name.to_string(),
                use_external: true, // Use existing Ollama instance
                ..Default::default()
            };

            let llm = OllamaLlm::new(&llm_config);
            if let Err(e) = llm.start() {
                println!("Ollama not available: {}", e);
                println!("\nTo index files:");
                println!("  1. Start Ollama: ollama serve");
                println!("  2. Pull the model: ollama pull {}", model_name);
                println!("  3. Run this command again");
                return Ok(());
            }

            println!("Indexing with model '{}'...\n", model_name);

            let indexer_config = IndexerConfig {
                model: model_name.to_string(),
                ..Default::default()
            };
            let indexer = Indexer::new(llm, indexer_config);

            let mut indexed = 0;
            let mut errors = 0;

            for file_path in &files {
                let path = Path::new(file_path);
                print!("  {} ... ", file_path);

                match indexer.index_and_store(path, &store) {
                    Ok(result) => {
                        let partial = if result.is_partial { " (partial)" } else { "" };
                        println!("{} symbols{}", result.symbols.len(), partial);
                        indexed += 1;
                    }
                    Err(e) => {
                        println!("error: {}", e);
                        errors += 1;
                    }
                }
            }

            println!("\nIndexed {} files ({} errors)", indexed, errors);
        }

        IndexCommands::Diff { model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

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

            // Try to connect to Ollama
            let llm_config = LocalLlmConfig {
                model: model_name.to_string(),
                use_external: true,
                ..Default::default()
            };

            let llm = OllamaLlm::new(&llm_config);
            if llm.start().is_err() {
                // Silently skip if Ollama not available (pre-commit hook shouldn't block)
                return Ok(());
            }

            println!("Indexing {} staged file(s)...", staged_files.len());

            let indexer_config = IndexerConfig {
                model: model_name.to_string(),
                ..Default::default()
            };
            let indexer = Indexer::new(llm, indexer_config);

            for file_path in &staged_files {
                let path = Path::new(file_path);
                if path.exists() {
                    match indexer.index_and_store(path, &store) {
                        Ok(result) => {
                            println!("  {} - {} symbols", file_path, result.symbols.len());
                        }
                        Err(e) => {
                            println!("  {} - error: {}", file_path, e);
                        }
                    }
                }
            }
        }

        IndexCommands::File { path, model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");
            let file_path = Path::new(&path);

            if !file_path.exists() {
                println!("File not found: {}", path);
                return Ok(());
            }

            // Try to connect to Ollama
            let llm_config = LocalLlmConfig {
                model: model_name.to_string(),
                use_external: true,
                ..Default::default()
            };

            let llm = OllamaLlm::new(&llm_config);
            if let Err(e) = llm.start() {
                println!("Ollama not available: {}", e);
                println!("\nStart Ollama first: ollama serve");
                return Ok(());
            }

            println!("Indexing '{}' with '{}'...", path, model_name);

            let indexer_config = IndexerConfig {
                model: model_name.to_string(),
                ..Default::default()
            };
            let indexer = Indexer::new(llm, indexer_config);

            match indexer.index_and_store(file_path, &store) {
                Ok(result) => {
                    println!("\nSummary: {}", result.summary.unwrap_or_default());
                    if let Some(rel) = &result.relationships {
                        println!("\nRelationships:\n{}", rel);
                    }
                    println!("\nSymbols ({}):", result.symbols.len());
                    for sym in &result.symbols {
                        let lines = match (sym.start_line, sym.end_line) {
                            (Some(s), Some(e)) => format!(" (lines {}-{})", s, e),
                            (Some(s), None) => format!(" (line {})", s),
                            _ => String::new(),
                        };
                        println!("  {} ({}){}", sym.name, sym.kind, lines);
                    }
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }

        IndexCommands::Refresh { model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");
            let realm = "default";

            let (file_count, symbol_count) = store.get_index_stats(realm)?;
            println!("Current index: {} files, {} symbols", file_count, symbol_count);

            if file_count == 0 {
                println!("Index is empty. Run 'blue index all' first.");
                return Ok(());
            }

            // Get all indexed files and check which are stale
            let indexed_files = store.list_file_index(realm, None)?;
            let mut stale_files = Vec::new();

            for entry in &indexed_files {
                let path = Path::new(&entry.file_path);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let current_hash = hash_file_content(&content);
                        if current_hash != entry.file_hash {
                            stale_files.push(entry.file_path.clone());
                        }
                    }
                }
            }

            if stale_files.is_empty() {
                println!("All indexed files are up to date.");
                return Ok(());
            }

            println!("Found {} stale file(s)", stale_files.len());

            // Try to connect to Ollama
            let llm_config = LocalLlmConfig {
                model: model_name.to_string(),
                use_external: true,
                ..Default::default()
            };

            let llm = OllamaLlm::new(&llm_config);
            if let Err(e) = llm.start() {
                println!("Ollama not available: {}", e);
                println!("\nStale files:");
                for f in &stale_files {
                    println!("  {}", f);
                }
                return Ok(());
            }

            println!("Re-indexing stale files with '{}'...\n", model_name);

            let indexer_config = IndexerConfig {
                model: model_name.to_string(),
                ..Default::default()
            };
            let indexer = Indexer::new(llm, indexer_config);

            for file_path in &stale_files {
                let path = Path::new(file_path);
                print!("  {} ... ", file_path);

                match indexer.index_and_store(path, &store) {
                    Ok(result) => {
                        println!("{} symbols", result.symbols.len());
                    }
                    Err(e) => {
                        println!("error: {}", e);
                    }
                }
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
            println!("  {} ({}) - {}{}", symbol.name, symbol.kind, file.file_path, lines);
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

fn print_context_show(manifest: &blue_core::ContextManifest, resolution: &blue_core::ManifestResolution) {
    println!("Context Manifest (v{})", manifest.version);
    println!();

    // Identity tier
    println!("Identity Tier (always injected)");
    println!("  Budget: {} tokens", manifest.identity.max_tokens);
    println!("  Actual: {} tokens", resolution.identity.token_count);
    for source in &resolution.identity.sources {
        let label = source.label.as_deref().unwrap_or("");
        let status = if source.file_count > 0 { "✓" } else { "○" };
        println!("  {} {} ({} files, {} tokens)", status, source.uri, source.file_count, source.tokens);
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
        println!("  {} {} ({} files, {} tokens)", status, source.uri, source.file_count, source.tokens);
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

fn print_context_verbose(manifest: &blue_core::ContextManifest, resolution: &blue_core::ManifestResolution) {
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
                            println!("  {} | {} | {} | {} tokens",
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
            eprintln!("guard: blocked write to {} (not in RFC worktree scope)", path.display());
            std::process::exit(1);
        }
        None => {
            // Not in a worktree - check if there's an active RFC that might apply
            // For now, block writes to source code outside worktrees
            if is_source_code_path(path) {
                eprintln!("guard: blocked write to {} (no active worktree)", path.display());
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
        ".blue/docs/",      // Blue documentation
        ".claude/",         // Claude configuration
        "/tmp/",            // Temp files
        "*.md",             // Markdown at root (but not in crates/)
        ".gitignore",       // Git config
        ".blue/audit/",     // Audit logs
    ];

    for pattern in &allowlist {
        if pattern.starts_with("*.") {
            // Extension pattern - check only root level
            let ext = &pattern[1..];
            if path_str.ends_with(ext) && !path_str.contains("crates/") && !path_str.contains("src/") {
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
    let source_patterns = [
        "src/",
        "crates/",
        "apps/",
        "lib/",
        "packages/",
        "tests/",
    ];

    for pattern in &source_patterns {
        if path_str.contains(pattern) {
            return true;
        }
    }

    // Check file extensions
    if let Some(ext) = path.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) {
        let code_extensions = ["rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "h"];
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
            let dir_name = cwd.file_name()
                .and_then(|n: &std::ffi::OsStr| n.to_str())
                .unwrap_or("");

            let parent_is_worktrees = cwd.parent()
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

    let entry = format!("{} | {} | {} | {} | {}\n", timestamp, user, tool_str, path, reason);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
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

FILE_PATH=$(jq -r '.tool_input.file_path // empty')

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

blue guard --path="$FILE_PATH"
"#;

async fn handle_install_command(hooks_only: bool, skills_only: bool, mcp_only: bool, force: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let cwd = std::env::current_dir()?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    println!("Installing Blue for Claude Code...\n");

    let install_all = !hooks_only && !skills_only && !mcp_only;

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

    // Install MCP server
    if install_all || mcp_only {
        println!("\nMCP Server:");
        install_mcp_server(&cwd, &home)?;
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
            println!("  ✓ ~/.claude/skills/{} -> {}", skill_name.to_string_lossy(), path.display());
        }
    }

    Ok(())
}

fn install_mcp_server(project_dir: &std::path::Path, home: &std::path::Path) -> Result<()> {
    use serde_json::json;

    let config_path = home.join(".claude.json");

    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    // Ensure mcpServers object exists
    if config.get("mcpServers").is_none() {
        config["mcpServers"] = json!({});
    }

    // Add/update blue MCP server
    let binary_path = project_dir.join("target").join("release").join("blue");
    config["mcpServers"]["blue"] = json!({
        "command": binary_path.to_string_lossy(),
        "args": ["mcp"]
    });

    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    println!("  ✓ ~/.claude.json (blue server configured)");

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

    // Remove MCP server
    println!("\nMCP Server:");
    uninstall_mcp_server(&home)?;

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
                println!("  ✓ Removed ~/.claude/skills/{}", skill_name.to_string_lossy());
            }
        }
    }

    Ok(())
}

fn uninstall_mcp_server(home: &std::path::Path) -> Result<()> {
    let config_path = home.join(".claude.json");

    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = config.get_mut("mcpServers") {
                    if let Some(obj) = servers.as_object_mut() {
                        obj.remove("blue");
                    }
                }
                std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
                println!("  ✓ Removed blue from ~/.claude.json");
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
    if let Ok(path) = which::which("blue") {
        println!("  ✓ blue found at {}", path.display());
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
                            println!("  ✗ {} (symlink points to wrong target)", skill_name.to_string_lossy());
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

    // Check MCP server
    println!("\nMCP Server:");
    let config_path = home.join(".claude.json");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = config.get("mcpServers") {
                    if servers.get("blue").is_some() {
                        println!("  ✓ blue configured in ~/.claude.json");

                        // Check if binary path is correct
                        if let Some(cmd) = servers["blue"].get("command").and_then(|c| c.as_str()) {
                            if std::path::Path::new(cmd).exists() {
                                println!("  ✓ Binary path valid");
                            } else {
                                println!("  ✗ Binary path invalid: {}", cmd);
                                issues += 1;
                            }
                        }
                    } else {
                        println!("  ✗ blue not configured in ~/.claude.json");
                        issues += 1;
                    }
                } else {
                    println!("  ✗ No mcpServers in ~/.claude.json");
                    issues += 1;
                }
            }
        }
    } else {
        println!("  ✗ ~/.claude.json not found");
        issues += 1;
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
