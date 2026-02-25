#!/usr/bin/env bash
set -euo pipefail

# Determine repo root relative to this script's location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"

# If you put this script in a subdir (e.g., scripts/), uncomment the next line:
# REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

if [[ ! -f "Cargo.toml" ]]; then
  echo "error: Cargo.toml not found in $REPO_ROOT"
  echo "hint: move this script to the repo root, or adjust REPO_ROOT to point to it."
  exit 1
fi

echo "==> Repo root: $REPO_ROOT"
echo "==> Rust toolchain"
rustc -Vv
cargo -V
cargo clippy -V

echo
echo "==> Formatting (cargo fmt --check)"
cargo fmt --all -- --check

echo
echo "==> Clippy (deny warnings)"
cargo clippy --workspace --all-targets --all-features -- -D warnings

echo
echo "==> Tests (deny rustc warnings via RUSTFLAGS)"
RUSTFLAGS="-D warnings" cargo test --workspace --all-features