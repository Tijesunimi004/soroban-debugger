use crate::{DebuggerError, Result};
use soroban_env_host::Host;
use soroban_sdk::{Address, Env, InvokeError, Symbol, Val, Vec as SorobanVec};
use std::time::Instant;
use tracing::{info, warn};

/// Result of a contract execution including timing information
#[derive(Debug, serde::Serialize)]
pub struct ExecutionResult {
    pub result: String,
    pub execution_time_ms: f64,
}

/// Executes Soroban contracts in a test environment
pub struct ContractExecutor {
    env: Env,
    contract_address: Address,
}

impl ContractExecutor {
    /// Create a new contract executor
    pub fn new(wasm: Vec<u8>) -> Result<Self> {
        info!("Initializing contract executor");

        // Create a test environment
        let env = Env::default();

        // Register the contract with the WASM
        let contract_address = env.register(wasm.as_slice(), ());

        info!("Contract registered successfully");

        Ok(Self {
            env,
            contract_address,
        })
    }

    /// Execute a contract function
    pub fn execute(&self, function: &str, args: Option<&str>) -> Result<ExecutionResult> {
        info!("Executing function: {}", function);

        // Convert function name to Symbol
        let func_symbol = Symbol::new(&self.env, function);

        // Parse arguments (simplified for now)
        let parsed_args = if let Some(args_json) = args {
            self.parse_args(args_json)?
        } else {
            vec![]
        };

        // Create argument vector
        let args_vec = if parsed_args.is_empty() {
            SorobanVec::<Val>::new(&self.env)
        } else {
            SorobanVec::from_slice(&self.env, &parsed_args)
        };

        // Start timing
        let start = Instant::now();

        // Call the contract
        // try_invoke_contract returns Result<Result<Val, ConversionError>, Result<InvokeError, InvokeError>>
        let invoke_result = self.env.try_invoke_contract::<Val, InvokeError>(
            &self.contract_address,
            &func_symbol,
            args_vec,
        );

        // End timing
        let duration = start.elapsed();
        let execution_time_ms = duration.as_secs_f64() * 1000.0;

        match invoke_result {
            Ok(Ok(val)) => {
                info!("Function executed successfully in {:.2}ms", execution_time_ms);
                Ok(ExecutionResult {
                    result: format!("{:?}", val),
                    execution_time_ms,
                })
            }
            Ok(Err(conv_err)) => {
                warn!("Return value conversion failed: {:?}", conv_err);
                Ok(ExecutionResult {
                    result: format!("Error (Conversion): {:?}", conv_err),
                    execution_time_ms,
                })
            }
            Err(Ok(inv_err)) => {
                let err_msg = match inv_err {
                    InvokeError::Contract(code) => format!("Contract error code: {}", code),
                    InvokeError::Abort => "Contract execution aborted".to_string(),
                };
                warn!("{}", err_msg);
                Ok(ExecutionResult {
                    result: format!("Error: {}", err_msg),
                    execution_time_ms,
                })
            }
            Err(Err(inv_err)) => {
                warn!("Invocation error conversion failed: {:?}", inv_err);
                Ok(ExecutionResult {
                    result: format!("Error (Invocation Conversion): {:?}", inv_err),
                    execution_time_ms,
                })
            }
        }
    }

    /// Set initial storage state
    pub fn set_initial_storage(&mut self, _storage_json: String) -> Result<()> {
        // TODO: Implement storage initialization
        info!("Setting initial storage (not yet implemented)");
        Ok(())
    }

    /// Get the host instance
    pub fn host(&self) -> &Host {
        self.env.host()
    }

    /// Parse JSON arguments into contract values
    fn parse_args(&self, _args_json: &str) -> Result<Vec<Val>> {
        // TODO: Implement proper argument parsing
        // For now, return empty vec
        info!("Argument parsing not yet implemented");
        Ok(vec![])
    }
}
