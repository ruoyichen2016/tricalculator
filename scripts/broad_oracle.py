#!/usr/bin/env python3
"""Broad Java differential oracle: baseline generation and per-case gating.

`scripts/differential_java_pcgen_all.py` is a visibility tool (it always exits
0).  This module turns the same corpus into a real per-case regression gate by:

1. pinning a *cohort* (the exact case list that must be evaluated), and
2. comparing every case against a checked-in baseline of observed field values.

Two baseline modes are supported:

* ``generated`` cases: we compare the Rust engine output against a recorded
  JSON baseline (``tests/fixtures/differential/broad_baseline.json``).  This
  freezes *current* behaviour (including known divergences) so that changes are
  caught, without pretending the Java agreement is 100%.
* ``java_gated`` cases: we additionally require the Rust values to equal the
  Java csheet values for the fields listed for that case.  Only cases that
  actually pass today are listed here, so the gate is honest and tight.

Usage:
    scripts/broad_oracle.py --write-baseline        # regenerate the baseline
    scripts/broad_oracle.py --check                 # gate (default)
    scripts/broad_oracle.py --check --json          # structured gate summary
    scripts/broad_oracle.py --report                # human summary, exit 0
    scripts/broad_oracle.py --report --json         # structured report
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import differential_java_pcgen as curated  # type: ignore  # noqa: E402

COHORT_PATH = ROOT / "tests/fixtures/differential/broad_cohort.json"
BASELINE_PATH = ROOT / "tests/fixtures/differential/broad_baseline.json"

# Fields captured per computed case.  `position` is intentionally omitted:
# the Java sheets do not expose it and it is always None in this corpus.
FIELDS = [
    "final_stats",
    "bab",
    "hit_points",
    "initiative",
    "armor_class",
    "touch_ac",
    "flat_footed_ac",
]
SAVES = ["fortitude", "reflex", "will"]


def load_cohort() -> dict:
    return json.loads(COHORT_PATH.read_text(encoding="utf-8"))


def resolve_case(case: dict) -> tuple[Path, Path | None, Path]:
    java_root = Path(case["java_root"])
    pcg = Path(case["pcg"])
    xml = Path(case["xml"])
    pcc = Path(case["pcc"]) if case.get("pcc") else None
    return pcg, pcc, xml


def evaluate_case(case: dict) -> dict:
    """Return {'status': ..., 'values': ...} for one case."""
    pcg, pcc, xml = resolve_case(case)
    result: dict = {"status": "unknown", "values": {}}

    if not pcg.exists() or not xml.exists() or (pcc is not None and not pcc.exists()):
        result["status"] = "fixture_missing"
        return result

    try:
        java = curated.java_baseline(xml)
        base = curated.rust_base_state(curated.rust_character(pcg))
    except Exception as error:  # noqa: BLE001
        result["status"] = "error"
        result["error"] = f"{type(error).__name__}: {error}"
        return result

    result["values"]["base"] = {
        key: curated.normalise_value(key, base.get(key))
        for key in ("name", "race", "base_stats", "classes")
    }
    result["values"]["java"] = {
        key: curated.normalise_value(key, java.get(key))
        for key in ("name", "race", "base_stats", "classes", "final_stats", "bab",
                    "hit_points", "initiative", "armor_class", "touch_ac",
                    "flat_footed_ac")
    }
    result["values"]["java"]["saves"] = java.get("saves", {})

    if pcc is None:
        result["status"] = "base_only"
        return result

    try:
        summary = curated.rust_summary(pcg, pcc)
    except Exception as error:  # noqa: BLE001
        result["status"] = "error"
        result["error"] = f"{type(error).__name__}: {error}"
        return result

    missing_data = [
        diagnostic
        for diagnostic in summary.get("diagnostics", [])
        if any(
            marker in diagnostic
            for marker in ("unknown class", "missing progression", "unknown race")
        )
    ]
    if missing_data:
        result["status"] = "data_missing"
        result["data_missing"] = missing_data[:5]
        return result

    rules = {
        "final_stats": summary["stats"],
        "bab": summary["base_attack_bonus"],
        "hit_points": summary["hit_points"],
        "initiative": summary["initiative"],
        "armor_class": summary["armor_class"],
        "touch_ac": summary["touch_ac"],
        "flat_footed_ac": summary["flat_footed_ac"],
    }
    saves = {key.lower(): value for key, value in summary["saves"].items()}
    result["status"] = "computed"
    result["values"]["rust"] = {
        key: curated.normalise_value(key, rules.get(key)) for key in FIELDS
    }
    result["values"]["rust"]["saves"] = saves
    return result


def field_agreement(result: dict) -> dict[str, bool]:
    """Per-field Rust-vs-Java agreement for a computed case."""
    rust = result["values"].get("rust", {})
    java = result["values"].get("java", {})
    agreement = {}
    for field in FIELDS:
        agreement[field] = curated.normalise_value(field, rust.get(field)) == curated.normalise_value(
            field, java.get(field)
        )
    for save in SAVES:
        agreement[f"save_{save}"] = rust.get("saves", {}).get(save) == java.get("saves", {}).get(save)
    return agreement


def build_baseline(cohort: dict) -> dict:
    cases = {}
    for stem, case in cohort["cases"].items():
        result = evaluate_case(case)
        entry: dict = {"status": result["status"]}
        if result["status"] == "computed":
            entry["values"] = result["values"]["rust"]
            entry["java_agreement"] = field_agreement(result)
        elif result["status"] == "base_only":
            entry["values"] = result["values"]["base"]
        elif result["status"] == "data_missing":
            entry["data_missing"] = result["data_missing"]
        else:
            entry["error"] = result.get("error", "")
        cases[stem] = entry
    return {
        "schema_version": 1,
        "cohort": cohort["name"],
        "data_ref": cohort["data_ref"],
        "generated_by": "scripts/broad_oracle.py --write-baseline",
        "cases": cases,
    }


def compare_field(rust_value, baseline_value):
    return curated.normalise_value("field", rust_value) == curated.normalise_value(
        "field", baseline_value
    )


def check_case(
    stem: str,
    case: dict,
    baseline: dict,
    java_gate: bool = True,
    result: dict | None = None,
) -> list[str]:
    failures: list[str] = []
    expected = baseline["cases"].get(stem)
    if expected is None:
        return [f"{stem}: missing from baseline (regenerate with --write-baseline)"]

    if result is None:
        result = evaluate_case(case)
    if result["status"] != expected["status"]:
        failures.append(
            f"{stem}: status changed {expected['status']} -> {result['status']}"
        )
        return failures

    if result["status"] == "computed":
        current = result["values"]["rust"]
        for field in FIELDS:
            if not compare_field(current.get(field), expected["values"].get(field)):
                failures.append(
                    f"{stem}: {field} {current.get(field)!r} != baseline "
                    f"{expected['values'].get(field)!r}"
                )
        for save in SAVES:
            if current.get("saves", {}).get(save) != expected["values"].get("saves", {}).get(save):
                failures.append(
                    f"{stem}: save_{save} {current.get('saves', {}).get(save)!r} != "
                    f"baseline {expected['values'].get('saves', {}).get(save)!r}"
                )
    elif result["status"] == "base_only":
        for key, value in result["values"]["base"].items():
            if not compare_field(value, expected["values"].get(key)):
                failures.append(f"{stem}: base.{key} {value!r} != baseline {expected['values'].get(key)!r}")

    # Java-gated cases must match the Java sheet for the listed fields.
    gated = case.get("java_gated_fields") if java_gate else None
    if gated and result["status"] == "computed":
        java = result["values"]["java"]
        rust = result["values"]["rust"]
        for field in gated:
            if field.startswith("save_"):
                actual = rust.get("saves", {}).get(field[5:])
                want = java.get("saves", {}).get(field[5:])
            else:
                actual = rust.get(field)
                want = java.get(field)
            if curated.normalise_value(field, actual) != curated.normalise_value(field, want):
                failures.append(
                    f"{stem}: JAVA-GATED {field} rust={actual!r} java={want!r}"
                )
    return failures


def summarise(cohort: dict) -> tuple[dict[str, int], dict[str, int], list[str], list[str]]:
    """Field agreement over the *fixed* computed cohort.

    Only cases whose declared `expectation` is `computed` contribute to the
    denominator.  A case expected to compute but failing to do so is returned
    as an error instead of silently shrinking the denominator.
    """
    totals: dict[str, int] = {}
    agreements: dict[str, int] = {}
    errors: list[str] = []
    diverging: list[str] = []
    for stem, case in cohort["cases"].items():
        expectation = case.get("expectation", "computed")
        result = evaluate_case(case)
        if expectation in ("data_missing", "base_only"):
            continue
        # Both `computed` and `computed_diverging` must actually compute.
        if result["status"] != "computed":
            errors.append(f"{stem}: expected computed, got {result['status']}")
            continue
        if expectation == "computed_diverging":
            diverging.append(stem)
            continue
        for field, ok in field_agreement(result).items():
            totals[field] = totals.get(field, 0) + 1
            agreements[field] = agreements.get(field, 0) + (1 if ok else 0)
    return totals, agreements, errors, diverging


def _print_json(payload: dict) -> None:
    print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))


def _expected_payload(cohort: dict) -> dict:
    expected = cohort.get("expectation_summary", {})
    cases = cohort.get("cases", {})
    return {
        "cases": len(cases),
        "computed": expected.get("computed", 0),
        "computed_diverging": expected.get("computed_diverging", 0),
        "must_compute": expected.get("must_compute", 0),
        "data_missing": expected.get("data_missing", 0),
        "base_only": expected.get("base_only", 0),
        "data_missing_limit": cohort.get("data_missing_limit", 0),
        "java_gated": sum(1 for case in cases.values() if case.get("java_gated_fields")),
    }


def run_check(cohort: dict) -> tuple[dict, list[str]]:
    """Run the per-case check once and return (structured summary, failures)."""
    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    expected = _expected_payload(cohort)
    payload: dict = {
        "mode": "check",
        "ok": False,
        "cohort": cohort["name"],
        "data_ref": cohort["data_ref"],
        "expected": expected,
        "actual": {
            "computed": 0,
            "java_compared": 0,
            "known_diverging": 0,
            "java_gated": expected["java_gated"],
            "data_missing": 0,
        },
        "failures": [],
    }

    if baseline.get("data_ref") != cohort.get("data_ref"):
        error = (
            f"baseline data_ref {baseline.get('data_ref')!r} != cohort "
            f"{cohort.get('data_ref')!r}; regenerate with --write-baseline"
        )
        payload["failures"] = [error]
        return payload, [error]

    failures: list[str] = []
    data_missing_seen: list[str] = []
    known_diverging_seen: list[str] = []

    for stem, case in cohort["cases"].items():
        expectation = case.get("expectation", "computed")
        result = evaluate_case(case)
        status = result["status"]

        if status == "data_missing":
            data_missing_seen.append(stem)
        if expectation == "computed_diverging":
            if status == "computed":
                payload["actual"]["known_diverging"] += 1
                known_diverging_seen.append(stem)
        elif expectation == "computed" and status == "computed":
            payload["actual"]["java_compared"] += 1
        if expectation in ("computed", "computed_diverging") and status == "computed":
            payload["actual"]["computed"] += 1

        expected_status = (
            "computed" if expectation == "computed_diverging" else expectation
        )
        if expected_status and status != expected_status:
            failures.append(
                f"{stem}: expectation {expectation!r} but actual status {status!r}"
            )
        if expectation == "computed_diverging":
            failures.extend(
                check_case(stem, case, baseline, java_gate=False, result=result)
            )
            continue
        failures.extend(check_case(stem, case, baseline, result=result))

    limit = cohort.get("data_missing_limit")
    if limit is not None and len(data_missing_seen) > limit:
        failures.append(
            f"data_missing grew to {len(data_missing_seen)} (> limit {limit}): "
            f"{', '.join(data_missing_seen)}"
        )

    payload["actual"]["data_missing"] = len(data_missing_seen)
    payload["actual"]["known_diverging_cases"] = known_diverging_seen
    payload["actual"]["data_missing_cases"] = data_missing_seen
    payload["failures"] = failures
    payload["ok"] = not failures
    return payload, failures


def baseline_agreements(baseline: dict) -> dict[str, tuple[int, int]]:
    """Aggregate per-field Java agreement across a baseline's cases."""
    totals: dict[str, list[int]] = {}
    for case in baseline.get("cases", {}).values():
        for field, ok in (case.get("java_agreement") or {}).items():
            entry = totals.setdefault(field, [0, 0])
            entry[1] += 1
            if ok:
                entry[0] += 1
    return {field: (ok, total) for field, (ok, total) in totals.items()}


def baseline_regressions(old: dict, new: dict) -> list[str]:
    """Human-readable list of Java-agreement regressions old -> new."""
    regressions: list[str] = []
    old_status = {
        stem: case.get("status") for stem, case in old.get("cases", {}).items()
    }
    for stem, case in new.get("cases", {}).items():
        before = old_status.get(stem)
        after = case.get("status")
        if before == "computed" and after != "computed":
            regressions.append(f"{stem}: status {before} -> {after}")
    old_agreement = baseline_agreements(old)
    new_agreement = baseline_agreements(new)
    for field in sorted(set(old_agreement) | set(new_agreement)):
        old_ok, old_total = old_agreement.get(field, (0, 0))
        new_ok, new_total = new_agreement.get(field, (0, 0))
        if old_total == 0:
            continue
        if new_total == 0:
            regressions.append(
                f"{field}: java agreement {old_ok}/{old_total} -> "
                f"{new_ok}/{new_total}"
            )
            continue
        # Compare rates so a shrinking case population cannot fake a
        # regression (status changes are reported separately above).
        if new_ok / new_total < old_ok / old_total - 1e-9:
            regressions.append(
                f"{field}: java agreement {old_ok}/{old_total} -> "
                f"{new_ok}/{new_total}"
            )
    return regressions


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Broad differential oracle.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-baseline", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--report", action="store_true")
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit a machine-readable JSON summary on stdout",
    )
    parser.add_argument(
        "--accept-diff",
        action="store_true",
        help=(
            "allow --write-baseline to record a Java-agreement regression. "
            "Without this flag the write is refused so a regression cannot be "
            "frozen by accident."
        ),
    )
    args = parser.parse_args(argv)

    cohort = load_cohort()
    if not args.json:
        print(f"cohort: {cohort['name']} ({len(cohort['cases'])} cases)")
        print(f"data_ref: {cohort['data_ref']}")

    if args.write_baseline:
        baseline = build_baseline(cohort)
        if BASELINE_PATH.exists() and not args.accept_diff:
            old_baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
            regressions = baseline_regressions(old_baseline, baseline)
            if regressions:
                print("refusing to write baseline: Java agreement regressed")
                for regression in regressions:
                    print(f"  {regression}")
                print(
                    "re-run with --accept-diff only if this regression is "
                    "intentional and reviewed."
                )
                return 1
        BASELINE_PATH.write_text(
            json.dumps(baseline, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        computed = sum(
            1 for case in baseline["cases"].values() if case["status"] == "computed"
        )
        payload = {
            "mode": "write-baseline",
            "ok": True,
            "baseline": str(BASELINE_PATH),
            "cases": len(baseline["cases"]),
            "computed": computed,
        }
        if args.json:
            _print_json(payload)
        else:
            print(
                f"wrote {BASELINE_PATH} "
                f"({payload['cases']} cases, {payload['computed']} computed)"
            )
        return 0

    if args.report:
        totals, agreements, errors, diverging = summarise(cohort)
        fields = {}
        for field in sorted(totals):
            total = totals[field]
            fields[field] = {
                "ok": agreements[field],
                "total": total,
                "agreement": (agreements[field] / total) if total else 0.0,
            }
        payload = {
            "mode": "report",
            "ok": not errors,
            "cohort": cohort["name"],
            "data_ref": cohort["data_ref"],
            "fields": fields,
            "known_diverging": diverging,
            "errors": errors,
        }
        if args.json:
            _print_json(payload)
        else:
            expected = cohort.get("expectation_summary", {})
            print(
                f"cohort expectation: computed={expected.get('computed', '?')} "
                f"data_missing={expected.get('data_missing', '?')} "
                f"base_only={expected.get('base_only', '?')} "
                f"(data_missing_limit={cohort.get('data_missing_limit', '?')})"
            )
            print(f"{'field':16} {'ok':>6} {'total':>6} {'agreement':>10}")
            for field in sorted(totals):
                total = totals[field]
                ok = agreements[field]
                print(f"{field:16} {ok:>6} {total:>6} {100.0 * ok / total:9.1f}%")
            if diverging:
                print(
                    "known-diverging cases (excluded from denominator): "
                    f"{', '.join(diverging)}"
                )
            if errors:
                print("cohort errors (expected computed but did not):")
                for error in errors:
                    print(f"  {error}")
        return 1 if errors else 0

    # --check (default)
    payload, failures = run_check(cohort)
    if args.json:
        _print_json(payload)
    elif failures:
        print(f"broad oracle FAILED: {len(failures)} mismatch(es)")
        for failure in failures[:60]:
            print(f"  {failure}")
        if len(failures) > 60:
            print(f"  ... and {len(failures) - 60} more")
    else:
        actual = payload["actual"]
        print(
            f"broad oracle passed: {payload['expected']['cases']} cases "
            f"({actual['computed']} computed = {actual['java_compared']} "
            f"java-compared + {actual['known_diverging']} known-diverging, "
            f"{actual['java_gated']} java-gated)"
        )
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
