#!/usr/bin/env python3
"""Differential baseline: Rust PCGen rules vs Java PCGen XML sheets.

The Java PCGen test suite stores character-sheet XML under
``raw/pcgen-master/code/testsuite/csheets``.  This script compares:

* base-state fields read directly from the ``.pcg`` file
* computed fields produced by ``pcgen recalculate``

Currently compared:

* name, race, base ability scores, class levels
* final ability scores
* hit points
* BAB
* AC
* Fortitude / Reflex / Will saves
* skill modifiers

Spell DC has no direct Java XML field in these sheets, so it is covered by
the Rust unit tests rather than this XML baseline.
"""

from __future__ import annotations

import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from pcgen_cli import run_pcgen  # type: ignore  # noqa: E402

JAVA_ROOT = ROOT.parent / "raw" / "pcgen-master"
PCG_DIR = JAVA_ROOT / "code" / "testsuite" / "PCGfiles"
CSHEET_DIR = JAVA_ROOT / "code" / "testsuite" / "csheets"

CASES = [
    {
        "pcg": "3e_FigFae.pcg",
        "xml": "3e_FigFae.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
    },
    {
        "pcg": "35e_Bard.pcg",
        "xml": "35e_Bard.xml",
        "pcc": "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
    },
    {
        "pcg": "3e_SWizSam.pcg",
        "xml": "3e_SWizSam.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "rules_keys": [
            "name",
            "race",
            "final_stats",
            "bab",
            "hit_points",
            "initiative",
            "armor_class",
            "touch_ac",
            "flat_footed_ac",
        ],
        "compare_skills": False,
        "compare_spells": True,
        "base_keys": ["name", "race", "base_stats"],
    },
    {
        "pcg": "3e_BrdJoe.pcg",
        "xml": "3e_BrdJoe.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "compare_skills": True,
        "compare_spells": True,
    },
    {
        "pcg": "3e_WizShar.pcg",
        "xml": "3e_WizShar.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "compare_skills": False,
        "compare_spells": True,
    },
    {
        "pcg": "3e_CleElf.pcg",
        "xml": "3e_CleElf.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "rules_keys": [
            "name",
            "race",
            "final_stats",
            "bab",
            "hit_points",
            "initiative",
            "armor_class",
            "touch_ac",
            "flat_footed_ac",
            "saves",
        ],
        "compare_skills": False,
        "compare_spells": True,
    },
    {
        "pcg": "3e_BarJack.pcg",
        "xml": "3e_BarJack.xml",
        "pcc": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "rules_keys": [
            "name",
            "race",
            "final_stats",
            "bab",
            "hit_points",
            "initiative",
            "armor_class",
            "touch_ac",
            "saves",
        ],
        "compare_skills": False,
        "compare_spells": False,
    },
    {
        "pcg": "35e_Quasvin.pcg",
        "xml": "35e_Quasvin.xml",
        "pcc": "data/35e/wizards_of_the_coast/rsrd/_revised_system_reference_document.pcc",
        "base_keys": ["name", "race", "base_stats"],
        "rules_keys": [
            "name",
            "race",
            "final_stats",
            "bab",
            "hit_points",
            "initiative",
            "armor_class",
            "armor_bonus",
            "shield_bonus",
            "touch_ac",
            "flat_footed_ac",
            "saves",
        ],
        "compare_skills": False,
        "compare_spells": False,
        "no_validate": True,
    },
    {
        "pcg": "35e_Ivan.pcg",
        "xml": "35e_Ivan.xml",
        "pcc": None,
    },
]


def run_rust(args: list[str]) -> dict:
    return run_pcgen(args, parse_json=True)


def rust_character(pcg: Path) -> dict:
    return run_rust(["parse-pcg", str(pcg), "--format", "json"])


def rust_summary(pcg: Path, pcc: Path, no_validate: bool = True) -> dict:
    args = [
        "recalculate",
        "--pcc",
        str(pcc),
        str(pcg),
        "--summary",
        "--format",
        "json",
    ]
    return run_rust(args)


def java_baseline(xml_path: Path) -> dict:
    root = ET.parse(xml_path).getroot()

    base_abilities = {}
    final_abilities = {}
    for ability in root.findall("./abilities/ability"):
        short = ability.findtext("./name/short")
        base = ability.findtext("base")
        score = ability.findtext("score")
        if short and base is not None:
            base_abilities[short] = int(base)
        if short and score is not None:
            final_abilities[short] = int(score)

    classes = []
    for class_element in root.findall("./basics/classes/class"):
        name = class_element.findtext("name")
        level = class_element.findtext("level")
        if name and level:
            classes.append((name, int(level)))

    saves = {}
    for saving_throw in root.findall("./saving_throws/saving_throw"):
        name = saving_throw.findtext("./name/long")
        total = saving_throw.findtext("total")
        if name and total is not None:
            saves[name.lower()] = int(total)

    skills = {}
    skill_ranks = {}
    for skill in root.findall("./skills/skill"):
        name = skill.findtext("name")
        modifier = skill.findtext("skill_mod")
        ranks = skill.findtext("ranks")
        if name and modifier is not None:
            skills[name] = int(modifier)
        if name and ranks is not None:
            try:
                skill_ranks[name] = float(ranks)
            except ValueError:
                pass

    spell_dcs = {}
    spell_caster_levels = set()
    for spell in root.findall(".//spell"):
        name = spell.findtext("name")
        if not name:
            continue
        dc = spell.findtext("dc")
        if dc:
            spell_dcs[name] = int(dc)
        caster_level = spell.findtext("casterlevel")
        if caster_level:
            spell_caster_levels.add(int(caster_level))

    # `prepared_spells` must mean memorized/prepared spells, not the full
    # known-spell list.  Racial innate memorized spells are tracked separately
    # and are not part of the character's prepared spell list.
    prepared_spells = {}
    memorized = root.find("./spells/memorized_spells")
    if memorized is not None:
        racial_innate = set(memorized.findall("./racial_innate_memorized//spell"))
        for spell in memorized.findall(".//spell"):
            if spell in racial_innate:
                continue
            name = spell.findtext("name")
            if not name:
                continue
            times = spell.findtext("times_memorized")
            count = int(times) if times else 1
            prepared_spells[name] = prepared_spells.get(name, 0) + count

    def attack_total(section: str) -> int | None:
        value = root.findtext(f"./attack/{section}/total")
        if value is None:
            return None
        # Iterative attacks are shown as "+7/+2"; the first total is the
        # primary attack bonus used by this aggregate oracle.
        match = re.search(r"[-+]?\d+", value)
        return int(match.group(0)) if match else None

    return {
        "name": root.findtext("./basics/name"),
        "race": root.findtext("./basics/race"),
        "base_stats": base_abilities,
        "final_stats": final_abilities,
        "classes": classes,
        "bab": int(root.findtext("./basics/bab", "0").replace("+", "")),
        "hit_points": int(root.findtext("./hit_points/points", "0")),
        "initiative": int(root.findtext("./initiative/total", "0").replace("+", "")),
        "armor_class": int(root.findtext("./armor_class/total", "0")),
        "armor_bonus": int(root.findtext("./armor_class/armor_bonus", "0")),
        "shield_bonus": int(root.findtext("./armor_class/shield_bonus", "0")),
        "touch_ac": int(root.findtext("./armor_class/touch", "0")),
        "flat_footed_ac": int(root.findtext("./armor_class/flat", "0")),
        "melee_attack": attack_total("melee"),
        "ranged_attack": attack_total("ranged"),
        "grapple_attack": attack_total("grapple"),
        "saves": saves,
        "skills": skills,
        "skill_ranks": skill_ranks,
        "spell_dcs": spell_dcs,
        "prepared_spells": prepared_spells,
        "spell_caster_levels": spell_caster_levels,
    }


def rust_base_state(character: dict) -> dict:
    stats = {
        stat: character.get("stats", {}).get(stat)
        for stat in ("STR", "DEX", "CON", "INT", "WIS", "CHA")
    }
    classes = [
        (entry.get("class_id"), int(entry.get("level", 0)))
        for entry in character.get("class_levels", [])
    ]
    return {
        "name": character.get("name"),
        "race": character.get("race_id"),
        "base_stats": stats,
        "classes": classes,
    }


def normalise_value(key: str, value):
    if key == "race" and isinstance(value, str):
        # Java sheets sometimes suffix races with campaign/book qualifiers,
        # e.g. "Elan (Psionic)".
        return value.split(" (", 1)[0].strip()
    return value


def compare(label: str, rust: dict, java: dict, keys: list[str]) -> int:
    failures = 0
    for key in keys:
        if normalise_value(key, rust.get(key)) == normalise_value(key, java.get(key)):
            continue
        failures += 1
        print(f"FAIL {label}::{key}")
        print("  rust:", rust.get(key))
        print("  java:", java.get(key))
    return failures


def main() -> int:
    failures = 0
    for case in CASES:
        pcg_path = PCG_DIR / case["pcg"]
        xml_path = CSHEET_DIR / case["xml"]
        if not pcg_path.exists() or not xml_path.exists():
            print(f"SKIP {case['pcg']}: fixture not found")
            continue

        java = java_baseline(xml_path)
        rust_base = rust_base_state(rust_character(pcg_path))
        failures += compare(
            f"{case['pcg']} base",
            rust_base,
            java,
            case.get("base_keys", ["name", "race", "base_stats", "classes"]),
        )

        if case["pcc"] is None:
            print(f"PARTIAL {case['pcg']} (rules baseline unavailable)")
            continue

        pcc_path = JAVA_ROOT / case["pcc"]
        if not pcc_path.exists():
            print(f"SKIP {case['pcg']} rules: {pcc_path} not found")
            continue

        summary = rust_summary(pcg_path, pcc_path, case.get("no_validate", True))
        rules = {
            "name": summary["name"],
            "race": summary["race_id"],
            "final_stats": summary["stats"],
            "bab": summary["base_attack_bonus"],
            "hit_points": summary["hit_points"],
            "initiative": summary["initiative"],
            "armor_class": summary["armor_class"],
            "armor_bonus": summary["armor_bonus"],
            "shield_bonus": summary["shield_bonus"],
            "touch_ac": summary["touch_ac"],
            "flat_footed_ac": summary["flat_footed_ac"],
            "saves": {key.lower(): value for key, value in summary["saves"].items()},
            "skills": summary["skills"],
        }
        compare_keys = case.get(
            "rules_keys",
            [
                "name",
                "race",
                "final_stats",
                "bab",
                "hit_points",
                "initiative",
                "armor_class",
                "touch_ac",
                "flat_footed_ac",
                "saves",
            ],
        )
        rule_failures = compare(f"{case['pcg']} rules", rules, java, compare_keys)

        # Skills: compare the intersection.  PCGen XML includes every
        # skill; the Rust summary only includes skills recorded on the
        # character, which is exactly the set we can compare here.
        if case.get("compare_skills", True):
            java_skills = java["skills"]
            rust_skills = rules["skills"]
            skill_intersection = sorted(set(java_skills) & set(rust_skills))
            mismatched_skills = {
                name: (rust_skills[name], java_skills[name])
                for name in skill_intersection
                if rust_skills[name] != java_skills[name]
            }
            if mismatched_skills:
                rule_failures += len(mismatched_skills)
                print(f"FAIL {case['pcg']}::skills")
                for name, (rust_value, java_value) in mismatched_skills.items():
                    print(f"  {name}: rust={rust_value} java={java_value}")
            else:
                print(f"OK   {case['pcg']} skills ({len(skill_intersection)} checked)")

        if case.get("compare_spells", False):
            rust_spell_dcs = {}
            for key, dc in summary["spell_dcs"].items():
                name = key.split("|", 1)[0]
                rust_spell_dcs[name] = dc
            java_spell_dcs = java["spell_dcs"]
            intersection = sorted(set(rust_spell_dcs) & set(java_spell_dcs))
            mismatches = {
                name: (rust_spell_dcs[name], java_spell_dcs[name])
                for name in intersection
                if rust_spell_dcs[name] != java_spell_dcs[name]
            }
            if mismatches:
                rule_failures += len(mismatches)
                print(f"FAIL {case['pcg']}::spell_dcs")
                for name, (rust_value, java_value) in mismatches.items():
                    print(f"  {name}: rust={rust_value} java={java_value}")
            else:
                print(f"OK   {case['pcg']} spell DCs ({len(intersection)} checked)")

            rust_prepared = {}
            for key, count in summary["prepared_spells"].items():
                name = key.split("|", 1)[1] if "|" in key else key
                rust_prepared[name] = count
            java_prepared = java["prepared_spells"]
            intersection = sorted(set(rust_prepared) & set(java_prepared))
            prepared_mismatches = {
                name: (rust_prepared[name], java_prepared[name])
                for name in intersection
                if rust_prepared[name] != java_prepared[name]
            }
            if prepared_mismatches:
                rule_failures += len(prepared_mismatches)
                print(f"FAIL {case['pcg']}::prepared_spells")
                for name, (rust_value, java_value) in prepared_mismatches.items():
                    print(f"  {name}: rust={rust_value} java={java_value}")
            else:
                print(f"OK   {case['pcg']} prepared spells ({len(intersection)} checked)")

            rust_caster_levels = set(summary["caster_levels"].values())
            java_caster_levels = java["spell_caster_levels"]
            if rust_caster_levels and not rust_caster_levels.issubset(java_caster_levels):
                rule_failures += 1
                print(f"FAIL {case['pcg']}::caster_levels")
                print("  rust:", sorted(rust_caster_levels))
                print("  java:", sorted(java_caster_levels))
            else:
                print(
                    f"OK   {case['pcg']} caster levels ({len(rust_caster_levels)} checked)"
                )

        failures += rule_failures
        if rule_failures == 0:
            print(f"OK   {case['pcg']} rules")

    if failures:
        print(f"{failures} differential case(s) failed")
        return 1
    print("all differential cases passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
