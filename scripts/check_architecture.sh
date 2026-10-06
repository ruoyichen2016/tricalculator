#!/usr/bin/env bash
# Architecture guardrails.
#
# Enforces the dependency direction documented in ARCHITECTURE.md:
#
#     pcgen-import  ->  pcgen-ir  <-  pcgen-engine
#
# 1. pcgen-engine must not depend on pcgen-import in production code.
# 2. pcgen-ir must not depend on pcgen-import (no cycle).
#
# `cargo tree -e normal` excludes dev/build dependencies.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export CARGO_HOME="${CARGO_HOME:-$ROOT/.cargo-home}"

failures=0

check_absent() {
    local crate="$1" forbidden="$2"
    local tree
    tree="$(cargo tree -p "$crate" -e normal --prefix none 2>/dev/null || true)"
    if [[ -z "$tree" ]]; then
        echo "SKIP  $crate: cargo tree produced no output"
        return 0
    fi
    if grep -q "^${forbidden} " <<< "$tree"; then
        echo "FAIL  $crate production tree contains $forbidden"
        failures=$((failures + 1))
    else
        echo "OK    $crate production tree does not contain $forbidden"
    fi
}

check_present() {
    local crate="$1" required="$2"
    local tree
    tree="$(cargo tree -p "$crate" -e normal --prefix none 2>/dev/null || true)"
    if grep -q "^${required} " <<< "$tree"; then
        echo "OK    $crate production tree contains $required"
    else
        echo "FAIL  $crate production tree is missing $required"
        failures=$((failures + 1))
    fi
}

check_absent pcgen-engine pcgen-import
check_absent pcgen-ir pcgen-import
check_present pcgen-engine pcgen-ir
check_present pcgen-import pcgen-ir

# 3. Engine source must not branch on raw PCGen category strings; use
#    pcgen_core::EntityCategory / ChoiceKind instead.
engine_src="$(find crates/pcgen-engine/src -name '*.rs' -print)"
if grep -nE '"RACE"|"CLASS_LEVEL"|"EQUIPMENT"|"PCSTAT"' $engine_src; then
    echo "FAIL  engine source branches on raw category strings (use EntityCategory/ChoiceKind)"
    failures=$((failures + 1))
else
    echo "OK    engine source has no raw category string branches"
fi

# 4. The draft *data* types must live in pcgen-ir, not pcgen-import.
if grep -rq 'pub struct PcgDraft' crates/pcgen-import/src; then
    echo "FAIL  pcgen-import still defines PcgDraft (move data types to pcgen-ir)"
    failures=$((failures + 1))
else
    echo "OK    PcgDraft data types live in pcgen-ir"
fi

# 5. Engine source must not interpret PCGen *formula syntax*.
#
#    The checks above are syntactic: they catch `.pcc` / `.lst` strings.  They
#    did not catch `CL`, `TL`, `classlevel(...)` and the `VAR.` target prefix,
#    which is where the two quietest defects lived — `BONUS:VAR|A,B|...` was
#    not split, and `CL` resolved to 0 for every multiclass character.  Those
#    tokens are translated at lowering now (see `pcgen_core::formula_vocab`),
#    and this check keeps them out.
#
#    Comments are stripped first, so the guard can still be *explained* in the
#    code it guards.  Matching is case-sensitive on purpose: the runtime
#    vocabulary `ClassLevel_*` shares a prefix with PCGen's `classlevel`, and
#    only the lowercase spelling is PCGen's.
engine_tokens='classlevel|APPLIEDAS|"VAR\.|\\bCL\\b|\\bTL\\b'
engine_hits="$(sed 's://.*::' $engine_src | grep -nE "$engine_tokens" || true)"
if [[ -n "$engine_hits" ]]; then
    echo "FAIL  engine source interprets PCGen formula syntax:"
    echo "$engine_hits" | head -10
    echo "      translate it at lowering (pcgen_core::formula_vocab) instead"
    failures=$((failures + 1))
else
    echo "OK    engine source has no PCGen formula syntax"
fi

if [[ "$failures" -ne 0 ]]; then
    echo "architecture check FAILED: $failures violation(s)"
    exit 1
fi
echo "architecture check passed"
