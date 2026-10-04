#!/usr/bin/env python3
"""Acceptance gate for the PF-as-3.5e extension (`extensions/pf35e`).

Requires the pinned PCGen test data (for the RSRD base). Exit codes:

* 0  gate passed
* 2  skipped (data or built CLI missing) — callers decide whether that fails
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA = Path(os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw" / "pcgen-master"))
RSRD = DATA / "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"


def run_pcgen(*args: str) -> str:
    binary = os.environ.get("PCGEN_BIN") or str(ROOT / "target" / "debug" / "pcgen")
    result = subprocess.run([binary, *args], cwd=ROOT, text=True, capture_output=True)
    if result.returncode != 0:
        raise SystemExit(f"pcgen {' '.join(args)} failed:\n{result.stderr}")
    return result.stdout


def main() -> int:
    if not RSRD.exists():
        print(f"SKIP pf35e extension gate: missing {RSRD}")
        return 2
    binary = os.environ.get("PCGEN_BIN") or str(ROOT / "target" / "debug" / "pcgen")
    if not shutil.which(binary) and not Path(binary).exists():
        print(f"SKIP pf35e extension gate: missing {binary}")
        return 2

    # Materialise the extension next to the data tree.
    subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "package_pf35e_extension.py")],
        cwd=ROOT,
        check=True,
    )
    pcc = DATA / "data" / "_pf35e_extension" / "rsrd_pf35e.pcc"

    report = json.loads(run_pcgen("import-report", str(pcc), "--json"))
    failures = []
    if report["provenance"]["game_mode"] != "35e":
        failures.append(f"game_mode = {report['provenance']['game_mode']}, want 35e")
    # RSRD alone has 89 classes under this entry; the extension adds 3.
    if report["classes"] < 92:
        failures.append(f"classes = {report['classes']}, want >= 92")

    # PF formulas: Cleric 3 -> BAB +2, F4/R1/W4 base (before ability mods);
    # Fighter 3 -> BAB +3, F4/R2/W2; Wizard 3 -> BAB +1, F2/R2/W4.
    expectations = {
        "Cleric": {"bab": 2, "saves": {"Fortitude": 4, "Reflex": 2, "Will": 4}},
        "Fighter": {"bab": 3, "saves": {"Fortitude": 4, "Reflex": 2, "Will": 2}},
        "Wizard": {"bab": 1, "saves": {"Fortitude": 2, "Reflex": 2, "Will": 4}},
    }
    for cls, want in expectations.items():
        snapshot = json.loads(
            run_pcgen(
                "create", "--pcc", str(pcc), "--name", f"Gate {cls}", "--race", "Human",
                "--class", cls, "--level", "3", "--point-buy", "25",
                "--stat", "STR=12", "--stat", "DEX=12", "--stat", "CON=12",
                "--stat", "INT=12", "--stat", "WIS=12", "--stat", "CHA=12",
                "--format", "json",
            )
        )
        if snapshot["base_attack_bonus"] != want["bab"]:
            failures.append(f"{cls} BAB {snapshot['base_attack_bonus']} != {want['bab']}")
        for save, value in want["saves"].items():
            if snapshot["saves"].get(save) != value:
                failures.append(f"{cls} {save} {snapshot['saves'].get(save)} != {value}")

    if failures:
        for failure in failures:
            print(f"FAIL pf35e: {failure}")
        return 1
    print("pf35e extension gate passed (3 classes, BAB/saves match PF formulas)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
