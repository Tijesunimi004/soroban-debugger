//! Soroban contract executor — public façade for the runtime sub-modules.
//!
//! [`ContractExecutor`] is the main entry-point for all contract execution.
//! Internally it delegates to four focused sub-modules:
//!
//! - [`super::loader`]  — WASM loading and environment bootstrap.
//! - [`super::parser`]  — Argument parsing and type-aware normalisation.
//! - [`super::invoker`] — Function invocation with timeout protection.
//! - [`super::result`]  — Result types and formatting helpers.

use crate::inspector::budget::MemorySummary;
use crate::runtime::mocking::{MockCallLogEntry, MockContractDispatcher, MockRegistry};
use crate::{DebuggerError, Result};

use soroban_env_host::Host;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use tracing::info;

// ── re-exports so callers never need to import sub-modules directly ───────────
pub use crate::runtime::mocking::MockCallLogEntry as MockCallEntry;
pub use crate::runtime::result::{ExecutionRecord, InstructionCounts, StorageSnapshot};

/// Executes Soroban contracts in a test environment.
pub struct ContractExecutor {
    env: Env,
    contract_address: Address,
    last_execution: Option<ExecutionRecord>,
    last_memory_summary: Option<MemorySummary>,
    mock_registry: Arc<Mutex<MockRegistry>>,
    wasm_bytes: Vec<u8>,
    timeout_secs: u64,
    error_db: crate::debugger::error_db::ErrorDatabase,
}

impl ContractExecutor {
    /// Create a new contract executor by loading and registering `wasm`.
    #[tracing::instrument(skip_all)]
    pub fn new(wasm: Vec<u8>) -> Result<Self> {
        let loaded = crate::runtime::loader::load_contract(&wasm)?;
        Ok(Self {
            env: loaded.env,
            contract_address: loaded.contract_address,
            last_execution: None,
            last_memory_summary: None,
            mock_registry: Arc::new(Mutex::new(MockRegistry::default())),
            wasm_bytes: wasm,
            timeout_secs: 30,
            error_db: loaded.error_db,
        })
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    pub fn set_timeout(&mut self, secs: u64) {
        self.timeout_secs = secs;
    }

    /// Enable auth mocking for interactive/test-like execution flows (e.g. REPL).
    pub fn enable_mock_all_auths(&self) {
        self.env.mock_all_auths();
    }

    /// Generate a test account address (StrKey) for REPL shorthand aliases.
    pub fn generate_repl_account_strkey(&self) -> Result<String> {
        let addr = Address::generate(&self.env);
        let debug = format!("{:?}", addr);
        for token in debug
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|s| !s.is_empty())
        {
            if (token.starts_with('G') || token.starts_with('C')) && token.len() >= 10 {
                return Ok(token.to_string());
            }
        }
        Err(DebuggerError::ExecutionError(format!(
            "Failed to format generated REPL address alias (debug={debug})"
        ))
        .into())
    }

    /// Execute a contract function.
    #[tracing::instrument(skip(self), fields(function = function))]
    pub fn execute(&mut self, function: &str, args: Option<&str>) -> Result<String> {
        // 1. Validate function exists in the WASM export section.
        let exported = crate::utils::wasm::parse_functions(&self.wasm_bytes)?;
        if !exported.contains(&function.to_string()) {
            return Err(DebuggerError::InvalidFunction(function.to_string()).into());
        }

        // 2. Parse arguments.
        let parsed_args = match args {
            Some(json) => {
                crate::runtime::parser::parse_args(&self.env, &self.wasm_bytes, function, json)?
            }
            None => vec![],
        };

        // 3. Invoke and capture the result.
        let storage_fn = || self.get_storage_snapshot();
        let (display, record) = crate::runtime::invoker::invoke_function(
            &self.env,
            &self.contract_address,
            &self.error_db,
            function,
            parsed_args,
            self.timeout_secs,
            storage_fn,
        )?;

        self.last_execution = Some(record);
        Ok(display)
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    pub fn last_execution(&self) -> Option<&ExecutionRecord> {
        self.last_execution.as_ref()
    }
    pub fn last_memory_summary(&self) -> Option<&MemorySummary> {
        self.last_memory_summary.as_ref()
    }
    pub fn set_initial_storage(&mut self, _storage_json: String) -> Result<()> {
        info!("Setting initial storage (not yet implemented)");
        Ok(())
    }
    pub fn set_mock_specs(&mut self, specs: &[String]) -> Result<()> {
        let registry = MockRegistry::from_cli_specs(&self.env, specs)?;
        self.set_mock_registry(registry)
    }
    pub fn set_mock_registry(&mut self, registry: MockRegistry) -> Result<()> {
        self.mock_registry = Arc::new(Mutex::new(registry));
        self.install_mock_dispatchers()
    }
    pub fn get_mock_call_log(&self) -> Vec<MockCallLogEntry> {
        self.mock_registry
            .lock()
            .map(|r| r.calls().to_vec())
            .unwrap_or_default()
    }
    pub fn get_instruction_counts(&self) -> Result<InstructionCounts> {
        Ok(InstructionCounts {
            function_counts: Vec::new(),
            total: 0,
        })
    }
    pub fn host(&self) -> &Host {
        self.env.host()
    }
    pub fn get_auth_tree(&self) -> Result<Vec<crate::inspector::auth::AuthNode>> {
        crate::inspector::auth::AuthInspector::get_auth_tree(&self.env)
    }
    pub fn get_events(&self) -> Result<Vec<crate::inspector::events::ContractEvent>> {
        crate::inspector::events::EventInspector::get_events(self.env.host())
    }
    pub fn get_storage_snapshot(&self) -> Result<HashMap<String, String>> {
        Ok(crate::inspector::storage::StorageInspector::capture_snapshot(self.env.host()))
    }
    pub fn get_ledger_snapshot(&self) -> Result<soroban_ledger_snapshot::LedgerSnapshot> {
        Ok(self.env.to_ledger_snapshot())
    }
    pub fn finish(
        &mut self,
    ) -> Result<(
        soroban_env_host::storage::Footprint,
        soroban_env_host::storage::Storage,
    )> {
        let dummy_env = Env::default();
        let dummy_addr = Address::generate(&dummy_env);
        let old_env = std::mem::replace(&mut self.env, dummy_env);
        self.contract_address = dummy_addr;
        let host = old_env.host().clone();
        drop(old_env);
        let (storage, _events) = host.try_finish().map_err(|e| {
            DebuggerError::ExecutionError(format!(
                "Failed to finalize host execution tracking: {:?}",
                e
            ))
        })?;
        Ok((storage.footprint.clone(), storage))
    }
    pub fn snapshot_storage(&self) -> Result<StorageSnapshot> {
        let storage = self
            .env
            .host()
            .with_mut_storage(|s| Ok(s.clone()))
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to snapshot storage: {:?}", e))
            })?;
        Ok(StorageSnapshot { storage })
    }
    pub fn restore_storage(&mut self, snapshot: &StorageSnapshot) -> Result<()> {
        self.env
            .host()
            .with_mut_storage(|s| {
                *s = snapshot.storage.clone();
                Ok(())
            })
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to restore storage: {:?}", e))
            })?;
        info!("Storage state restored (dry-run rollback)");
        Ok(())
    }
    pub fn get_diagnostic_events(&self) -> Result<Vec<soroban_env_host::xdr::ContractEvent>> {
        Ok(self
            .env
            .host()
            .get_diagnostic_events()
            .map_err(|e| {
                DebuggerError::ExecutionError(format!("Failed to get diagnostic events: {}", e))
            })?
            .0
            .into_iter()
            .map(|he| he.event)
            .collect())
    }

    // ── private helpers ───────────────────────────────────────────────────────

    fn install_mock_dispatchers(&self) -> Result<()> {
        let ids = self
            .mock_registry
            .lock()
            .map(|r| r.mocked_contract_ids())
            .map_err(|_| DebuggerError::ExecutionError("Mock registry lock poisoned".into()))?;

        for contract_id in ids {
            let address = self.parse_contract_address(&contract_id)?;
            let dispatcher =
                MockContractDispatcher::new(contract_id.clone(), Arc::clone(&self.mock_registry))
                    .boxed();
            self.env
                .host()
                .register_test_contract(address.to_object(), dispatcher)
                .map_err(|e| {
                    DebuggerError::ExecutionError(format!(
                        "Failed to register test contract: {}",
                        e
                    ))
                })?;
        }
        Ok(())
    }

    fn parse_contract_address(&self, contract_id: &str) -> Result<Address> {
        catch_unwind(AssertUnwindSafe(|| {
            Address::from_str(&self.env, contract_id)
        }))
        .map_err(|_| {
            DebuggerError::InvalidArguments(format!("Invalid contract id in --mock: {contract_id}"))
                .into()
        })
    }
}
