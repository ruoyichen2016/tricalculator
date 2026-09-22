#!/usr/bin/env bash
# Fetch the pinned PCGen test-data snapshot used by the real-data tests and
# coverage/differential gates.
#
# The repository, ref, tarball URL, required files, blob hashes and minimum
# counts all come from scripts/testdata.lock.json.  Bumping the data revision
# only requires editing that single file.
#
# The default layout matches what the integration tests expect:
#
#   <workspace>/raw/pcgen-master/
#       data/...
#       code/testsuite/...
#       outputsheets/...
#
# Usage:
#   scripts/fetch_testdata.sh [--dest DIR] [--force] [--dry-run]
#
# Environment overrides:
#   PCGEN_TESTDATA_DIR   destination directory
#   PCGEN_TARBALL_URL    custom codeload/mirror URL (overrides the lock file)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKSPACE="$(cd "$ROOT/.." && pwd)"
DEFAULT_DEST="$WORKSPACE/raw/pcgen-master"
LOCK="$ROOT/scripts/testdata.lock.json"

DEST="${PCGEN_TESTDATA_DIR:-$DEFAULT_DEST}"
FORCE=0
DRY_RUN=0

usage() {
    sed -n '2,22p' "$0"
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dest) DEST="$2"; shift ;;
        --force) FORCE=1 ;;
        --dry-run) DRY_RUN=1 ;;
        -h|--help) usage ;;
        *) echo "unknown argument: $1" >&2; usage ;;
    esac
    shift
done

command -v python3 >/dev/null 2>&1 || { echo "ERROR: python3 is required" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "ERROR: curl is required" >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { echo "ERROR: tar is required" >&2; exit 1; }
[[ -f "$LOCK" ]] || { echo "ERROR: lock file missing: $LOCK" >&2; exit 1; }

# The lock file is the single source of truth.  Read the fields we need.
eval "$(python3 - "$LOCK" <<'PY'
import json, shlex, sys
lock = json.load(open(sys.argv[1]))
def emit(name, value):
    print(f"{name}={shlex.quote(str(value))}")
emit("LOCK_REPO", lock["repo"])
emit("LOCK_REF", lock["ref"])
emit("LOCK_TAG", lock.get("tag", ""))
emit("LOCK_URL", lock["tarball_url"])
emit("LOCK_PATHS", " ".join(lock.get("sparse_paths", [])))
PY
)"

TARBALL_URL="${PCGEN_TARBALL_URL:-$LOCK_URL}"

echo "PCGen test data"
echo "  lock: $LOCK"
echo "  repo: $LOCK_REPO"
echo "  ref:  $LOCK_REF${LOCK_TAG:+ ($LOCK_TAG)}"
echo "  url:  $TARBALL_URL"
echo "  dest: $DEST"

if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "dry-run: nothing fetched"
    exit 0
fi

if [[ -e "$DEST" && "$FORCE" -ne 1 ]]; then
    if "$ROOT/scripts/check_testdata.sh" "$DEST" --quiet 2>/dev/null; then
        echo "test data already present and complete; nothing to do"
        echo "use --force to replace it"
        exit 0
    fi
    echo "ERROR: $DEST exists but is incomplete or from another revision." >&2
    echo "       re-run with --force to replace it." >&2
    exit 1
fi

if [[ -e "$DEST" && "$FORCE" -eq 1 ]]; then
    echo "removing existing $DEST"
    rm -rf -- "$DEST"
fi

mkdir -p "$(dirname "$DEST")"
TMP="$(mktemp -d "$(dirname "$DEST")/.pcgen-testdata.XXXXXX")"
cleanup() { [[ -n "${TMP:-}" && -d "$TMP" ]] && rm -rf -- "$TMP"; }
trap cleanup EXIT

# Build tar wildcard patterns from the lock file's sparse_paths.
# Directory-like paths get "dir/*"; file paths (e.g. gradle.properties) are
# matched exactly, otherwise GNU tar fails with "Not found in archive".
tar_patterns=()
for path in $LOCK_PATHS; do
    case "$path" in
        *.*|*/*.*)
            # Heuristic: contains a dot in the last component -> treat as file
            # unless it is a known directory.  Directory entries end without an
            # extension in this lock file, so this is sufficient.
            tar_patterns+=("*/$path")
            ;;
        *)
            tar_patterns+=("*/$path/*")
            ;;
    esac
done

echo "downloading and extracting (${LOCK_PATHS})..."
curl -fsSL --retry 3 --retry-delay 2 "$TARBALL_URL" \
    | tar xzf - -C "$TMP" --strip-components=1 --no-same-owner --wildcards \
        "${tar_patterns[@]}"

echo "verifying required files and pinned blob hashes..."
"$ROOT/scripts/check_testdata.sh" "$TMP" --quiet 2>&1 | tail -5

cat > "$TMP/.pcgen-rust-testdata" <<JSON
{
  "repo": "$LOCK_REPO",
  "ref": "$LOCK_REF",
  "tag": "$LOCK_TAG",
  "tarball_url": "$TARBALL_URL",
  "fetched_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "lock_file": "scripts/testdata.lock.json"
}
JSON

mv -- "$TMP" "$DEST"
TMP=""

echo "test data ready: $DEST"
du -sh "$DEST" 2>/dev/null || true
