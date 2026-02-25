use assert_cmd::Command;
use jsonschema::JSONSchema;
use serde_json::Value;
use std::fs;

#[test]
fn test_json_output_schema_validation() {
    let wasm_path = "tests/fixtures/wasm/counter.wasm";
    let schema_path = "tests/schemas/execution_output.json";

    // Run the debugger with --output json and --quiet to ensure only JSON is on stdout
    let mut cmd = Command::cargo_bin("soroban-debug").unwrap();
    let output = cmd
        .arg("--quiet")
        .arg("run")
        .arg("--contract")
        .arg(wasm_path)
        .arg("--function")
        .arg("increment")
        .arg("--output")
        .arg("json")
        .arg("--show-events")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("Stdout is not valid UTF-8");

    // We expect stdout to be valid JSON
    let json_val: Value =
        serde_json::from_str(&stdout).expect(&format!("Failed to parse JSON output: {}", stdout));

    // Load schema
    let schema_content = fs::read_to_string(schema_path).expect("Failed to read schema file");
    let schema_json: Value =
        serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");

    let compiled = JSONSchema::compile(&schema_json).expect("Failed to compile schema");
    let result = compiled.validate(&json_val);

    if let Err(errors) = result {
        let mut error_msgs = Vec::new();
        for error in errors {
            error_msgs.push(format!(
                "Property: {}, Error: {}",
                error.instance_path, error
            ));
        }
        panic!("JSON Schema validation failed:\n{}", error_msgs.join("\n"));
    }
}
