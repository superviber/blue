//! Blue CLI - Welcome home
//!
//! Command-line interface for Blue.

use clap::{Parser, Subcommand};
use anyhow::Result;

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
    }

    Ok(())
}
