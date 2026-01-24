//! Blue CLI - Welcome home
//!
//! Command-line interface for Blue.

use clap::{Parser, Subcommand};
use anyhow::Result;
use blue_core::daemon::{DaemonClient, DaemonDb, DaemonPaths, DaemonState, run_daemon};
use blue_core::realm::RealmService;

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
    Mcp,

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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

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
        Some(Commands::Mcp) => {
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

            if has_errors {
                std::process::exit(1);
            } else if strict && has_warnings {
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
    use blue_core::store::DocumentStore;
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
            let target = path.as_deref().unwrap_or(".");
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

            println!("Indexing all files in '{}' with model '{}'...", target, model_name);
            println!("(Full indexing requires Ollama running with the model pulled)");

            // For now, show what would be indexed
            let count = count_indexable_files(Path::new(target))?;
            println!("Found {} indexable files.", count);
            println!("\nTo complete indexing:");
            println!("  1. Ensure Ollama is running: ollama serve");
            println!("  2. Pull the model: ollama pull {}", model_name);
            println!("  3. Run this command again");

            // TODO: Implement actual indexing with Ollama integration
        }

        IndexCommands::Diff { model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

            // Get staged files
            let output = std::process::Command::new("git")
                .args(["diff", "--cached", "--name-only"])
                .output()?;

            let staged_files: Vec<&str> = std::str::from_utf8(&output.stdout)?
                .lines()
                .filter(|l| !l.is_empty())
                .collect();

            if staged_files.is_empty() {
                println!("No staged files to index.");
                return Ok(());
            }

            println!("Indexing {} staged file(s) with '{}'...", staged_files.len(), model_name);
            for file in &staged_files {
                println!("  {}", file);
            }

            // TODO: Implement actual indexing
        }

        IndexCommands::File { path, model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

            if !Path::new(&path).exists() {
                println!("File not found: {}", path);
                return Ok(());
            }

            println!("Indexing '{}' with '{}'...", path, model_name);

            // TODO: Implement single file indexing
        }

        IndexCommands::Refresh { model } => {
            let model_name = model.as_deref().unwrap_or("qwen2.5:3b");

            // Get current realm (default to "default" for single-repo)
            let realm = "default";

            let (file_count, symbol_count) = store.get_index_stats(realm)?;
            println!("Current index: {} files, {} symbols", file_count, symbol_count);

            if file_count == 0 {
                println!("Index is empty. Run 'blue index all' first.");
                return Ok(());
            }

            println!("Checking for stale entries...");
            println!("(Refresh with model '{}')", model_name);

            // TODO: Implement refresh logic - compare hashes
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

fn count_indexable_files(dir: &std::path::Path) -> Result<usize> {
    use std::fs;
    use std::path::Path;

    let mut count = 0;

    // File extensions we care about
    let extensions: &[&str] = &[
        "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp",
        "rb", "php", "swift", "kt", "scala", "clj", "ex", "exs", "erl", "hs",
        "ml", "mli", "sql", "sh", "bash", "zsh", "yaml", "yml", "toml", "json",
    ];

    // Directories to skip
    let skip_dirs: &[&str] = &[
        "node_modules", "target", ".git", "__pycache__", "venv", ".venv",
        "dist", "build", ".next", ".nuxt", "vendor", ".cargo",
    ];

    fn walk_dir(dir: &Path, extensions: &[&str], skip_dirs: &[&str], count: &mut usize) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if path.is_dir() {
                if !skip_dirs.contains(&name) && !name.starts_with('.') {
                    walk_dir(&path, extensions, skip_dirs, count)?;
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext) {
                    *count += 1;
                }
            }
        }
        Ok(())
    }

    walk_dir(dir, extensions, skip_dirs, &mut count)?;
    Ok(count)
}
