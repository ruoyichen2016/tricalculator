#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_HOME="${CARGO_HOME:-$ROOT/.cargo-home}"

cargo fmt --all -- --check
bash scripts/check_architecture.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -q -p pcgen-cli
pcgen_bin="$ROOT/target/debug/pcgen"
[ -x "$pcgen_bin" ] || pcgen_bin="$ROOT/target/debug/pcgen.exe"
export PCGEN_BIN="$pcgen_bin"
"$pcgen_bin" demo >/dev/null
"$pcgen_bin" import-report tests/fixtures/campaign-mini/mini.pcc >/dev/null

if bash scripts/check_testdata.sh --quiet >/dev/null 2>&1; then
    # broad oracle runs once inside coverage_gate.py (structured JSON).
    python3 scripts/coverage_gate.py
    archive_bin="${ARCHIVE_PCGEN:-$ROOT/../reference/pcgen-rust-archive/target/debug/pcgen}"
    if [ -x "$archive_bin" ]; then
        ARCHIVE_PCGEN="$archive_bin" python3 scripts/snapshot_diff_archive.py --strict
    else
        echo "SKIP archive differential: $archive_bin not built" >&2
        echo "     build with: (cd ../reference/pcgen-rust-archive && cargo build)" >&2
    fi
else
    echo "WARNING: pinned PCGen test data not found." >&2
    echo "         running the no-data smoke gate only; real-data gates are skipped." >&2
    echo "         run scripts/fetch_testdata.sh for the full gate." >&2
    python3 scripts/coverage_gate.py --allow-skip
fi
