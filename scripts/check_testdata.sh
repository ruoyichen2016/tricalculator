#!/usr/bin/env bash
# Verify that a PCGen test-data checkout matches scripts/testdata.lock.json.
# Exit 0 = complete and matching, 1 = missing/corrupt.
#
# Usage:
#   scripts/check_testdata.sh [DATA_ROOT] [--quiet] [--no-hash]
#
# DATA_ROOT defaults to $PCGEN_TESTDATA_DIR or <workspace>/raw/pcgen-master,
# matching the paths hard-coded by the integration tests:
#   <crate>/../../../raw/pcgen-master/...
#
# All expectations (files, hashes, minimum counts) come from the lock file,
# so bumping the data revision only requires editing scripts/testdata.lock.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE="$(cd "$ROOT/.." && pwd)"
DEFAULT_DATA_ROOT="$WORKSPACE/raw/pcgen-master"
LOCK="$ROOT/scripts/testdata.lock.json"

QUIET=0
CHECK_HASH=1
DATA_ROOT="$DEFAULT_DATA_ROOT"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --quiet) QUIET=1 ;;
        --no-hash) CHECK_HASH=0 ;;
        -h|--help)
            sed -n '2,14p' "$0"
            exit 0
            ;;
        *) DATA_ROOT="$1" ;;
    esac
    shift
done

if [[ -n "${PCGEN_TESTDATA_DIR:-}" && "$DATA_ROOT" == "$DEFAULT_DATA_ROOT" ]]; then
    DATA_ROOT="$PCGEN_TESTDATA_DIR"
fi

log() { printf '%s\n' "$*" >&2; }
say() { if [[ "$QUIET" -eq 0 ]]; then printf '%s\n' "$*"; fi; }

command -v python3 >/dev/null 2>&1 || { log "ERROR: python3 is required"; exit 1; }
[[ -f "$LOCK" ]] || { log "ERROR: lock file missing: $LOCK"; exit 1; }

if [[ ! -d "$DATA_ROOT" ]]; then
    log "MISSING test-data root: $DATA_ROOT"
    exit 1
fi

# Dump the lock as tab-separated lines that are trivial to parse in bash:
#   F <sha1> <size> <path>
#   D <pattern> <min> <dir>
lock_lines="$(python3 - "$LOCK" <<'PY'
import json, sys
lock = json.load(open(sys.argv[1]))
for item in lock["required_files"]:
    print("F\t{}\t{}\t{}".format(item["sha1"], item.get("size", 0), item["path"]))
for path, spec in lock.get("minimum_counts", {}).items():
    print("D\t{}\t{}\t{}".format(spec["pattern"], spec["min"], path))
PY
)"

missing=0
while IFS=$'\t' read -r kind a b c; do
    case "$kind" in
        F)
            sha1="$a"; size="$b"; path="$c"
            file="$DATA_ROOT/$path"
            if [[ ! -f "$file" ]]; then
                say "MISSING $path"
                missing=1
                continue
            fi
            if [[ "$CHECK_HASH" -eq 1 ]]; then
                if command -v git >/dev/null 2>&1; then
                    actual="$(git -C "$DATA_ROOT" hash-object "$file")"
                else
                    actual="$(python3 -c 'import hashlib,sys;print(hashlib.sha1(open(sys.argv[1],"rb").read()).hexdigest())' "$file")"
                fi
                if [[ "$actual" != "$sha1" ]]; then
                    say "HASH    $path: $actual != $sha1 (pinned data mismatch)"
                    missing=1
                    continue
                fi
                say "OK      $path"
            else
                say "OK      $path (hash check skipped)"
            fi
            ;;
        D)
            pattern="$a"; min="$b"; dir="$c"
            full="$DATA_ROOT/$dir"
            if [[ ! -d "$full" ]]; then
                say "MISSING $dir/"
                missing=1
                continue
            fi
            count="$(find "$full" -type f -name "$pattern" | wc -l | tr -d ' ')"
            if [[ "$count" -lt "$min" ]]; then
                say "MISSING $dir: only $count files matching '$pattern' (need >= $min)"
                missing=1
            else
                say "OK      $dir/ ($count files matching '$pattern')"
            fi
            ;;
    esac
done <<< "$lock_lines"

marker="$DATA_ROOT/.pcgen-rust-testdata"
if [[ -f "$marker" ]]; then
    say "marker  $marker"
else
    say "note    no .pcgen-rust-testdata marker (data was not fetched by scripts/fetch_testdata.sh)"
fi

if [[ "$missing" -ne 0 ]]; then
    log "test data incomplete or mismatched: $DATA_ROOT"
    exit 1
fi

say "test data complete: $DATA_ROOT"
