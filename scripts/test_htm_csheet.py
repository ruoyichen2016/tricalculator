#!/usr/bin/env python3
"""Unit checks for the HTM csheet extractor (run via pytest or directly)."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import htm_csheet  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
CSHEETS = ROOT.parent / "raw" / "pcgen-master" / "code" / "testsuite" / "csheets"

CASES = {
    "35e_L7-3(fighter-cleric-dwarf-std).htm": {
        "hit_points": 103,
        "armor_class": 21,
        "initiative": 3,
        "bab": 9,
        "save_fortitude": 12,
        "save_reflex": 6,
        "save_will": 9,
    },
    "35e_L7-3(fighter-cleric-dwarf-cmb).htm": {
        "hit_points": 103,
        "armor_class": 21,
        "initiative": 3,
        "bab": 9,
        "save_fortitude": 12,
        "save_reflex": 6,
        "save_will": 9,
        "touch_ac": 13,
        "flat_footed_ac": 18,
    },
    "35e_L10(fighter-human-std).htm": {
        "hit_points": 114,
        "armor_class": 28,
        "initiative": 4,
        "bab": 10,
        "save_fortitude": 12,
        "save_reflex": 7,
        "save_will": 6,
    },
    "35e_L10(fighter-human-cmb).htm": {
        "hit_points": 114,
        "armor_class": 28,
        "initiative": 4,
        "bab": 10,
        "save_fortitude": 12,
        "save_reflex": 7,
        "save_will": 6,
        "touch_ac": 11,
        "flat_footed_ac": 27,
    },
}


def main() -> int:
    failures = 0
    for name, expected in CASES.items():
        path = CSHEETS / name
        if not path.exists():
            print(f"SKIP {name}: fixture missing")
            continue
        parsed = htm_csheet.parse_htm_csheet(path)
        for field, want in expected.items():
            got = parsed.get(field)
            if got != want:
                failures += 1
                print(f"FAIL {name}::{field} got={got} want={want}")
    if failures:
        print(f"{failures} htm csheet mismatch(es)")
        return 1
    print("all htm csheet variants parsed as expected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
