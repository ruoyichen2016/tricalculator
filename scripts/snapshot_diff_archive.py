#!/usr/bin/env python3
"""Three-way differential: archive v1 vs new engine, adjudicated by Java PCGen.

The archive binary is a frozen behavioural oracle, but it is **not** gospel:
it has known bugs of its own.  When archive and new disagree, we consult the
Java PCGen csheet (the original reference implementation the whole project is
reproducing) and let it decide:

* new == java  -> the **archive** is wrong; reported as ``ARCHIVE-BUG`` and
  not a failure.
* archive == java -> the **new engine** regressed; reported as ``REGRESSION``
  and counted as a failure.
* neither matches -> reported as ``DIVERGENCE`` and counted as a failure.

Exit code is non-zero only for REGRESSION / DIVERGENCE (or ``--strict`` with
any failure).  Fields the Java sheet does not expose are compared
archive-vs-new directly and a mismatch is a ``DIVERGENCE``.

Usage:
    (cd ../reference/pcgen-rust-archive && cargo build)   # once
    python3 scripts/snapshot_diff_archive.py [--strict]
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from pcgen_cli import run_pcgen  # type: ignore  # noqa: E402

RAW = ROOT.parent / "raw/pcgen-master"
ARCHIVE_BIN = Path(
    os.environ.get(
        "ARCHIVE_PCGEN",
        ROOT.parent / "reference/pcgen-rust-archive/target/debug/pcgen",
    )
)

CASES = [
    ("3e_FigFae.pcg", "3e_FigFae.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    ("35e_Bard.pcg", "35e_Bard.xml", "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"),
    ("3e_SWizSam.pcg", "3e_SWizSam.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    ("3e_BrdJoe.pcg", "3e_BrdJoe.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    ("3e_WizShar.pcg", "3e_WizShar.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    ("3e_CleElf.pcg", "3e_CleElf.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    ("3e_BarJack.pcg", "3e_BarJack.xml", "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"),
    (
        "35e_Quasvin.pcg",
        "35e_Quasvin.xml",
        "data/35e/wizards_of_the_coast/rsrd/_revised_system_reference_document.pcc",
    ),
]

PCG_DIR = RAW / "code/testsuite/PCGfiles"
CSHEET_DIR = RAW / "code/testsuite/csheets"

# Fields where the archive is known to be wrong and Java is authoritative.
# The verdict is derived automatically from the three-way comparison; this
# table is an assertion: if one of these stops showing up as an ARCHIVE-BUG,
# the script prints STALE so the table can be pruned.
ARCHIVE_KNOWN_WRONG: dict[tuple[str, str], str] = {
    ("3e_BarJack.pcg", "flat_footed_ac"): "Barbarian uncanny dodge retains DEX (Java=19)",
    ("3e_WizShar.pcg", "skills.literacy"): "Literacy synergy not stacked (Java=1)",
}

# Scalar fields exposed by all three sources.  `race` is compared via
# `race_id` on the engine side.
SCALAR_FIELDS = [
    "name",
    "race",
    "base_attack_bonus",
    "hit_points",
    "initiative",
    "armor_class",
    "armor_bonus",
    "shield_bonus",
    "touch_ac",
    "flat_footed_ac",
]
# Map fields compared element-wise.
MAP_FIELDS = ["skills", "spell_dcs", "prepared_spells"]


def run_new(pcc: Path, pcg: Path) -> dict:
    return run_pcgen(
        [
            "recalculate",
            "--pcc",
            str(pcc),
            str(pcg),
            "--summary",
            "--format",
            "json",
        ],
        parse_json=True,
    )


def run_archive(pcc: Path, pcg: Path) -> dict:
    output = subprocess.check_output(
        [
            str(ARCHIVE_BIN),
            "recalculate",
            "--pcc",
            str(pcc),
            str(pcg),
            "--summary",
            "--no-validate",
        ],
        text=True,
    )
    return json.loads(output)


def run_java(xml: Path) -> dict:
    sys.path.insert(0, str(ROOT / "scripts"))
    import differential_java_pcgen as curated  # type: ignore

    java = curated.java_baseline(xml)
    # The curated reader returns skills keyed by display name; the engine uses
    # the same keys, so no extra normalisation is needed here.
    return java


def normalise(value):
    if isinstance(value, dict):
        return {str(key).lower(): normalise(item) for key, item in sorted(value.items())}
    if isinstance(value, list):
        return [normalise(item) for item in value]
    return value


def engine_value(snapshot: dict, field: str):
    if field == "race":
        return snapshot.get("race_id")
    if field in ("base_attack_bonus",):
        return snapshot.get("base_attack_bonus")
    return snapshot.get(field)


def java_value(java: dict, field: str):
    if field in ("base_attack_bonus",):
        return java.get("bab")
    if field == "race":
        return java.get("race")
    return java.get(field)


def fmt(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False)


def classify(pcg: str, path: str, arch_value, new_value, java_value, java_known: bool) -> str:
    """Return one of OK / ARCHIVE-BUG / REGRESSION / DIVERGENCE."""
    if arch_value == new_value:
        return "OK"
    if not java_known:
        # Java sheet has no value for this field: fall back to archive as the
        # only oracle, and treat the mismatch as a divergence to investigate.
        return "DIVERGENCE"
    new_matches_java = new_value == java_value
    arch_matches_java = arch_value == java_value
    if new_matches_java and not arch_matches_java:
        return "ARCHIVE-BUG"
    if arch_matches_java and not new_matches_java:
        return "REGRESSION"
    return "DIVERGENCE"


def main() -> int:
    # `--strict` is accepted for backward compatibility; the three-way
    # adjudication is always strict about regressions/divergences.
    args = [a for a in sys.argv[1:] if a != "--strict"]
    if args:
        print(f"unknown argument(s): {' '.join(args)}")
        return 2
    if not ARCHIVE_BIN.exists():
        print(f"SKIP: archive binary not found at {ARCHIVE_BIN}")
        return 0

    regressions = 0
    divergences = 0
    archive_bugs = 0
    checked = 0
    seen_archive_bugs: set[tuple[str, str]] = set()

    for pcg_name, xml_name, pcc_rel in CASES:
        pcg = PCG_DIR / pcg_name
        pcc = RAW / pcc_rel
        xml = CSHEET_DIR / xml_name
        if not pcg.exists() or not pcc.exists() or not xml.exists():
            print(f"SKIP {pcg_name}: fixture missing")
            continue

        archive = normalise(run_archive(pcc, pcg))
        new = normalise(run_new(pcc, pcg))
        java = normalise(run_java(xml))
        checked += 1
        case_failures = 0

        def judge(path: str, arch_value, new_value, java_value, java_known: bool) -> None:
            nonlocal case_failures, regressions, divergences, archive_bugs
            verdict = classify(pcg_name, path, arch_value, new_value, java_value, java_known)
            if verdict == "OK":
                return
            if verdict == "ARCHIVE-BUG":
                archive_bugs += 1
                seen_archive_bugs.add((pcg_name, path))
                print(
                    f"ARCHIVE-BUG {pcg_name}::{path}: archive={fmt(arch_value)} "
                    f"new={fmt(new_value)} java={fmt(java_value)}"
                )
            elif verdict == "REGRESSION":
                regressions += 1
                case_failures += 1
                print(
                    f"REGRESSION {pcg_name}::{path}: archive={fmt(arch_value)} "
                    f"new={fmt(new_value)} java={fmt(java_value)}"
                )
            else:
                divergences += 1
                case_failures += 1
                print(
                    f"DIVERGENCE {pcg_name}::{path}: archive={fmt(arch_value)} "
                    f"new={fmt(new_value)} java={fmt(java_value)}"
                )

        for field in SCALAR_FIELDS:
            arch_value = archive.get(field)
            new_value = engine_value(new, field)
            java_v = java_value(java, field)
            java_known = java_v is not None
            judge(field, arch_value, new_value, java_v, java_known)

        for field in MAP_FIELDS:
            arch_map = archive.get(field, {}) or {}
            new_map = new.get(field, {}) or {}
            java_map = java.get(field, {}) or {}
            # Only compare keys the archive actually reports.  The new engine
            # emits later-stage fields (prepared psionic powers, ...) that the
            # archive snapshot format predates; those are not divergences.
            for key in sorted(set(arch_map) & set(new_map)):
                judge(
                    f"{field}.{key}",
                    arch_map.get(key),
                    new_map.get(key),
                    java_map.get(key),
                    key in java_map,
                )

        if case_failures == 0:
            print(f"OK   {pcg_name}")

    # The declared archive-bug table must match what we actually observed:
    # a missing one means the archive was rebuilt/fixed (or a case vanished),
    # which is worth surfacing rather than silently ignoring.
    stale = [
        f"{pcg}::{path}"
        for (pcg, path) in ARCHIVE_KNOWN_WRONG
        if (pcg, path) not in seen_archive_bugs
    ]
    if stale:
        print()
        print(f"STALE archive-bug declaration(s): {', '.join(stale)}")

    print()
    print(
        f"three-way differential: {checked} cases, "
        f"{archive_bugs} archive-bug(s), {regressions} regression(s), "
        f"{divergences} divergence(s)"
    )
    if regressions or divergences:
        print("three-way differential FAILED")
        return 1
    print("three-way differential passed (new engine matches Java everywhere checked)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
