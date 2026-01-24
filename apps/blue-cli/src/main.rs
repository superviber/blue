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

    // Check if Goose is installed
    let goose_check = Command::new("goose")
        .arg("--version")
        .output();

    match goose_check {
        Err(_) => {
            println!("Goose not found. Install it first:");
            println!("  pipx install goose-ai");
            println!("  # or");
            println!("  brew install goose");
            println!("\nSee https://github.com/block/goose for more options.");
            return Ok(());
        }
        Ok(output) if !output.status.success() => {
            println!("Goose check failed. Ensure it's properly installed.");
            return Ok(());
        }
        Ok(_) => {}
    }

    // Get the path to the blue binary
    let blue_binary = std::env::current_exe()?;

    // Build the extension command
    let extension_cmd = format!("{} mcp", blue_binary.display());

    println!("Starting Goose with Blue extension...");
    println!("  Extension: {}", extension_cmd);

    // Build goose command
    let mut cmd = Command::new("goose");
    cmd.arg("session");
    cmd.arg("--with-extension").arg(&extension_cmd);

    // Add model if specified
    if let Some(m) = model {
        cmd.arg("--model").arg(m);
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
