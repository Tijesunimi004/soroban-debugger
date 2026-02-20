use anyhow::Result;
use clap::Parser;
use soroban_debugger::cli::{Cli, Commands};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();
    let verbosity = cli.verbosity();

    // Initialize logging with verbosity-aware level
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| verbosity.to_log_level().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Execute command with verbosity
    match cli.command {
        Commands::Run(args) => {
            soroban_debugger::cli::commands::run(args, verbosity)?;
        }
        Commands::Interactive(args) => {
            soroban_debugger::cli::commands::interactive(args, verbosity)?;
        }
        Commands::Inspect(args) => {
            soroban_debugger::cli::commands::inspect(args, verbosity)?;
        }
        Commands::Optimize(args) => {
            soroban_debugger::cli::commands::optimize(args, verbosity)?;
        }
        Commands::UpgradeCheck(args) => {
            soroban_debugger::cli::commands::upgrade_check(args, verbosity)?;
        }
    }

    Ok(())
}
