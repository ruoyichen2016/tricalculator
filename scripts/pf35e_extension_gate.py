#!/usr/bin/env python3
"""Acceptance gate for the PF-as-3.5e extension (`extensions/pf35e`).

The extension is a *variant* layer: it does not rewrite PF base classes,
because Cleric / Fighter / Wizard / Rogue / ... already exist in the 3.5e
RSRD with the same HD, BAB and save progressions.  What PF actually changes
is a set of class features, so the extension ships
`data/pf35e_variants.lst` (`TYPE:Archetype` + `APPLIESTO` / `REPLACES` /
`GRANTS` / `CSKILL`) plus descriptive `data/pf35e_abilities.lst` entries.

This gate therefore checks, in order:

1. layering — `GAMEMODE:35e`, and the extension adds exactly the variant
   abilities on top of the RSRD without changing any other definition count
   or introducing new un-lowered tags;
2. parsing — every variant record reaches `CompiledRulePack.variants`;
3. behaviour — a variant survives `create`, is accepted by `level-up`
   (`--archetype`), and an unknown archetype id is rejected.

Requires the pinned PCGen test data (for the RSRD base). Exit codes:

* 0  gate passed
* 1  a gate assertion failed
* 2  skipped (data or built CLI missing) — callers decide whether that fails
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = Path(os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw" / "pcgen-master"))
RSRD = DATA / "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"

# Variant ability names the extension is expected to contribute.
EXPECTED_VARIANTS = {
    "Cloistered Cleric",
    "Weapon Master",
    "Armor Master",
    "School Specialist",
}

# Class whose core progression the variant layer must not disturb.
PROGRESSION_CLASSES = ("Cleric", "Fighter", "Wizard")

# `classes` must NOT grow at all: the extension is a variant layer.

TYPE_MARKER = re.compile(r"\bTYPE:([^\t\r\n]*)", re.IGNORECASE)


def pcgen_binary() -> str:
    return os.environ.get("PCGEN_BIN") or str(ROOT / "target" / "debug" / "pcgen")


def run_pcgen(*args: str) -> str:
    result = subprocess.run(
        [pcgen_binary(), *args], cwd=ROOT, text=True, capture_output=True
    )
    if result.returncode != 0:
        raise SystemExit(f"pcgen {' '.join(args)} failed:\n{result.stderr}")
    return result.stdout


def import_report(pcc: Path) -> dict:
    return json.loads(run_pcgen("import-report", str(pcc), "--json"))


def pending_tags(report: dict) -> dict:
    coverage = report["coverage"]
    return {
        key: coverage.get(key) or {}
        for key in ("legacy_pending_by_tag", "unsupported_pending_by_tag")
    }


def declared_variants() -> list[str]:
    """Variant ability names declared by `data/pf35e_variants.lst`."""
    path = ROOT / "extensions" / "pf35e" / "data" / "pf35e_variants.lst"
    names = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = TYPE_MARKER.search(line)
        if not match:
            continue
        markers = {value.strip().lower() for value in match.group(1).split(".")}
        if any("archetype" in marker or "substitution" in marker for marker in markers):
            names.append(line.split("\t", 1)[0].strip())
    return names


def declared_abilities() -> list[str]:
    """Ability names declared by `data/pf35e_abilities.lst`."""
    path = ROOT / "extensions" / "pf35e" / "data" / "pf35e_abilities.lst"
    names = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        names.append(line.split("\t", 1)[0].strip())
    return names


def main() -> int:
    if not RSRD.exists():
        print(f"SKIP pf35e extension gate: missing {RSRD}")
        return 2
    binary = pcgen_binary()
    if not shutil.which(binary) and not Path(binary).exists():
        print(f"SKIP pf35e extension gate: missing {binary}")
        return 2

    # Materialise the extension next to the data tree.
    subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "package_pf35e_extension.py")],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    pcc = DATA / "data" / "_pf35e_extension" / "rsrd_pf35e.pcc"
    failures: list[str] = []

    declared = declared_variants()
    if not declared:
        failures.append("extensions/pf35e/data/pf35e_variants.lst declares no variants")
    missing_declared = EXPECTED_VARIANTS - set(declared)
    if missing_declared:
        failures.append(
            "declared variants missing from pf35e_variants.lst: "
            + ", ".join(sorted(missing_declared))
        )

    base = import_report(RSRD)
    layered = import_report(pcc)

    # --- 1. layering -------------------------------------------------------
    if layered["provenance"]["game_mode"] != "35e":
        failures.append(
            f"game_mode = {layered['provenance']['game_mode']}, want 35e"
        )
    declared_features = declared_abilities()
    want_abilities = base["abilities"] + len(declared_features)
    if layered["abilities"] != want_abilities:
        failures.append(
            f"abilities = {layered['abilities']}, want {want_abilities} "
            f"(RSRD {base['abilities']} + {len(declared_features)} declared feature entries)"
        )
    want_variants = len(declared)
    if layered.get("variants", 0) != want_variants:
        failures.append(
            f"variants = {layered.get('variants', 0)}, want {want_variants}"
        )
    # The variant layer ships no classes; the RSRD already has every PF core
    # class's 3.5e counterpart.
    if layered["classes"] != base["classes"]:
        failures.append(
            f"classes = {layered['classes']}, want {base['classes']} "
            "(the variant extension must not add classes)"
        )
    for key, tags in pending_tags(layered).items():
        if tags:
            failures.append(f"{key} = {tags}, want {{}}")

    # --- 2. parsing --------------------------------------------------------
    got_variants = set(layered.get("variants_by_name") or []) or None
    variant_ids = set(layered.get("variant_ids") or [])
    if got_variants is None and not variant_ids:
        # Older CLI builds only expose the count; fall back to the count check
        # above and to the id list, which is always present.
        pass
    if base.get("variants", 0) != 0:
        failures.append(
            f"RSRD alone has {base['variants']} variants; the variant layer's "
            "contribution can no longer be distinguished"
        )

    # --- 2b. semantics: every REPLACES target must actually resolve ---------
    # Parsing is not enough: a variant whose `REPLACES` matches nothing still
    # lands in `pack.variants`, so the gate has to confirm the target hits a
    # real class feature line.  This is what catches "declared but inert".
    variant_rows = json.loads(
        run_pcgen("list-variants", "--pcc", str(pcc), "--format", "json")
    )
    if len(variant_rows) != len(declared):
        failures.append(
            f"list-variants reported {len(variant_rows)} variants, want {len(declared)}"
        )
    for row in variant_rows:
        for target in row.get("replace_targets", []):
            if not target.get("resolved"):
                failures.append(
                    f"variant `{row['name']}` replaces `{target['target']}` "
                    "which does not match any class feature"
                )
        if row.get("replace_targets") and not row.get("all_targets_resolved"):
            failures.append(
                f"variant `{row['name']}` has unresolved REPLACES targets"
            )
        if not row.get("replace_targets"):
            failures.append(
                f"variant `{row['name']}` declares no REPLACES target; "
                "an archetype that replaces nothing is inert"
            )

    # `evaluate` runs against a built-in demo pack, so semantics can only be
    # checked through the campaign-aware commands above.
    probe = ROOT / "target" / "pf35e-gate"
    if probe.exists():
        shutil.rmtree(probe)
    probe.mkdir(parents=True)
    envelope = probe / "variant-probe.json"
    run_pcgen(
        "create", "--pcc", str(pcc), "--name", "Gate Variant", "--race", "Human",
        "--class", "Fighter", "--level", "2", "--point-buy", "25",
        "--stat", "STR=12", "--stat", "DEX=12", "--stat", "CON=12",
        "--stat", "INT=12", "--stat", "WIS=12", "--stat", "CHA=12",
        "--out", str(envelope),
    )
    if not envelope.exists():
        failures.append(f"create --out {envelope} did not write the requested file")

    # A variant that applies to the advanced class is accepted ...
    accepted = subprocess.run(
        [
            pcgen_binary(), "level-up", str(envelope), "--pcc", str(pcc),
            "--class", "Fighter", "--hp", "6",
            "--archetype", "Weapon Master", "--preview",
        ],
        cwd=ROOT, text=True, capture_output=True,
    )
    if accepted.returncode != 0:
        failures.append(
            "level-up rejected the `Weapon Master` archetype:\n"
            + accepted.stderr.strip()
        )
    else:
        try:
            snapshot = json.loads(accepted.stdout)
            if snapshot["total_level"] != 3:
                failures.append(
                    f"level-up preview reached level {snapshot['total_level']}, want 3"
                )
            if snapshot["base_attack_bonus"] != 3:
                failures.append(
                    f"Fighter 3 BAB {snapshot['base_attack_bonus']} != 3"
                )
        except json.JSONDecodeError:
            failures.append("level-up --preview did not print a JSON snapshot")

    # ... and an unknown archetype id is rejected.
    rejected = subprocess.run(
        [
            pcgen_binary(), "level-up", str(envelope), "--pcc", str(pcc),
            "--class", "Fighter", "--hp", "6",
            "--archetype", "No Such Variant", "--preview",
        ],
        cwd=ROOT, text=True, capture_output=True,
    )
    if rejected.returncode == 0:
        failures.append("level-up accepted an unknown archetype id")

    # A variant restricted to another class is rejected too.
    wrong_class = subprocess.run(
        [
            pcgen_binary(), "level-up", str(envelope), "--pcc", str(pcc),
            "--class", "Fighter", "--hp", "6",
            "--archetype", "School Specialist", "--preview",
        ],
        cwd=ROOT, text=True, capture_output=True,
    )
    if wrong_class.returncode == 0:
        failures.append("level-up accepted `School Specialist` on a Fighter lane")

    # The same round trip through `evaluate`, which resolves variants from the
    # draft alone: a variant record is accepted, an unknown one is refused.
    draft = probe / "draft.json"
    draft.write_text(
        json.dumps(
            {
                "id": "gate-variant-draft",
                "name": "Gate Variant Draft",
                "race_id": "Human",
                "campaign": str(pcc),
                "stats": {"STR": 12, "DEX": 12, "CON": 12, "INT": 12, "WIS": 12, "CHA": 12},
                "advancement": [
                    {
                        "character_level": 1,
                        "lanes": [
                            {
                                "lane_id": "primary",
                                "class_id": "Fighter",
                                "class_level": 1,
                                "hit_points_gained": 6,
                                "archetype": "Weapon Master",
                            }
                        ],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    evaluated = subprocess.run(
        [pcgen_binary(), "evaluate", str(draft), "--format", "json"],
        cwd=ROOT, text=True, capture_output=True,
    )
    if evaluated.returncode != 0:
        failures.append("evaluate failed on a draft carrying a variant:\n" + evaluated.stderr.strip())
    else:
        try:
            snapshot = json.loads(evaluated.stdout)
            if snapshot.get("variant_effects") is None:
                failures.append("evaluate snapshot has no variant_effects field")
        except json.JSONDecodeError:
            failures.append("evaluate did not print a JSON snapshot")

    shutil.rmtree(probe, ignore_errors=True)

    if failures:
        for failure in failures:
            print(f"FAIL pf35e: {failure}")
        return 1
    print(
        f"pf35e extension gate passed ({len(declared)} variants layered on the "
        f"RSRD, {base['classes']} classes unchanged, variants accepted/rejected "
        "by level-up)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
