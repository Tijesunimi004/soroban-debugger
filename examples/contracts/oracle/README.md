# Oracle Price Feed Contract

A Soroban smart-contract example that stores external asset prices on-chain
and demonstrates how to **debug stale or incorrect price data** with the
Soroban Debugger.

---

## Overview

| Function | Description |
|---|---|
| `initialize(admin, stale_ttl)` | Bootstrap the oracle – sets the admin address and the staleness TTL in seconds |
| `set_price(asset, price)` | Admin-only; writes a price (micro-units) and the current ledger timestamp |
| `get_price(asset)` | Returns the latest price in micro-units |
| `get_timestamp(asset)` | Returns the UNIX timestamp of the last price update |
| `is_stale(asset)` | Returns `true` when `now − last_timestamp > stale_ttl` |
| `get_stale_ttl()` | Returns the configured staleness window (seconds) |

Prices are stored in **micro-units** so integer arithmetic avoids floating-point
issues.  For example, a price of `$1.10` is stored as `1_100_000`.

---

## Building

```bash
# From the repo root
cd examples/contracts/oracle

# Install the wasm32 target if not already present
rustup target add wasm32-unknown-unknown

# Build a release WASM
cargo build --target wasm32-unknown-unknown --release

# The compiled WASM lands here:
#   target/wasm32-unknown-unknown/release/soroban_oracle.wasm
```

Run the unit tests (uses the Soroban test framework, no WASM target needed):

```bash
cargo test
```

---

## Storage Layout

The contract uses two storage tiers:

| Storage tier | Key | Value | Notes |
|---|---|---|---|
| `instance` | `Admin` | `Address` | Set once at init |
| `instance` | `StaleTtl` | `u64` (seconds) | Set once at init |
| `persistent` | `Price(asset)` | `i128` (micro-units) | Updated on every `set_price` |
| `persistent` | `Timestamp(asset)` | `u64` (UNIX seconds) | Updated on every `set_price` |

Using `persistent` storage for price/timestamp entries means each asset's data
survives ledger close independently and can be observed individually in a
storage diff.

---

## Debugging Stale Price Scenarios

### Scenario: Consumer contract rejects a stale price

A downstream lending protocol calls `is_stale("XLM")` and receives `true`,
causing a transaction to fail.  Use the debugger to trace exactly when the
price went stale.

### Step 1 – Reproduce the failing invocation

```bash
soroban-debugger invoke \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --id oracle_contract \
  --fn is_stale \
  --arg '"XLM"' \
  --snapshot examples/snapshot.json \
  --ledger-time 1_720_000_000
```

Expected output (stale price):

```
Result: true
```

### Step 2 – Inspect stored timestamp

```bash
soroban-debugger invoke \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --id oracle_contract \
  --fn get_timestamp \
  --arg '"XLM"' \
  --snapshot examples/snapshot.json
```

Sample output:

```
Result: 1719999000
```

Calculate the age:

```
age = 1_720_000_000 − 1_719_999_000 = 1_000 seconds
stale_ttl = 300 seconds
→ price is stale (1000 > 300)
```

### Step 3 – View storage diff after a fresh price push

Run `set_price` and capture the storage diff to confirm both `Price` and
`Timestamp` entries are updated atomically:

```bash
soroban-debugger invoke \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --id oracle_contract \
  --fn set_price \
  --arg '"XLM"' --arg '1100000' \
  --snapshot examples/snapshot.json \
  --diff
```

Example storage diff output:

```
Storage diff for contract oracle_contract
─────────────────────────────────────────
  MODIFIED  persistent::Price("XLM")
    before: 1_050_000
    after:  1_100_000

  MODIFIED  persistent::Timestamp("XLM")
    before: 1_719_999_000
    after:  1_720_000_000
─────────────────────────────────────────
```

The diff confirms that both the price and its timestamp moved forward in the
same invocation – no partial update is possible.

### Step 4 – Confirm price is fresh

```bash
soroban-debugger invoke \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --id oracle_contract \
  --fn is_stale \
  --arg '"XLM"' \
  --snapshot examples/snapshot.json \
  --ledger-time 1_720_000_000
```

```
Result: false
```

---

## Watch Mode – monitor staleness in real time

Use the debugger's watch mode to alert you whenever an asset's price crosses
the staleness threshold during a batch replay:

```bash
soroban-debugger watch \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --snapshot examples/snapshot.json \
  --watch 'persistent::Timestamp("XLM")' \
  --batch examples/batch_args.json
```

The debugger will emit a diff event each time the watched key changes.

---

## Step-Through Debugging

Open an interactive debug session to step through `is_stale` instruction by
instruction and inspect each storage read:

```bash
soroban-debugger debug \
  --wasm target/wasm32-unknown-unknown/release/soroban_oracle.wasm \
  --fn is_stale \
  --arg '"ETH"' \
  --snapshot examples/snapshot.json
```

Useful debugger commands inside the session:

| Command | Description |
|---|---|
| `n` / `next` | Advance one WASM instruction |
| `s storage` | Dump current contract storage |
| `w persistent::Timestamp("ETH")` | Watch a specific storage key |
| `b is_stale` | Set a breakpoint on function entry |
| `q` | Quit the session |

---

## Common Root Causes for Stale Prices

| Symptom | Likely Cause | Fix |
|---|---|---|
| `is_stale` always `true` | Relayer stopped submitting `set_price` | Restart or re-deploy the price-feed relayer |
| Timestamp frozen | `set_price` transactions failing auth | Verify admin key rotation |
| Price correct but timestamp old | Clock skew between relayer and ledger | Sync relayer to ledger timestamp source |
| Wrong asset symbol | Ticker case mismatch (`xlm` vs `XLM`) | Normalise to uppercase before calling |
