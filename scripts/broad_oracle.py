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

Cohort paths are relative to the data root (``PCGEN_TESTDATA_DIR`` or
``<workspace>/raw/pcgen-master``); ``java_root`` is provenance only.  A field
the Java sheet exposes must be either in ``java_gated_fields`` or in an owned
``known_gaps`` entry, and new gaps are refused by default.

Usage:
    scripts/broad_oracle.py --check-paths           # resolve cohort paths
    scripts/broad_oracle.py --write-baseline        # regenerate the baseline
    scripts/broad_oracle.py --write-baseline --accept-known-gaps
    scripts/broad_oracle.py --check                 # gate (default)
    scripts/broad_oracle.py --check --json          # structured gate summary
    scripts/broad_oracle.py --report                # all-field / gated report
    scripts/broad_oracle.py --report --json         # structured report
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import differential_java_pcgen as curated  # type: ignore  # noqa: E402

COHORT_PATH = ROOT / "tests/fixtures/differential/broad_cohort.json"
BASELINE_PATH = ROOT / "tests/fixtures/differential/broad_baseline.json"

# The data revision is pinned by scripts/testdata.lock.json; fetch_testdata.sh
# installs it here by default.  PCGEN_TESTDATA_DIR overrides the location, so a
# checkout or CI job can relocate the read-only snapshot without editing the
# cohort.  Cohort paths are stored relative to this root; `java_root` is kept
# only as provenance for the machine that generated the baseline.
DEFAULT_DATA_ROOT = ROOT.parent / "raw" / "pcgen-master"


def data_root() -> Path:
    override = os.environ.get("PCGEN_TESTDATA_DIR")
    if override:
        return Path(override).expanduser()
    return DEFAULT_DATA_ROOT


def resolve_path(root: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else root / path

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


def choice_state(summary: dict) -> dict:
    """Stable Rust-only choice / ability-pool state for the baseline.

    Java sheets do not expose these fields directly, so they are frozen as a
    generated-only regression surface while the Java-gated numeric fields stay
    unchanged.
    """
    choices = {
        normalize_key(key): value
        for key, value in sorted(summary.get("choices", {}).items())
    }
    unresolved = sorted(set(summary.get("unresolved_choices", [])))
    repeatable = sorted(set(summary.get("repeatable_abilities", [])))
    ability_pools = {
        normalize_key(key): int(value)
        for key, value in sorted(summary.get("ability_pools", {}).items())
    }
    ability_lists = {
        normalize_key(key): sorted(set(values))
        for key, values in sorted(summary.get("ability_lists", {}).items())
    }
    return {
        "choices": choices,
        "choice_bindings": {
            normalize_key(key): sorted(set(values))
            for key, values in sorted(summary.get("choice_bindings", {}).items())
        },
        "unresolved_choices": unresolved,
        "ability_pools": ability_pools,
        "ability_pool_usage": {
            normalize_key(key): {
                "pool": int(value.get("pool", 0)),
                "spent": int(value.get("spent", 0)),
                "remaining": int(value.get("remaining", 0)),
                "selection_counts": dict(
                    sorted((normalize_key(k), int(v)) for k, v in value.get("selection_counts", {}).items())
                ),
            }
            for key, value in sorted(summary.get("ability_pool_usage", {}).items())
        },
        "ability_lists": ability_lists,
        "repeatable_abilities": repeatable,
        "domain_spells": {
            normalize_key(key): sorted(spell.get("name", "") for spell in spells)
            for key, spells in sorted(summary.get("domain_spells", {}).items())
        },
        "deity_domains": sorted(summary.get("deity_domains", [])),
    }


def normalize_key(value: str) -> str:
    return value.trim().lower().replace(" ", "_") if hasattr(value, "trim") else str(value).strip().lower().replace(" ", "_")


def load_cohort() -> dict:
    return json.loads(COHORT_PATH.read_text(encoding="utf-8"))


import htm_csheet  # type: ignore  # noqa: E402


ALL_FIELDS = FIELDS + [f"save_{save}" for save in SAVES]


def resolve_case(case: dict) -> tuple[Path, Path | None, Path]:
    """Return (pcg, pcc, java_sheet) resolved under the active data root.

    The Java sheet is XML by default and may be an HTM csheet when the case
    declares ``java_sheet: "htm"``.  Cohort paths are stored relative to the
    data root; ``java_root`` is provenance only and deliberately not used for
    resolution, so the same cohort works on another checkout or in CI.
    """
    base = data_root()
    pcg = resolve_path(base, case["pcg"])
    sheet = resolve_path(base, case.get("htm") or case.get("xml"))
    pcc = resolve_path(base, case["pcc"]) if case.get("pcc") else None
    return pcg, pcc, sheet


def evaluate_case(case: dict) -> dict:
    """Return {'status': ..., 'values': ...} for one case."""
    pcg, pcc, xml = resolve_case(case)
    result: dict = {"status": "unknown", "values": {}}

    if not pcg.exists() or not xml.exists() or (pcc is not None and not pcc.exists()):
        result["status"] = "fixture_missing"
        return result

    try:
        if case.get("java_sheet") == "htm":
            java = htm_java_baseline(xml)
        else:
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
    result["values"]["rust"]["choice_state"] = choice_state(summary)
    return result


def htm_java_baseline(path: Path) -> dict:
    """Adapt an HTM csheet to the baseline shape used by XML sheets.

    Fields the HTML sheets do not expose (final stats, touch/flat-footed AC,
    classes) stay `None` and are excluded from `java_gated_fields`.
    """
    parsed = htm_csheet.parse_htm_csheet(path)
    return {
        "name": None,
        "race": None,
        "base_stats": {},
        "final_stats": {},
        "classes": [],
        "bab": parsed.get("bab"),
        "hit_points": parsed.get("hit_points"),
        "initiative": parsed.get("initiative"),
        "armor_class": parsed.get("armor_class"),
        "touch_ac": parsed.get("touch_ac"),
        "flat_footed_ac": parsed.get("flat_footed_ac"),
        "saves": {
            "fortitude": parsed.get("save_fortitude"),
            "reflex": parsed.get("save_reflex"),
            "will": parsed.get("save_will"),
        },
        "skills": {},
        "spell_dcs": {},
        "prepared_spells": {},
        "spell_caster_levels": set(),
    }


def build_baseline(cohort: dict) -> dict:
    cases = {}
    for stem, case in cohort["cases"].items():
        result = evaluate_case(case)
        entry: dict = {"status": result["status"]}
        if result["status"] == "computed":
            entry["values"] = {
                key: value
                for key, value in result["values"]["rust"].items()
                if key != "choice_state"
            }
            entry["choice_state"] = result["values"]["rust"]["choice_state"]
            entry["java_agreement"] = comparable_agreement(result)
            entry["known_gaps"] = normalise_known_gaps(case.get("known_gaps"))
        elif result["status"] == "base_only":
            entry["values"] = result["values"]["base"]
        elif result["status"] == "data_missing":
            entry["data_missing"] = result["data_missing"]
        else:
            entry["error"] = result.get("error", "")
        cases[stem] = entry
    return {
        "schema_version": 3,
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
        expected_choice = expected.get("choice_state", {})
        actual_choice = current.get("choice_state", {})
        if actual_choice != expected_choice:
            failures.append(
                f"{stem}: choice_state changed; run --write-baseline after review"
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


def summarise(cohort: dict) -> dict:
    """Field agreement over the fixed computed cohort.

    Returns three distinct tables so missing Java data is never reported as a
    disagreement:

    * ``all_field``: Rust-vs-Java over fields the Java sheet exposes;
    * ``gated``: subset declared in each case's ``java_gated_fields``;
    * ``java_missing``: fields the sheet does not expose (counted separately).
    """
    all_field: dict[str, dict[str, int]] = {}
    gated: dict[str, dict[str, int]] = {}
    java_missing: dict[str, int] = {}
    known_gaps: dict[str, list[dict[str, str]]] = {}
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
        agreement = comparable_agreement(result)
        for field, ok in agreement.items():
            entry = all_field.setdefault(field, {"ok": 0, "total": 0})
            entry["total"] += 1
            entry["ok"] += 1 if ok else 0
        for field in ALL_FIELDS:
            if field not in agreement:
                java_missing[field] = java_missing.get(field, 0) + 1
        for field in case.get("java_gated_fields", []):
            if field not in agreement:
                errors.append(
                    f"{stem}: gated field {field!r} is not exposed by the Java sheet"
                )
                continue
            entry = gated.setdefault(field, {"ok": 0, "total": 0})
            entry["total"] += 1
            entry["ok"] += 1 if agreement[field] else 0
        gap_list = normalise_known_gaps(case.get("known_gaps"))
        if gap_list:
            known_gaps[stem] = gap_list
    return {
        "all_field": all_field,
        "gated": gated,
        "java_missing": java_missing,
        "known_gaps": known_gaps,
        "known_diverging": diverging,
        "errors": errors,
    }


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
            "known_gaps": 0,
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

        expected_case = baseline.get("cases", {}).get(stem, {})
        current_gaps = normalise_known_gaps(case.get("known_gaps"))
        baseline_gaps = normalise_known_gaps(expected_case.get("known_gaps"))
        if current_gaps != baseline_gaps:
            failures.append(
                f"{stem}: known_gaps changed; regenerate with "
                "--write-baseline --accept-known-gaps after review"
            )
        payload["actual"]["known_gaps"] += len(current_gaps)
        failures.extend(validate_case_contract(stem, case, result))

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
    """Aggregate per-field Java agreement across a baseline's cases.

    Only fields that were actually comparable are counted; fields the Java
    sheet does not expose are absent from `java_agreement`.
    """
    totals: dict[str, list[int]] = {}
    for case in baseline.get("cases", {}).values():
        for field, ok in (case.get("java_agreement") or {}).items():
            entry = totals.setdefault(field, [0, 0])
            entry[1] += 1
            if ok:
                entry[0] += 1
    return {field: (ok, total) for field, (ok, total) in totals.items()}


def comparable_agreement(result: dict) -> dict[str, bool]:
    """Agreement only over fields the Java sheet actually exposes.

    HTM csheets do not carry final stats or touch/flat-footed AC, so those
    fields are skipped instead of counted as disagreements.
    """
    rust = result["values"].get("rust", {})
    java = result["values"].get("java", {})
    agreement = {}
    for field in FIELDS:
        want = java.get(field)
        if want in (None, {}, []):
            continue
        agreement[field] = curated.normalise_value(field, rust.get(field)) == curated.normalise_value(
            field, want
        )
    for save in SAVES:
        want = java.get("saves", {}).get(save)
        if want is None:
            continue
        agreement[f"save_{save}"] = rust.get("saves", {}).get(save) == want
    return agreement


def normalise_known_gaps(raw: object) -> list[dict[str, str]]:
    """Return a canonical list of owned known-gap objects.

    Each entry must be an object with non-empty ``field`` / ``owner`` /
    ``issue``.  Malformed entries are returned with an empty field so the
    contract validator reports them instead of silently dropping them.
    """
    if raw is None:
        return []
    if not isinstance(raw, list):
        return [{"field": "", "owner": "", "issue": "known_gaps must be a list"}]
    gaps: list[dict[str, str]] = []
    for entry in raw:
        if not isinstance(entry, dict):
            gaps.append(
                {"field": "", "owner": "", "issue": "known_gaps entries must be objects"}
            )
            continue
        gaps.append(
            {
                "field": str(entry.get("field", "")).strip(),
                "owner": str(entry.get("owner", "")).strip(),
                "issue": str(entry.get("issue", "")).strip(),
            }
        )
    return sorted(gaps, key=lambda gap: gap["field"])


def validate_case_contract(stem: str, case: dict, result: dict) -> list[str]:
    """Validate java_gated_fields / known_gaps against the Java sheet.

    Contract:

    * both lists use unique fields and are disjoint;
    * every known gap is an object with non-empty field / owner / issue;
    * for a computed case, their union exactly matches the fields the Java
      sheet exposes (``comparable_agreement``), so a field cannot quietly
      disappear from the gate.
    """
    failures: list[str] = []
    gated_raw = case.get("java_gated_fields", [])
    if not isinstance(gated_raw, list) or any(
        not isinstance(field, str) for field in gated_raw
    ):
        failures.append(f"{stem}: java_gated_fields must be a list of strings")
        gated_raw = []
    gated = [field.strip() for field in gated_raw if field.strip()]
    if len(gated) != len(set(gated)):
        failures.append(f"{stem}: java_gated_fields contains duplicates")

    gap_objects = normalise_known_gaps(case.get("known_gaps"))
    for gap in gap_objects:
        if not gap["field"] or not gap["owner"] or not gap["issue"]:
            failures.append(
                f"{stem}: known_gaps entries need non-empty field / owner / issue: {gap!r}"
            )
    gap_fields = [gap["field"] for gap in gap_objects if gap["field"]]
    if len(gap_fields) != len(set(gap_fields)):
        failures.append(f"{stem}: known_gaps contains duplicate fields")

    overlap = set(gated) & set(gap_fields)
    if overlap:
        failures.append(f"{stem}: fields both gated and known-gap: {sorted(overlap)}")

    if result.get("status") == "computed":
        exposed = set(comparable_agreement(result))
        classified = set(gated) | set(gap_fields)
        uncovered = exposed - classified
        extra = classified - exposed
        if uncovered:
            failures.append(
                f"{stem}: Java-exposed fields missing from "
                f"java_gated_fields/known_gaps: {sorted(uncovered)}"
            )
        if extra:
            failures.append(
                f"{stem}: gated/known fields not exposed by the Java sheet: "
                f"{sorted(extra)}"
            )
    return failures


def added_known_gap_fields(old: dict, new: dict) -> list[str]:
    """Return known-gap fields present in ``new`` but not in ``old``."""
    additions: list[str] = []
    old_cases = old.get("cases", {})
    for stem, case in new.get("cases", {}).items():
        old_gaps = {
            gap.get("field")
            for gap in normalise_known_gaps(old_cases.get(stem, {}).get("known_gaps"))
        }
        new_gaps = {
            gap.get("field") for gap in normalise_known_gaps(case.get("known_gaps"))
        }
        for field in sorted(new_gaps - old_gaps):
            additions.append(f"{stem}: {field}")
    return additions


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
    mode.add_argument(
        "--check-paths",
        action="store_true",
        help="resolve every cohort path under the active data root and exit",
    )
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
    parser.add_argument(
        "--accept-known-gaps",
        action="store_true",
        help=(
            "allow --write-baseline to record newly-added known_gaps. "
            "Without this flag new gaps are refused even if Java agreement "
            "itself did not regress."
        ),
    )
    args = parser.parse_args(argv)

    cohort = load_cohort()
    base = data_root()
    if not args.json:
        print(f"cohort: {cohort['name']} ({len(cohort['cases'])} cases)")
        print(f"data_ref: {cohort['data_ref']}")
        print(f"data_root: {base}")

    if not base.exists():
        print(
            f"data root not found: {base}; run scripts/fetch_testdata.sh "
            "or set PCGEN_TESTDATA_DIR",
            file=sys.stderr,
        )
        return 2

    if args.check_paths:
        failures: list[str] = []
        for stem, case in cohort["cases"].items():
            pcg, pcc, sheet = resolve_case(case)
            for label, path in (("pcg", pcg), ("sheet", sheet), ("pcc", pcc)):
                if path is not None and not path.exists():
                    failures.append(f"{stem}: {label} missing: {path}")
        payload = {
            "mode": "check-paths",
            "ok": not failures,
            "data_root": str(base),
            "cases": len(cohort["cases"]),
            "failures": failures,
        }
        if args.json:
            _print_json(payload)
        elif failures:
            print(f"broad oracle path check FAILED: {len(failures)} missing path(s)")
            for failure in failures:
                print(f"  {failure}")
        else:
            print(
                f"broad oracle path check passed: {len(cohort['cases'])} cases "
                f"under {base}"
            )
        return 0 if payload["ok"] else 1

    if args.write_baseline:
        baseline = build_baseline(cohort)
        old_baseline = None
        if BASELINE_PATH.exists():
            old_baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
        if old_baseline is not None:
            new_gaps = added_known_gap_fields(old_baseline, baseline)
            if not args.accept_diff:
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
            if new_gaps and not args.accept_known_gaps:
                print("refusing to write baseline: new known_gaps added")
                for addition in new_gaps:
                    print(f"  {addition}")
                print(
                    "re-run with --accept-known-gaps only if these gaps have "
                    "an owner and a reviewable issue."
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
        summary = summarise(cohort)
        errors = summary["errors"]
        known_diverging = summary["known_diverging"]
        all_field = {}
        for field in sorted(summary["all_field"]):
            entry = summary["all_field"][field]
            all_field[field] = {
                "ok": entry["ok"],
                "total": entry["total"],
                "missing": summary["java_missing"].get(field, 0),
                "agreement": (entry["ok"] / entry["total"]) if entry["total"] else 0.0,
            }
        gated = {
            field: {
                "ok": entry["ok"],
                "total": entry["total"],
                "agreement": (entry["ok"] / entry["total"]) if entry["total"] else 0.0,
            }
            for field, entry in sorted(summary["gated"].items())
        }
        payload = {
            "mode": "report",
            "ok": not errors,
            "cohort": cohort["name"],
            "data_ref": cohort["data_ref"],
            "data_root": str(base),
            "all_field": all_field,
            "gated": gated,
            "java_missing": dict(sorted(summary["java_missing"].items())),
            "known_gaps": summary["known_gaps"],
            "known_diverging": known_diverging,
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
            print()
            print("all-field agreement (Java-exposed fields only)")
            print(f"{'field':16} {'ok':>6} {'total':>6} {'missing':>8} {'agreement':>10}")
            for field, entry in all_field.items():
                print(
                    f"{field:16} {entry['ok']:>6} {entry['total']:>6} "
                    f"{entry['missing']:>8} {100.0 * entry['agreement']:9.1f}%"
                )
            print()
            print("java-gated field agreement")
            print(f"{'field':16} {'ok':>6} {'total':>6} {'agreement':>10}")
            for field, entry in gated.items():
                print(
                    f"{field:16} {entry['ok']:>6} {entry['total']:>6} "
                    f"{100.0 * entry['agreement']:9.1f}%"
                )
            if summary["java_missing"]:
                print()
                print("java_missing (excluded from all-field denominator)")
                for field, count in sorted(summary["java_missing"].items()):
                    print(f"  {field:16} {count}")
            if summary["known_gaps"]:
                print()
                print("owned known_gaps (excluded from java-gated gate)")
                for stem, gaps in summary["known_gaps"].items():
                    for gap in gaps:
                        print(
                            f"  {stem}: {gap['field']} owner={gap['owner']} "
                            f"issue={gap['issue']}"
                        )
            if known_diverging:
                print(
                    "known-diverging cases (excluded from denominator): "
                    f"{', '.join(known_diverging)}"
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
            f"{actual['java_gated']} case-gated, {actual['known_gaps']} owned known gaps)"
        )
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
