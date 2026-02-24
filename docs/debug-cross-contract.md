
```markdown
# Debugging Cross-Contract Calls in Soroban

This tutorial shows how to **debug contracts that call other contracts** in Soroban.  
We’ll create a simple **caller + callee** example, set breakpoints in both contracts, and inspect the call stack and event logs.

---

## 1. Directory Structure

```

examples/contracts/cross-contract/
├── callee_contract.rs as there are no example contracts 
├── caller_contract.rs
└── integration_test.rs

````

---

## 2. Callee Contract

Create `examples/contracts/cross-contract/callee_contract.rs`:

```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct CalleeContract;

#[contractimpl]
impl CalleeContract {
    // Increment a value and emit an event
    pub fn increment(env: Env, value: i32) -> i32 {
        let new_value = value + 1;
        env.events().publish("incremented", new_value);
        new_value
    }
}
````

**Notes:**

* Emits an event `"incremented"` each time the function is called.

---

## 3. Caller Contract

Create `examples/contracts/cross-contract/caller_contract.rs`:

```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address};

#[contract]
pub struct CallerContract;

#[contractimpl]
impl CallerContract {
    // Call the CalleeContract increment function
    pub fn call_increment(env: Env, callee: Address, value: i32) -> i32 {
        env.invoke_contract::<i32>(&callee, &("increment", value))
    }
}
```

**Notes:**

* Uses `invoke_contract` to call the callee contract.
* Demonstrates a cross-contract call.

---

## 4. Integration Test

Create `examples/contracts/cross-contract/integration_test.rs`:

```rust
#![cfg(test)]
use soroban_sdk::{Env, Address};
use cross_contract::{CallerContractClient, CalleeContractClient};

#[test]
fn test_cross_contract_call() {
    let env = Env::default();

    // Deploy CalleeContract
    let callee_id = env.register_contract(None, CalleeContractClient);
    let callee = Address::from_contract_id(callee_id.clone());

    // Deploy CallerContract
    let caller_id = env.register_contract(None, CallerContractClient);
    let caller = Address::from_contract_id(caller_id.clone());

    // Call Callee directly
    let result_direct = CalleeContractClient::increment(&env, &callee, 5);
    assert_eq!(result_direct, 6);

    // Call Callee via Caller
    let result_via_caller = CallerContractClient::call_increment(&env, &caller, &callee, 5);
    assert_eq!(result_via_caller, 6);

    // Verify event emitted
    let events = env.events().all();
    assert_eq!(events.len(), 2); // one for direct, one for via caller
}
```

**Notes:**

* Tests direct call vs cross-contract call.
* Verifies events are logged even across contract boundaries.

---

## 5. Debugging Steps

1. Compile the contracts:

```bash
cargo build --release
```

2. Launch the debugger on the CallerContract:

```bash
soroban-debugger examples/contracts/cross-contract/caller_contract.wasm
```

3. Set breakpoints:

```text
break caller_contract.rs:10  # Before calling callee
break callee_contract.rs:7   # Inside increment function
```

4. Step through the debugger:

```text
step        # Move to next instruction
bt          # View call stack
```

**Expected Call Stack Output:**

```
Frame 0: CallerContract::call_increment
Frame 1: CalleeContract::increment
```

---

## 6. Inspect Event Logs

Even across contract calls:

```text
Event: "incremented" = 6
Event: "incremented" = 6
```

* Events emitted in the callee are visible in the debugger.

---

## 7. Key Takeaways

* Cross-contract calls appear in the **call stack**.
* Breakpoints work in **both caller and callee**.
* Event logs from callee are observable.
* Step-by-step debugging helps trace issues in multi-contract interactions.

---

## 8. Git Workflow

```bash
git checkout -b docs/tutorial-cross-contract
mkdir -p examples/contracts/cross-contract
# Add the two contracts and integration_test.rs
# Add docs/tutorials/debug-cross-contract.md
git add examples/contracts/cross-contract/*.rs docs/tutorials/debug-cross-contract.md
git commit -m "docs: add cross-contract debugging tutorial"
git push origin docs/tutorial-cross-contract
```

---

## 9. Next Steps

* Try nested cross-contract calls and watch the stack grow.
* Add more complex callee logic and test how the caller handles it.
* Combine debugging with unit tests for automated verification.

```

```
