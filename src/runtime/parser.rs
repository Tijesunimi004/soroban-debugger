//! Contract function argument parsing and type-aware normalisation.
//!
//! Accepts raw JSON strings from the CLI, inspects the WASM contract spec to
//! determine expected parameter types, and wraps values in the typed-annotation
//! envelope (`{"type": "...", "value": ...}`) that [`ArgumentParser`] expects.
//!
//! # Key responsibilities
//! - Parse raw `--args` JSON into [`soroban_sdk::Val`] slices.
//! - Normalise `Option<T>` and `Tuple<…>` arguments automatically so callers
//!   do not need to spell out the annotation envelope themselves.

use crate::{DebuggerError, Result};
use serde_json::Value as JsonValue;
use soroban_sdk::{Env, Val};
use tracing::warn;

/// Parse a raw JSON argument string into a `Vec<Val>` using the given environment.
///
/// `wasm_bytes` is used to look up the function signature so that `Option` and
/// `Tuple` parameters are wrapped in the typed-annotation envelope automatically.
pub fn parse_args(
    env: &Env,
    wasm_bytes: &[u8],
    function: &str,
    args_json: &str,
) -> Result<Vec<Val>> {
    let parser = crate::utils::ArgumentParser::new(env.clone());
    let normalized = normalize_args_for_function(wasm_bytes, function, args_json)?;
    parser.parse_args_string(&normalized).map_err(|e| {
        warn!("Failed to parse arguments: {}", e);
        DebuggerError::InvalidArguments(e.to_string()).into()
    })
}

/// Normalise argument JSON against the contract's function signature.
///
/// Wraps `Option<T>` arguments in `{"type":"option","value":…}` and
/// `Tuple<…>` arguments in `{"type":"tuple","arity":N,"value":[…]}` so that
/// the downstream [`ArgumentParser`] can handle them without caller involvement.
fn normalize_args_for_function(
    wasm_bytes: &[u8],
    function: &str,
    args_json: &str,
) -> Result<String> {
    let signatures = crate::utils::wasm::parse_function_signatures(wasm_bytes)?;
    let Some(signature) = signatures.into_iter().find(|sig| sig.name == function) else {
        return Ok(args_json.to_string());
    };

    let mut args_value: JsonValue = serde_json::from_str(args_json)
        .map_err(|e| DebuggerError::InvalidArguments(format!("Invalid JSON in --args: {}", e)))?;

    let JsonValue::Array(args) = &mut args_value else {
        return Ok(args_json.to_string());
    };

    for (arg, param) in args.iter_mut().zip(signature.params.iter()) {
        if param.type_name.starts_with("Option<") {
            if !is_typed_annotation(arg) {
                *arg = serde_json::json!({"type": "option", "value": arg.clone()});
            }
            continue;
        }

        if param.type_name.starts_with("Tuple<") {
            let arity = tuple_arity_from_type_name(&param.type_name).ok_or_else(|| {
                DebuggerError::InvalidArguments(format!(
                    "Invalid tuple type in function spec for '{}': {}",
                    param.name, param.type_name
                ))
            })?;

            let JsonValue::Array(actual_arr) = arg else {
                return Err(DebuggerError::InvalidArguments(format!(
                    "Argument '{}' expects tuple with {} elements, got {}",
                    param.name,
                    arity,
                    json_type_name(arg)
                ))
                .into());
            };

            if actual_arr.len() != arity {
                return Err(DebuggerError::InvalidArguments(format!(
                    "Tuple arity mismatch: expected {}, got {}",
                    arity,
                    actual_arr.len()
                ))
                .into());
            }

            *arg =
                serde_json::json!({"type": "tuple", "arity": arity, "value": actual_arr.clone()});
        }
    }

    serde_json::to_string(&args_value).map_err(|e| {
        DebuggerError::ExecutionError(format!("Failed to normalise arguments JSON: {}", e)).into()
    })
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn tuple_arity_from_type_name(type_name: &str) -> Option<usize> {
    let inner = type_name.strip_prefix("Tuple<")?.strip_suffix('>')?;
    if inner.trim().is_empty() {
        return Some(0);
    }
    let mut depth = 0usize;
    let mut arity = 1usize;
    for ch in inner.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => arity += 1,
            _ => {}
        }
    }
    Some(arity)
}

fn is_typed_annotation(value: &JsonValue) -> bool {
    matches!(
        value,
        JsonValue::Object(obj) if obj.get("type").is_some() && obj.get("value").is_some()
    )
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::tuple_arity_from_type_name;

    #[test]
    fn tuple_arity_counts_top_level_types() {
        assert_eq!(tuple_arity_from_type_name("Tuple<U32, Symbol>"), Some(2));
        assert_eq!(
            tuple_arity_from_type_name("Tuple<U32, Option<Vec<Symbol>>, Map<U32, String>>"),
            Some(3)
        );
    }

    #[test]
    fn tuple_arity_zero_for_empty() {
        assert_eq!(tuple_arity_from_type_name("Tuple<>"), Some(0));
        assert_eq!(tuple_arity_from_type_name("Tuple<  >"), Some(0));
    }

    #[test]
    fn tuple_arity_returns_none_for_bad_prefix() {
        assert_eq!(tuple_arity_from_type_name("Vec<U32>"), None);
    }
}
