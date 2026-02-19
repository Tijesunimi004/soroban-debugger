use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "soroban-debug")]
#[command(about = "A debugger for Soroban smart contracts", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run a contract function with the debugger
    Run(RunArgs),

    /// Start an interactive debugging session
    Interactive(InteractiveArgs),

    /// Inspect contract information without executing
    Inspect(InspectArgs),
}

#[derive(Parser)]
pub struct RunArgs {
    /// Path to the contract WASM file
    #[arg(short, long)]
    pub contract: PathBuf,

    /// Function name to execute
    #[arg(short, long)]
    pub function: String,

    /// Function arguments as JSON array (e.g., '["arg1", "arg2"]')
    #[arg(short, long)]
    pub args: Option<String>,

    /// Initial storage state as JSON object
    #[arg(short, long)]
    pub storage: Option<String>,

    /// Set breakpoint at function name
    #[arg(short, long)]
    pub breakpoint: Vec<String>,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Parser)]
pub struct InteractiveArgs {
    /// Path to the contract WASM file
    #[arg(short, long)]
    pub contract: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Parser)]
pub struct InspectArgs {
    /// Path to the contract WASM file
    #[arg(short, long)]
    pub contract: PathBuf,

    /// Show exported functions
    #[arg(long)]
    pub functions: bool,

    /// Show contract metadata
    #[arg(long)]
    pub metadata: bool,
}
