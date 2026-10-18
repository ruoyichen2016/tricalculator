#!/usr/bin/env bash
# Install the rolling executable and discard disposable compiler output.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --offline -p pcgen-web
mkdir -p dist/bin
install -m755 target/debug/pcgen-web dist/bin/.pcgen-web.new
mv -f dist/bin/.pcgen-web.new dist/bin/pcgen-web
cargo clean --profile dev
printf '%s\n' 'Installed dist/bin/pcgen-web; compiler caches removed.'
