#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_HOME="${CARGO_HOME:-$ROOT/.cargo-home}"
export PYTHONDONTWRITEBYTECODE=1

echo "==> [1/5] Checking code formatting..."
cargo fmt --all -- --check

echo "==> [2/5] Checking architectural guardrails..."
bash scripts/check_architecture.sh

echo "==> [3/5] Checking clippy lints..."
cargo clippy --workspace --all-targets -- -D warnings -A clippy::too_many_arguments -A clippy::large_enum_variant -A clippy::manual_div_ceil -A clippy::needless_return_with_question_mark -A clippy::needless_borrow -A clippy::unnecessary_get_then_check -A clippy::useless_vec -A clippy::collapsible_if -A clippy::field_reassign_with_default

echo "==> [4/5] Running workspace tests..."
cargo test --workspace

echo "==> [5/5] Testing packaging and release tooling..."
python3 scripts/test_package.py

echo "All checks passed successfully."
