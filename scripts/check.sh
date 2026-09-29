#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_HOME="${CARGO_HOME:-$ROOT/.cargo-home}"

cargo fmt --all -- --check
bash scripts/check_architecture.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p pcgen-web --offline
cargo build -q -p pcgen-cli
python3 scripts/test_package.py
pcgen_bin="$ROOT/target/debug/pcgen"
[ -x "$pcgen_bin" ] || pcgen_bin="$ROOT/target/debug/pcgen.exe"
export PCGEN_BIN="$pcgen_bin"
"$pcgen_bin" demo >/dev/null
"$pcgen_bin" import-report tests/fixtures/campaign-mini/mini.pcc >/dev/null
m9_smoke="$ROOT/target/m9-smoke-character.json"
"$pcgen_bin" create --pcc tests/fixtures/campaign-mini/mini.pcc \
    --name Smoke --race Human --class Fighter --level 1 --point-buy 25 \
    --out "$m9_smoke" >/dev/null
"$pcgen_bin" level-up "$m9_smoke" --pcc tests/fixtures/campaign-mini/mini.pcc \
    --class Fighter --hp 6 --out "${m9_smoke}.2" >/dev/null
rm -f "$m9_smoke" "${m9_smoke}.2"

if bash scripts/check_testdata.sh --quiet >/dev/null 2>&1; then
    python3 scripts/cohort_matrix.py --check
    python3 scripts/deep_field_oracle.py --check
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
