#!/usr/bin/env bash
# Launch pcgen-web with a data pack and open the browser.
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$DIR/.." && pwd)"

BIN="${PCGEN_WEB_BIN:-}"
if [ -z "$BIN" ]; then
    if [ -x "$DIR/pcgen-web" ]; then
        BIN="$DIR/pcgen-web"
    else
        BIN="$ROOT/target/release/pcgen-web"
    fi
fi

DATA="${PCGEN_DATA_ROOT:-}"
if [ -z "$DATA" ]; then
    for candidate in \
        "$DIR/data/packs/custom/curated/bahamut-all-35e3e" \
        "$ROOT/data/packs/custom/curated/bahamut-all-35e3e" \
        "$DIR/data/packs/rsrd35" \
        "$ROOT/data/packs/rsrd35" \
        "$ROOT/../raw/pcgen-master"; do
        if [ -d "$candidate" ]; then
            DATA="$candidate"
            break
        fi
    done
fi

CHARACTERS="${PCGEN_CHARACTERS_ROOT:-$ROOT/characters}"
URL="http://127.0.0.1:8787"

if [ -z "$DATA" ] || [ ! -d "$DATA" ]; then
    echo "data pack not found; set PCGEN_DATA_ROOT or run scripts/package_datapack.py" >&2
    exit 1
fi
if [ ! -x "$BIN" ]; then
    echo "pcgen-web binary not found at $BIN" >&2
    echo "build it with: cargo build --release -p pcgen-web" >&2
    exit 1
fi

echo "data root:    $DATA"
echo "characters:   $CHARACTERS"
echo "starting:     $BIN"
"$BIN" --data-root "$DATA" --cohort-root "$ROOT" --characters-root "$CHARACTERS" &
PID=$!
trap 'kill "$PID" 2>/dev/null || true' EXIT INT TERM

sleep 2
if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$URL" >/dev/null 2>&1 || true
elif command -v open >/dev/null 2>&1; then
    open "$URL" >/dev/null 2>&1 || true
fi

echo "open: $URL"
wait "$PID"
