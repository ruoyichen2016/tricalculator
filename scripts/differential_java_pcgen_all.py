#!/usr/bin/env python3
"""Broad Java PCGen differential report.

Unlike ``differential_java_pcgen.py`` (the curated regression gate), this
script scans every matching PCG/XML pair it can map to a generic data set,
reports field-level agreement, and exits 0 by default.  It is intended as a
visibility tool for coverage work, not as a release gate.
"""

from __future__ import annotations

import json
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JAVA_ROOT = ROOT.parent / "raw" / "pcgen-master"
PCG_DIR = JAVA_ROOT / "code" / "testsuite" / "PCGfiles"
CSHEET_DIR = JAVA_ROOT / "code" / "testsuite" / "csheets"

PCC_BY_PREFIX = {
    "3e": JAVA_ROOT / "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
    "35e": JAVA_ROOT / "data/35e/wizards_of_the_coast/rsrd/_revised_system_reference_document.pcc",
}

PTS = JAVA_ROOT / "data/zen_test/pcgen_test_advanced/pcgen_test_advanced.pcc"

PCC_BY_STEM = {
    "35e_Alice": PTS,
    "35e_Bob": PTS,
    "35e_Charlie": PTS,
    "35e_CloudGiantHalfDragon": PTS,
    "35e_Dave": PTS,
    "35e_Fran": PTS,
    "35e_Gordon": PTS,
    "35e_Harold": PTS,
    "35e_Jango": PTS,
    "35e_JimDop": PTS,
    "35e_Lauren": PTS,
}



def load_module():
    sys.path.insert(0, str(ROOT / "scripts"))
    import differential_java_pcgen as curated  # type: ignore

    return curated


def main() -> int:
    curated = load_module()
    totals: Counter[str] = Counter()
    mismatches: Counter[str] = Counter()
    cases = 0
    data_missing = 0

    for xml_path in sorted(CSHEET_DIR.glob("*.xml")):
        stem = xml_path.stem
        prefix = stem.split("_", 1)[0]
        pcc = PCC_BY_STEM.get(stem, PCC_BY_PREFIX.get(prefix))
        pcg = PCG_DIR / f"{stem}.pcg"
        if not pcc or not pcc.exists() or not pcg.exists():
            continue
        try:
            java = curated.java_baseline(xml_path)
            rust_base = curated.rust_base_state(curated.rust_character(pcg))
        except Exception:
            continue
        cases += 1

        for key in ("name", "race", "base_stats", "classes"):
            totals[key] += 1
            if curated.normalise_value(key, rust_base.get(key)) != curated.normalise_value(
                key, java.get(key)
            ):
                mismatches[key] += 1

        try:
            summary = curated.rust_summary(pcg, pcc)
        except Exception:
            data_missing += 1
            continue
        missing_data = [
            diagnostic
            for diagnostic in summary.get("diagnostics", [])
            if any(
                marker in diagnostic
                for marker in (
                    "unknown class",
                    "missing progression",
                    "unknown race",
                )
            )
        ]
        if missing_data:
            data_missing += 1
            continue
        rules = {
            "final_stats": summary["stats"],
            "bab": summary["base_attack_bonus"],
            "hit_points": summary["hit_points"],
            "initiative": summary["initiative"],
            "armor_class": summary["armor_class"],
            "touch_ac": summary["touch_ac"],
            "flat_footed_ac": summary["flat_footed_ac"],
            "position": None,
        }
        for key in (
            "final_stats",
            "bab",
            "hit_points",
            "initiative",
            "armor_class",
            "touch_ac",
            "flat_footed_ac",
        ):
            totals[key] += 1
            if curated.normalise_value(key, rules.get(key)) != curated.normalise_value(
                key, java.get(key)
            ):
                mismatches[key] += 1
        rust_saves = {key.lower(): value for key, value in summary["saves"].items()}
        for save in ("fortitude", "reflex", "will"):
            totals[f"save_{save}"] += 1
            if rust_saves.get(save) != java["saves"].get(save):
                mismatches[f"save_{save}"] += 1

    print(f"broad differential cases: {cases}")
    print(f"data-missing/skipped cases: {data_missing}")
    print(f"{'field':16} {'ok':>6} {'bad':>6} {'agreement':>10}")
    for key in totals:
        total = totals[key]
        bad = mismatches[key]
        agreement = 100.0 * (total - bad) / total if total else 0.0
        print(f"{key:16} {total - bad:6d} {bad:6d} {agreement:9.1f}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
