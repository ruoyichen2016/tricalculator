#!/usr/bin/env python3
"""Aggregate deep-field gate for selected primary cases.

This is a small, separate oracle next to ``broad_oracle.py``.  It compares
Java csheet details that the flat 10-field broad oracle does not cover:

* equipment AC components: ``armor_bonus`` / ``shield_bonus``
* aggregate skill total over keys present in both sides
* max spell DC over spells the Java sheet exposes
* max caster level
* prepared spell count
* attack totals as visibility fields (Rust snapshot does not expose them yet)

Every comparable field must be either Java-gated or an owned ``known_gaps``
entry; new gaps are refused unless ``--accept-known-gaps`` is explicit.

Usage:
    python3 scripts/deep_field_oracle.py --write-baseline
    python3 scripts/deep_field_oracle.py --check
    python3 scripts/deep_field_oracle.py --report
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

CONFIG_PATH = ROOT / "tests/fixtures/differential/deep_field_cohort.json"
BASELINE_PATH = ROOT / "tests/fixtures/differential/deep_field_baseline.json"
DEFAULT_DATA_ROOT = ROOT.parent / "raw" / "pcgen-master"

COMPARABLE_FIELDS = [
    "equipment_ac_bonus",
    "skill_total",
    "spell_dc_max",
    "caster_level_max",
    "prepared_count",
    "melee_attack",
    "ranged_attack",
    "grapple_attack",
]
VISIBILITY_FIELDS: list[str] = []


def data_root() -> Path:
    override = os.environ.get("PCGEN_TESTDATA_DIR")
    return Path(override).expanduser() if override else DEFAULT_DATA_ROOT


def resolve(base: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else base / path


def spell_name(key: str) -> str:
    return key.split("|", 1)[0].strip()


def prepared_spell_name(key: str) -> str:
    return key.rsplit("|", 1)[-1].strip()


def evaluate_case(case: dict) -> dict:
    base = data_root()
    source = case["source"]
    pcc = resolve(base, source["pcc"])
    pcg = resolve(base, source["pcg"])
    xml = resolve(base, source["xml"])
    java = curated.java_baseline(xml)
    rust = curated.rust_summary(pcg, pcc)

    fields: dict[str, dict] = {}

    def add(field: str, rust_value, java_value, note: str = "") -> None:
        fields[field] = {
            "rust": rust_value,
            "java": java_value,
            "agreement": (
                rust_value is not None
                and java_value is not None
                and rust_value == java_value
            ),
            "note": note,
        }

    # PCGen stores armor+shield total in one XML field for some cases
    # (e.g. 3e_CleElf), so compare the combined component.
    add(
        "equipment_ac_bonus",
        (rust.get("armor_bonus") or 0) + (rust.get("shield_bonus") or 0),
        (java.get("armor_bonus") or 0) + (java.get("shield_bonus") or 0),
    )

    rust_skills = rust.get("skills", {}) or {}
    java_skills = java.get("skills", {}) or {}
    java_ranks = java.get("skill_ranks", {}) or {}
    trained = sorted(name for name, rank in java_ranks.items() if rank > 0)
    missing_trained = [name for name in trained if name not in rust_skills]
    if missing_trained:
        add(
            "skill_total",
            None,
            sum(java_skills[name] for name in trained if name in java_skills),
            f"trained skills missing in Rust: {missing_trained[:6]}",
        )
    else:
        add(
            "skill_total",
            sum(rust_skills[name] for name in trained),
            sum(java_skills[name] for name in trained),
            f"trained skill keys={len(trained)}",
        )

    java_dcs = java.get("spell_dcs", {}) or {}
    rust_dcs: dict[str, int] = {}
    for key, value in (rust.get("spell_dcs") or {}).items():
        rust_dcs.setdefault(spell_name(key), value)
    common_dc = sorted(set(java_dcs) & set(rust_dcs))
    if common_dc:
        add(
            "spell_dc_max",
            max(rust_dcs[name] for name in common_dc),
            max(java_dcs[name] for name in common_dc),
            f"common spell DC keys={len(common_dc)}",
        )
    else:
        add("spell_dc_max", None, None, "no common spell DC key")

    rust_caster = sorted(set((rust.get("caster_levels") or {}).values()))
    java_caster = sorted(java.get("spell_caster_levels") or set())
    if rust_caster and java_caster:
        add(
            "caster_level_max",
            max(rust_caster) + int(rust.get("caster_level_bonus") or 0),
            max(java_caster),
            f"domain caster bonus={rust.get('caster_level_bonus') or 0}",
        )
    else:
        add("caster_level_max", None, None, "not comparable on both sides")

    java_prepared = java.get("prepared_spells", {}) or {}
    add(
        "prepared_count",
        int(rust.get("explicit_prepared_count") or 0),
        sum(java_prepared.values()),
        f"rust explicit={rust.get('explicit_prepared_count')} java spells={len(java_prepared)}",
    )

    add("melee_attack", rust.get("melee_attack"), java.get("melee_attack"))
    add("ranged_attack", rust.get("ranged_attack"), java.get("ranged_attack"))
    add("grapple_attack", rust.get("grapple_attack"), java.get("grapple_attack"))

    return {
        "status": "computed",
        "fields": fields,
        "known_gaps": case.get("known_gaps", []),
    }


def validate_case(stem: str, case: dict, result: dict) -> list[str]:
    failures: list[str] = []
    gated = list(case.get("gated", []))
    gaps = list(case.get("known_gaps", []))
    gap_fields = [gap.get("field", "") for gap in gaps]

    if len(gated) != len(set(gated)):
        failures.append(f"{stem}: gated contains duplicates")
    for gap in gaps:
        if not gap.get("field") or not gap.get("owner") or not gap.get("issue"):
            failures.append(f"{stem}: known_gaps need field / owner / issue: {gap!r}")
    if len(gap_fields) != len(set(gap_fields)):
        failures.append(f"{stem}: known_gaps contains duplicate fields")
    overlap = set(gated) & set(gap_fields)
    if overlap:
        failures.append(f"{stem}: fields both gated and known-gap: {sorted(overlap)}")

    comparable = {
        field
        for field in COMPARABLE_FIELDS
        if result["fields"][field]["java"] is not None
    }
    classified = set(gated) | set(gap_fields)
    missing = comparable - classified
    extra = classified - comparable
    if missing:
        failures.append(f"{stem}: comparable fields not classified: {sorted(missing)}")
    if extra:
        failures.append(f"{stem}: classified fields not comparable: {sorted(extra)}")

    for field in gated:
        info = result["fields"].get(field)
        if info is None or not info["agreement"]:
            failures.append(
                f"{stem}: JAVA-GATED {field} rust={info and info['rust']!r} "
                f"java={info and info['java']!r}"
            )
    for field in gap_fields:
        info = result["fields"].get(field)
        if info is not None and info["agreement"]:
            failures.append(f"{stem}: stale known_gap {field} now matches Java; move to gated")
    return failures


def build_baseline(config: dict) -> dict:
    cases = {}
    for stem, case in config["cases"].items():
        result = evaluate_case(case)
        cases[stem] = {
            "fields": result["fields"],
            "known_gaps": case.get("known_gaps", []),
        }
    return {
        "schema_version": 1,
        "config": config["name"],
        "data_ref": config["data_ref"],
        "generated_by": "scripts/deep_field_oracle.py --write-baseline",
        "cases": cases,
    }


def known_gap_fields(entry: dict) -> set[str]:
    return {gap.get("field", "") for gap in (entry.get("known_gaps") or [])}


def added_known_gap_fields(old: dict, new: dict) -> list[str]:
    additions: list[str] = []
    for stem, entry in new.get("cases", {}).items():
        old_fields = known_gap_fields(old.get("cases", {}).get(stem, {}))
        for field in sorted(known_gap_fields(entry) - old_fields):
            additions.append(f"{stem}: {field}")
    return additions


def run_check(config: dict, baseline: dict) -> tuple[dict, list[str]]:
    failures: list[str] = []
    actual = {"cases": 0, "gated": 0, "known_gaps": 0}
    if baseline.get("data_ref") != config.get("data_ref"):
        return {"ok": False, "failures": ["data_ref mismatch; regenerate baseline"]}, [
            "data_ref mismatch; regenerate baseline"
        ]
    for stem, case in config["cases"].items():
        result = evaluate_case(case)
        actual["cases"] += 1
        actual["gated"] += len(case.get("gated", []))
        actual["known_gaps"] += len(case.get("known_gaps", []))
        failures.extend(validate_case(stem, case, result))
        expected = baseline.get("cases", {}).get(stem)
        if expected is None:
            failures.append(f"{stem}: missing from baseline")
            continue
        if known_gap_fields(case) != known_gap_fields(expected):
            failures.append(
                f"{stem}: known_gaps changed; regenerate with --write-baseline "
                "--accept-known-gaps after review"
            )
        if result["fields"] != expected.get("fields"):
            failures.append(f"{stem}: deep-field observation changed; run --report")
    payload = {
        "ok": not failures,
        "mode": "check",
        "config": config["name"],
        "actual": actual,
        "failures": failures,
    }
    return payload, failures


def report(config: dict) -> dict:
    cases = {}
    for stem, case in config["cases"].items():
        result = evaluate_case(case)
        cases[stem] = {
            "gated": case.get("gated", []),
            "known_gaps": case.get("known_gaps", []),
            "fields": result["fields"],
            "errors": validate_case(stem, case, result),
        }
    return cases


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Deep-field aggregate oracle.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write-baseline", action="store_true")
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--report", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--accept-known-gaps",
        action="store_true",
        help="allow --write-baseline to record newly-added known gaps",
    )
    args = parser.parse_args(argv)

    config = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    base = data_root()
    if not args.json:
        print(f"deep-field config: {config['name']} ({len(config['cases'])} cases)")
        print(f"data_root: {base}")
    if not base.exists():
        print(f"data root not found: {base}", file=sys.stderr)
        return 2

    if args.write_baseline:
        baseline = build_baseline(config)
        old = None
        if BASELINE_PATH.exists():
            old = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
        if old is not None:
            additions = added_known_gap_fields(old, baseline)
            if additions and not args.accept_known_gaps:
                print("refusing to write baseline: new deep-field known_gaps")
                for addition in additions:
                    print(f"  {addition}")
                return 1
        BASELINE_PATH.write_text(
            json.dumps(baseline, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {BASELINE_PATH} ({len(baseline['cases'])} cases)")
        return 0

    if args.report:
        cases = report(config)
        if args.json:
            print(json.dumps(cases, indent=2, sort_keys=True, ensure_ascii=False))
        else:
            for stem, entry in cases.items():
                print(f"== {stem}")
                for field in COMPARABLE_FIELDS:
                    info = entry["fields"][field]
                    state = "GATED" if field in entry["gated"] else (
                        "GAP" if field in known_gap_fields(entry) else "vis"
                    )
                    print(
                        f"  {field:16} rust={info['rust']!r:>6} "
                        f"java={info['java']!r:>6} {state}"
                    )
                for error in entry["errors"]:
                    print(f"  ERROR {error}")
        return 0

    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    payload, failures = run_check(config, baseline)
    if args.json:
        print(json.dumps(payload, indent=2, sort_keys=True, ensure_ascii=False))
    elif failures:
        print(f"deep-field oracle FAILED: {len(failures)} issue(s)")
        for failure in failures[:50]:
            print(f"  {failure}")
    else:
        print(
            f"deep-field oracle passed: {payload['actual']['cases']} cases, "
            f"{payload['actual']['known_gaps']} owned known gaps"
        )
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
