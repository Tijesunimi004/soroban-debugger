/// REPL session management with history and state
///
/// Handles user input, command history, and persistent state across
/// multiple function calls within a single REPL session.
use super::commands::ReplCommand;
use super::executor::ReplExecutor;
use super::ReplConfig;
use crate::ui::formatter::Formatter;
use crate::Result;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{DefaultEditor, Editor};
use std::path::PathBuf;

/// REPL session state and editor
pub struct ReplSession {
    editor: Editor<(), FileHistory>,
    config: ReplConfig,
    executor: ReplExecutor,
    history_path: PathBuf,
}

impl ReplSession {
    /// Create a new REPL session
    pub fn new(config: ReplConfig) -> Result<Self> {
        let history_path = dirs::home_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(".soroban_repl_history");

        let mut editor = DefaultEditor::new()
            .map_err(|e| miette::miette!("Failed to initialize REPL editor: {}", e))?;

        // Load history if it exists
        let _ = editor.load_history(&history_path);

        let executor = ReplExecutor::new(&config)?;

        Ok(ReplSession {
            editor,
            config,
            executor,
            history_path,
        })
    }

    /// Run the REPL event loop
    pub async fn run(&mut self) -> Result<()> {
        self.print_welcome();

        loop {
            let prompt = format!(
                "{}> ",
                Formatter::info(
                    format!(
                        "soroban-debug repl [{}]",
                        self.config.contract_path.display()
                    )
                    .as_str()
                )
            );

            match self.editor.readline(&prompt) {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = self.editor.add_history_entry(line.clone());

                    match self.execute_command(&line).await {
                        Ok(true) => break, // Exit requested
                        Ok(false) => {}    // Continue
                        Err(e) => {
                            eprintln!("{}", Formatter::error(format!("Error: {}", e).as_str()));
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("\n{}", Formatter::info("Use 'exit' or Ctrl+D to quit"));
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl+D
                    println!("\n{}", Formatter::success("Goodbye!"));
                    break;
                }
                Err(e) => {
                    eprintln!("{}", Formatter::error(format!("Error: {}", e).as_str()));
                }
            }
        }

        // Save history
        let _ = self.editor.save_history(&self.history_path);

        Ok(())
    }

    /// Execute a single command
    async fn execute_command(&mut self, line: &str) -> Result<bool> {
        let cmd = ReplCommand::parse(line)?;

        match cmd {
            ReplCommand::Exit => Ok(true),
            ReplCommand::Help => {
                self.print_help();
                Ok(false)
            }
            ReplCommand::History => {
                self.print_history();
                Ok(false)
            }
            ReplCommand::Storage => {
                self.executor.inspect_storage()?;
                Ok(false)
            }
            ReplCommand::Call { function, args } => {
                self.executor.call_function(&function, args).await?;
                Ok(false)
            }
            ReplCommand::Clear => {
                // Print ANSI escape code to clear screen
                print!("\x1B[2J\x1B[1;1H");
                Ok(false)
            }
        }
    }

    fn print_welcome(&self) {
        println!("{}", Formatter::success("=== Soroban Debug REPL ==="));
        println!(
            "{}",
            Formatter::info(format!("Contract: {}", self.config.contract_path.display()).as_str())
        );
        println!("{}", Formatter::info("Type 'help' for available commands"));
        println!();
    }

    fn print_help(&self) {
        println!();
        println!("{}", Formatter::success("Available Commands:"));
        println!(
            "  {} <func> [args...]  Call a contract function",
            Formatter::info("call")
        );
        println!(
            "  {}                 Show contract storage state",
            Formatter::info("storage")
        );
        println!(
            "  {}                 Show command history",
            Formatter::info("history")
        );
        println!(
            "  {}                    Clear the screen",
            Formatter::info("clear")
        );
        println!(
            "  {}                     Show this help message",
            Formatter::info("help")
        );
        println!(
            "  {}                     Exit the REPL",
            Formatter::info("exit")
        );
        println!();
    }

    fn print_history(&self) {
        println!();
        println!("{}", Formatter::success("Command History:"));
        for (idx, item) in self.editor.history().iter().enumerate() {
            println!("  {}: {}", idx, item);
        }
        println!();
    }
}
