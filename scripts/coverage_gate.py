#!/usr/bin/env python3
"""Coverage non-regression gate for PCGen-Rust import."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from pcgen_cli import run_pcgen  # type: ignore  # noqa: E402

CARGO_HOME = Path(os.environ.get("CARGO_HOME", ROOT / ".cargo-home"))
DATA_ROOT = Path(
    os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw/pcgen-master")
)

MINI = ROOT / "tests/fixtures/campaign-mini/mini.pcc"
REAL = (
    DATA_ROOT / "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc"
)
FIXTURE_PCG = ROOT / "tests/fixtures/pcg/3e_FigFae.pcg"
BRDJOE_PCG = DATA_ROOT / "code/testsuite/PCGfiles/3e_BrdJoe.pcg"
BARD_PCG = DATA_ROOT / "code/testsuite/PCGfiles/35e_Bard.pcg"
BARD_PCC = (
    DATA_ROOT
    / "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"
)
# DS-3E is the regression proxy; DS-35E is the primary acceptance dataset.
# Both are frozen independently so a change that improves one cannot hide a
# regression in the other.
DS35E = (
    DATA_ROOT
    / "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"
)
# DS-PSI is the psionics acceptance dataset: it exercises POWER-style
# `SPELL:` records with `TYPE:Psionic`, power points and manifester levels.
DSPSI = (
    DATA_ROOT
    / "data/35e/wizards_of_the_coast/rsrd/psionics/_rsrd_psionics.pcc"
)
DATASETS = {"ds-3e": REAL, "ds-35e": DS35E, "ds-psi": DSPSI}
COVERAGE_BASELINE = ROOT / "tests/fixtures/coverage/coverage_baseline.json"
LOCK_PATH = ROOT / "scripts/testdata.lock.json"


def run_report(pcc: Path) -> dict:
    return run_pcgen(
        ["import-report", str(pcc), "--json"],
        parse_json=True,
    )


def run_snapshot(pcc: Path, pcg: Path) -> dict:
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


def check_execution(label: str, snapshot: dict, thresholds: dict[str, int]) -> int:
    failures = 0
    print(f"{label}:")
    for field, expected in thresholds.items():
        value = snapshot.get(field, 0)
        if isinstance(value, (list, dict)):
            actual = len(value)
        else:
            actual = int(value or 0)
        ok = actual >= expected
        print(f"  {field:24} {actual:>8} >= {expected:<8} {'OK' if ok else 'FAIL'}")
        if not ok:
            failures += 1
    return failures


def lookup(snapshot: dict, path: str):
    value = snapshot
    for part in path.split("."):
        if isinstance(value, dict):
            value = value.get(part)
        else:
            return None
    return value


def check_values(label: str, snapshot: dict, expected: dict[str, object]) -> int:
    failures = 0
    print(f"{label}:")
    for path, wanted in expected.items():
        actual = lookup(snapshot, path)
        ok = actual == wanted
        print(f"  {path:24} {actual!r:>10} == {wanted!r:<10} {'OK' if ok else 'FAIL'}")
        if not ok:
            failures += 1
    return failures


def check(
    label: str,
    report: dict,
    thresholds: dict[str, tuple[str, int]],
    typed: dict[str, tuple[str, int]] | None = None,
    zero_tags: list[str] | None = None,
) -> int:
    failures = 0
    print(f"{label}:")
    for field, (operator, expected) in thresholds.items():
        if field == "unsupported_pending":
            # Derived counter: the CLI exposes only the per-tag buckets.
            actual = sum(
                int(value)
                for value in report["coverage"]
                .get("unsupported_pending_by_tag", {})
                .values()
            )
        else:
            actual = report["coverage"][field]
        if operator == ">=":
            ok = actual >= expected
        elif operator == "<=":
            ok = actual <= expected
        elif operator == "==":
            ok = actual == expected
        else:
            raise ValueError(f"unsupported operator: {operator}")
        print(f"  {field:24} {actual:>8} {operator} {expected:<8} {'OK' if ok else 'FAIL'}")
        if not ok:
            failures += 1
    if typed:
        for field, (operator, expected) in typed.items():
            actual = int(report.get(field, 0))
            ok = actual >= expected if operator == ">=" else actual <= expected
            print(f"  {field:24} {actual:>8} {operator} {expected:<8} {'OK' if ok else 'FAIL'}")
            if not ok:
                failures += 1
    if zero_tags:
        legacy = report["coverage"].get("legacy_pending_by_tag", {})
        unsupported = report["coverage"].get("unsupported_pending_by_tag", {})
        for tag in zero_tags:
            actual = int(legacy.get(tag, 0)) + int(unsupported.get(tag, 0))
            ok = actual == 0
            print(f"  pending:{tag:17} {actual:>8} == 0        {'OK' if ok else 'FAIL'}")
            if not ok:
                failures += 1
    return failures


def data_provenance() -> str:
    """Best-effort description of where the pinned data came from."""
    marker = DATA_ROOT / ".pcgen-rust-testdata"
    if marker.exists():
        try:
            info = json.loads(marker.read_text(encoding="utf-8"))
            return (
                f"{info.get('ref', 'unknown')} "
                f"(fetched {info.get('fetched_at', 'unknown')})"
            )
        except (OSError, json.JSONDecodeError):
            pass
    if (DATA_ROOT / ".git").exists():
        try:
            ref = subprocess.check_output(
                ["git", "-C", str(DATA_ROOT), "rev-parse", "HEAD"],
                text=True,
            ).strip()
            return f"{ref} (git checkout)"
        except (OSError, subprocess.SubprocessError):
            pass
    return "unknown (no marker/.git; run scripts/fetch_testdata.sh)"


def _lock_ref() -> str:
    try:
        return json.loads(LOCK_PATH.read_text(encoding="utf-8"))["ref"]
    except (OSError, KeyError, json.JSONDecodeError):
        return "unknown"


def coverage_metrics(report: dict) -> dict:
    """Extract the frozen coverage counters from an `import-report` payload."""
    coverage = report["coverage"]
    legacy_allow = coverage.get("legacy_allowlisted_by_tag", {})
    legacy_pend = coverage.get("legacy_pending_by_tag", {})
    uns_allow = coverage.get("unsupported_allowlisted_by_tag", {})
    uns_pend = coverage.get("unsupported_pending_by_tag", {})
    la = sum(legacy_allow.values())
    lp = sum(legacy_pend.values())
    ua = sum(uns_allow.values())
    up = sum(uns_pend.values())
    semantic_pending = lp + up
    lowered = coverage["lowered_effects"]
    pending_by_tag: dict[str, int] = {}
    for tag, count in legacy_pend.items():
        pending_by_tag[tag] = pending_by_tag.get(tag, 0) + count
    for tag, count in uns_pend.items():
        pending_by_tag[tag] = pending_by_tag.get(tag, 0) + count
    return {
        "entities": coverage["entities"],
        "tag_occurrences": coverage["tag_occurrences"],
        "lowered_effects": lowered,
        "psionic_effects": report.get("psionic_effects", 0),
        "legacy_tags": coverage["legacy_tags"],
        "legacy_allowlisted": la,
        "legacy_pending": lp,
        "unsupported_tags": coverage["unsupported_tags"],
        "unsupported_allowlisted": ua,
        "unsupported_pending": up,
        "semantic_pending": semantic_pending,
        "semantic_completion": lowered / max(1, lowered + semantic_pending),
        "pending_by_tag": pending_by_tag,
    }


def _check_split(label: str, part_a: int, part_b: int, total: int) -> int:
    ok = part_a + part_b == total
    print(
        f"  {label:24} allow {part_a:>7}  pending {part_b:>7}  "
        f"sum == total {'OK' if ok else 'FAIL'}"
    )
    return 0 if ok else 1


def check_coverage_baseline(label: str, report: dict, entry: dict) -> int:
    """Non-regression check against a frozen per-dataset coverage baseline.

    The guard is monotonic rather than exact so a change may move work between
    the legacy/unsupported buckets (reclassification) or lower it, but no tag
    may end up with *more* pending occurrences, and the completion ratio must
    not drop.  Regenerate with ``--write-coverage-baseline`` after an
    intentional regression is reviewed.
    """
    coverage = report["coverage"]
    metrics = coverage_metrics(report)
    failures = 0
    print(f"{label}:")

    # 1) self-consistency of the three-way split
    failures += _check_split(
        "legacy_split",
        metrics["legacy_allowlisted"],
        metrics["legacy_pending"],
        coverage["legacy_tags"],
    )
    failures += _check_split(
        "unsupported_split",
        metrics["unsupported_allowlisted"],
        metrics["unsupported_pending"],
        coverage["unsupported_tags"],
    )

    # 2) dataset must not shrink and lowering must not be lost
    for name in ("entities", "tag_occurrences", "lowered_effects", "psionic_effects"):
        actual = int(metrics.get(name, 0))
        floor = int(entry.get(name, 0))
        ok = actual >= floor
        print(f"  {name:24} {actual:>8} >= {floor:<8} {'OK' if ok else 'FAIL'}")
        if not ok:
            failures += 1

    # 3) total pending work and completion ratio
    ok = metrics["semantic_pending"] <= int(entry["semantic_pending"])
    print(
        f"  {'semantic_pending':24} {metrics['semantic_pending']:>8} "
        f"<= {int(entry['semantic_pending']):<8} {'OK' if ok else 'FAIL'}"
    )
    if not ok:
        failures += 1
    completion = metrics["semantic_completion"]
    floor = float(entry["semantic_completion"])
    ok = completion >= floor - 1e-9
    print(
        f"  {'semantic_completion':24} {completion * 100:7.2f}% "
        f">= {floor * 100:6.2f}%  {'OK' if ok else 'FAIL'}"
    )
    if not ok:
        failures += 1
    print(
        f"  {'semantic_pending':24} {metrics['semantic_pending']:>8} "
        f"(legacy {metrics['legacy_pending']} + unsupported {metrics['unsupported_pending']})"
    )

    # 4) per-tag pending must never grow (reclassification towards allowlist or
    #    lowering is the only accepted direction).
    base_tags = entry.get("pending_by_tag", {})
    current_tags = metrics["pending_by_tag"]
    regressions = [
        (tag, int(base_tags.get(tag, 0)), int(current_tags.get(tag, 0)))
        for tag in sorted(set(base_tags) | set(current_tags))
        if int(current_tags.get(tag, 0)) > int(base_tags.get(tag, 0))
    ]
    if regressions:
        for tag, before, after in regressions:
            print(f"  pending:{tag:20} {before:>8} -> {after:>8}  FAIL (grew)")
        failures += len(regressions)
    else:
        print(
            f"  {'pending_by_tag':24} no tag increased across "
            f"{len(current_tags)} pending tags  OK"
        )
    return failures


def build_coverage_baseline() -> dict:
    """Regenerate the frozen coverage baseline (intentional updates only)."""
    datasets: dict[str, dict] = {}
    for key, pcc in DATASETS.items():
        metrics = coverage_metrics(run_report(pcc))
        datasets[key] = {
            "pcc": str(pcc.relative_to(DATA_ROOT)),
            "entities": metrics["entities"],
            "tag_occurrences": metrics["tag_occurrences"],
            "lowered_effects": metrics["lowered_effects"],
            "psionic_effects": metrics["psionic_effects"],
            "legacy_tags": metrics["legacy_tags"],
            "legacy_allowlisted": metrics["legacy_allowlisted"],
            "legacy_pending": metrics["legacy_pending"],
            "unsupported_tags": metrics["unsupported_tags"],
            "unsupported_allowlisted": metrics["unsupported_allowlisted"],
            "unsupported_pending": metrics["unsupported_pending"],
            "semantic_pending": metrics["semantic_pending"],
            "semantic_completion": metrics["semantic_completion"],
            "pending_by_tag": dict(sorted(metrics["pending_by_tag"].items())),
        }
    return {
        "description": (
            "Frozen coverage counters for DS-3E (regression proxy) and DS-35E "
            "(primary acceptance). Regenerate with "
            "`python3 scripts/coverage_gate.py --write-coverage-baseline`."
        ),
        "data_ref": _lock_ref(),
        "datasets": datasets,
    }


def coverage_entry_regressions(old: dict, new: dict) -> list[str]:
    """Regressions when replacing baseline `old` with freshly-measured `new`.

    Used by ``--write-coverage-baseline`` so an improvement cannot be followed
    by silently re-freezing a worse value.
    """
    regressions: list[str] = []
    for name in ("entities", "tag_occurrences", "lowered_effects", "psionic_effects"):
        if int(new.get(name, 0)) < int(old.get(name, 0)):
            regressions.append(f"{name}: {old.get(name)} -> {new[name]}")
    if int(new["semantic_pending"]) > int(old.get("semantic_pending", 0)):
        regressions.append(
            f"semantic_pending: {old.get('semantic_pending')} -> "
            f"{new['semantic_pending']}"
        )
    if float(new["semantic_completion"]) < float(old.get("semantic_completion", 0.0)) - 1e-9:
        regressions.append(
            f"semantic_completion: {float(old.get('semantic_completion', 0.0)) * 100:.2f}%"
            f" -> {float(new['semantic_completion']) * 100:.2f}%"
        )
    old_tags = old.get("pending_by_tag", {})
    new_tags = new.get("pending_by_tag", {})
    for tag in sorted(set(old_tags) | set(new_tags)):
        if int(new_tags.get(tag, 0)) > int(old_tags.get(tag, 0)):
            regressions.append(
                f"pending:{tag}: {old_tags.get(tag, 0)} -> {new_tags.get(tag, 0)}"
            )
    return regressions


class GateStats:
    def __init__(self) -> None:
        self.passed = 0
        self.failed = 0
        self.skipped: list[str] = []

    def add(self, label: str, failures: int) -> None:
        if failures:
            self.failed += failures
            print(f"FAIL {label}: {failures} check(s)")
        else:
            self.passed += 1
            print(f"PASS {label}")

    def skip(self, label: str, reason: str) -> None:
        self.skipped.append(label)
        print(f"SKIP {label}: {reason}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="PCGen-Rust coverage non-regression gate.",
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument(
        "--allow-skip",
        action="store_true",
        help=(
            "run the gates that do not need pinned PCGen data and exit 0 even "
            "when real-data gates are skipped.  Use only for the no-data CI "
            "smoke job; it is not a correctness gate."
        ),
    )
    group.add_argument(
        "--require-data",
        action="store_true",
        help=(
            "explicitly require the pinned PCGen data.  This is the default; "
            "the flag exists so CI can be self-documenting."
        ),
    )
    parser.add_argument(
        "--accept-coverage-diff",
        action="store_true",
        help=(
            "allow --write-coverage-baseline to record a coverage regression. "
            "Without it the write is refused."
        ),
    )
    parser.add_argument(
        "--write-coverage-baseline",
        action="store_true",
        help=(
            "regenerate tests/fixtures/coverage/coverage_baseline.json from the "
            "pinned data and exit.  Only for a reviewed, intentional coverage "
            "regression."
        ),
    )
    args = parser.parse_args(argv)

    if args.write_coverage_baseline:
        missing = [str(p) for p in DATASETS.values() if not p.exists()]
        if missing:
            print("ERROR: cannot write coverage baseline; missing " + ", ".join(missing))
            return 2
        baseline = build_coverage_baseline()
        if COVERAGE_BASELINE.exists() and not args.accept_coverage_diff:
            old_data = json.loads(COVERAGE_BASELINE.read_text(encoding="utf-8"))
            regressions: list[str] = []
            for key, entry in baseline["datasets"].items():
                old_entry = old_data.get("datasets", {}).get(key)
                if old_entry is not None:
                    regressions.extend(
                        f"{key}: {item}"
                        for item in coverage_entry_regressions(old_entry, entry)
                    )
            if regressions:
                print("refusing to write coverage baseline: coverage regressed")
                for regression in regressions:
                    print(f"  {regression}")
                print(
                    "re-run with --accept-coverage-diff only if this regression "
                    "is intentional and reviewed."
                )
                return 1
        COVERAGE_BASELINE.parent.mkdir(parents=True, exist_ok=True)
        COVERAGE_BASELINE.write_text(
            json.dumps(baseline, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"wrote {COVERAGE_BASELINE}")
        for key, entry in baseline["datasets"].items():
            print(
                f"  {key}: entities={entry['entities']} "
                f"lowered={entry['lowered_effects']} "
                f"pending={entry['semantic_pending']} "
                f"completion={entry['semantic_completion'] * 100:.1f}%"
            )
        return 0

    stats = GateStats()
    print("coverage gate")
    print(f"  data root : {DATA_ROOT}")
    print(f"  provenance: {data_provenance()}")
    print()

    if args.require_data and not REAL.exists():
        print(
            f"ERROR: --require-data was given but {REAL} is missing; "
            "run scripts/fetch_testdata.sh first"
        )
        return 2

    if REAL.exists() and FIXTURE_PCG.exists():
        snapshot = run_snapshot(REAL, FIXTURE_PCG)
        stats.add(
            "execution coverage: FigFae",
            check_execution(
                "execution coverage: FigFae",
                snapshot,
                {
                    "applied_effects": 10,
                    "active_abilities": 5,
                    "skills": 3,
                    "variables": 1,
                },
            ),
        )
        stats.add(
            "field-value differential: FigFae",
            check_values(
                "field-value differential: FigFae",
                snapshot,
                {
                    "hit_points": 32,
                    "base_attack_bonus": 4,
                    "initiative": 3,
                    "armor_class": 13,
                    "touch_ac": 13,
                    "flat_footed_ac": 10,
                    "saves.Fortitude": 6,
                    "saves.Reflex": 4,
                    "saves.Will": 0,
                    "skills.Climb": 10,
                    "skills.Disable Device": 4,
                    "skills.Jump": 10,
                },
            ),
        )
    else:
        stats.skip(
            "execution coverage: FigFae",
            f"missing {REAL} or {FIXTURE_PCG}",
        )
        stats.skip(
            "field-value differential: FigFae",
            f"missing {REAL} or {FIXTURE_PCG}",
        )

    if REAL.exists() and BRDJOE_PCG.exists():
        snapshot = run_snapshot(REAL, BRDJOE_PCG)
        stats.add(
            "execution coverage: BrdJoe",
            check_execution(
                "execution coverage: BrdJoe",
                snapshot,
                {
                    "applied_effects": 10,
                    "active_abilities": 5,
                    "spell_dcs": 2,
                    "prepared_spells": 4,
                    "caster_levels": 1,
                },
            ),
        )
    else:
        stats.skip(
            "execution coverage: BrdJoe",
            f"missing {REAL} or {BRDJOE_PCG}",
        )

    if BARD_PCC.exists() and BARD_PCG.exists():
        snapshot = run_snapshot(BARD_PCC, BARD_PCG)
        stats.add(
            "field-value differential: Bard",
            check_values(
                "field-value differential: Bard",
                snapshot,
                {
                    "hit_points": 14,
                    "base_attack_bonus": 3,
                    "armor_class": 11,
                    "touch_ac": 11,
                    "flat_footed_ac": 10,
                    "saves.Fortitude": 1,
                    "saves.Reflex": 7,
                    "saves.Will": 4,
                    "skills.Climb": 0,
                    "skills.Diplomacy": 2,
                    "skills.Hide": 1,
                    "skills.Move Silently": 1,
                },
            ),
        )
    else:
        stats.skip(
            "field-value differential: Bard",
            f"missing {BARD_PCC} or {BARD_PCG}",
        )

    if MINI.exists():
        stats.add(
            "mini campaign",
            check(
                "mini campaign",
                run_report(MINI),
                {
                    "entities": (">=", 8),
                    "lowered_effects": (">=", 1),
                    "legacy_tags": ("<=", 2),
                    "unsupported_tags": ("<=", 5),
                    "tag_occurrences": (">=", 20),
                    "known_tag_occurrences": (">=", 20),
                },
                typed={
                    "races": (">=", 1),
                    "classes": (">=", 2),
                    "skills": (">=", 4),
                    "spells": (">=", 1),
                    "equipment": (">=", 1),
                    "progressions": (">=", 2),
                },
            ),
        )
    else:
        stats.skip("mini campaign", f"missing {MINI}")

    if REAL.exists():
        stats.add(
            "real 3e SRD",
            check(
                "real 3e SRD",
                run_report(REAL),
                {
                    "entities": (">=", 3500),
                    "lowered_effects": (">=", 3000),
                    "unsupported_tags": ("==", 0),
                    "unsupported_pending": ("==", 0),
                    "tag_occurrences": (">=", 20000),
                    "known_tag_occurrences": (">=", 18000),
                    "formula_effects": (">=", 700),
                    "grant_effects": (">=", 500),
                    "choose_effects": (">=", 50),
                    "variable_effects": (">=", 300),
                },
                typed={
                    "races": (">=", 60),
                    "classes": (">=", 45),
                    "skills": (">=", 90),
                    "spells": (">=", 500),
                    "equipment": (">=", 400),
                    "equipment_modifiers": (">=", 100),
                    "deities": (">=", 2),
                    "domains": (">=", 20),
                    "spell_effects": (">=", 10),
                    "progressions": (">=", 45),
                },
                zero_tags=[
                    "PREVARNEQ",
                    "PREMULT",
                    "PRETOTALAB",
                    "PRESPELLTYPE",
                    "PRESPELLBOOK",
                    "PRESPELLCAST",
                    "PRETEMPLATE",
                    "PREHANDSGTEQ",
                    "PREHANDSEQ",
                    "PREMOVE",
                    "ALTCRITMULT",
                    "ALTCRITRANGE",
                    "ALTDAMAGE",
                    "ALTEQMOD",
                    "ALTTYPE",
                    "ARMORTYPE",
                    "BASEQTY",
                    "CHANGEPROF",
                    "WEAPONBONUS",
                    "ADDDOMAINS",
                    "PROHIBITCOST",
                    "DONOTADD",
                    "VARIANTS",
                    "ABILITYLIST",
                    "POOL",
                    "STACK",
                    "CHOICE",
                ],
            ),
        )

        env = os.environ.copy()
        if CARGO_HOME.exists():
            env["CARGO_HOME"] = str(CARGO_HOME)
        result = subprocess.run(
            [sys.executable, "scripts/differential_java_pcgen.py"],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        if result.returncode == 0:
            stats.add("curated Java differential", 0)
            print("  curated Java differential: passed (9 cases)")
        else:
            stats.add("curated Java differential", 1)
            print("  curated Java differential: FAILED")
            print(result.stdout[-2000:])
            print(result.stderr[-1000:])
    else:
        stats.skip("real 3e SRD", f"missing {REAL}")
        stats.skip("curated Java differential", "raw PCGen fixtures missing")

    if DS35E.exists():
        stats.add(
            "real 35e RSRD basics",
            check(
                "real 35e RSRD basics",
                run_report(DS35E),
                {
                    "entities": (">=", 15000),
                    "tag_occurrences": (">=", 70000),
                    "lowered_effects": (">=", 7700),
                    "unsupported_tags": ("<=", 2),
                    "unsupported_pending": ("==", 0),
                },
                typed={"spell_effects": (">=", 30)},
                zero_tags=[
                    "PRESPELLTYPE",
                    "PRESPELLBOOK",
                    "PRESPELLCAST",
                    "PRESIZEEQ",
                    "PRESIZEGTEQ",
                    "PRETEMPLATE",
                    "PREHANDSGTEQ",
                    "PREHANDSEQ",
                    "PREMOVE",
                    "ALTCRITMULT",
                    "ALTCRITRANGE",
                    "ALTDAMAGE",
                    "ALTEQMOD",
                    "ALTTYPE",
                    "ARMORTYPE",
                    "BASEQTY",
                    "CHANGEPROF",
                    "WEAPONBONUS",
                    "ADDDOMAINS",
                    "PROHIBITCOST",
                    "DONOTADD",
                    "VARIANTS",
                    "ABILITYLIST",
                    "POOL",
                    "STACK",
                    "CHOICE",
                ],
            ),
        )
    else:
        stats.skip("real 35e RSRD basics", f"missing {DS35E}")

    if DSPSI.exists():
        stats.add(
            "real 35e RSRD psionics",
            check(
                "real 35e RSRD psionics",
                run_report(DSPSI),
                {
                    "entities": (">=", 1500),
                    "lowered_effects": (">=", 1100),
                    "unsupported_tags": ("==", 0),
                    "unsupported_pending": ("==", 0),
                },
                zero_tags=[
                    "ALTCRITMULT",
                    "ALTCRITRANGE",
                    "ALTDAMAGE",
                    "ALTEQMOD",
                    "ALTTYPE",
                    "ARMORTYPE",
                    "BASEQTY",
                    "CHANGEPROF",
                    "WEAPONBONUS",
                    "ADDDOMAINS",
                    "PROHIBITCOST",
                    "DONOTADD",
                    "VARIANTS",
                    "ABILITYLIST",
                    "POOL",
                    "STACK",
                    "CHOICE",
                ],
                typed={
                    "classes": (">=", 20),
                    "spells": (">=", 250),
                    "psionic_effects": (">=", 40),
                },
            ),
        )
    else:
        stats.skip("real 35e RSRD psionics", f"missing {DSPSI}")

    if COVERAGE_BASELINE.exists():
        baseline_data = json.loads(COVERAGE_BASELINE.read_text(encoding="utf-8"))
        ref = _lock_ref()
        baseline_ref = baseline_data.get("data_ref")
        if ref != "unknown" and baseline_ref not in (ref, "unknown"):
            print(
                f"WARNING: coverage baseline data_ref {baseline_ref!r} != lock "
                f"ref {ref!r}; regenerate with --write-coverage-baseline"
            )
        for key, pcc in DATASETS.items():
            label = f"coverage baseline ({key})"
            entry = baseline_data.get("datasets", {}).get(key)
            if not pcc.exists():
                stats.skip(label, f"missing {pcc}")
                continue
            if entry is None:
                print(f"FAIL {label}: no baseline entry")
                stats.add(label, 1)
                continue
            stats.add(label, check_coverage_baseline(label, run_report(pcc), entry))
    else:
        stats.skip("coverage baseline", f"missing {COVERAGE_BASELINE}")

    broad = ROOT / "scripts" / "broad_oracle.py"
    cohort = ROOT / "tests/fixtures/differential/broad_cohort.json"
    baseline = ROOT / "tests/fixtures/differential/broad_baseline.json"
    if DATA_ROOT.exists() and cohort.exists() and baseline.exists():
        env = os.environ.copy()
        if CARGO_HOME.exists():
            env["CARGO_HOME"] = str(CARGO_HOME)
        result = subprocess.run(
            [sys.executable, str(broad), "--check", "--json"],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
        )
        summary = None
        if result.stdout.strip():
            try:
                summary = json.loads(result.stdout)
            except json.JSONDecodeError:
                summary = None

        if result.returncode == 0 and isinstance(summary, dict) and summary.get("ok"):
            stats.add("broad Java oracle (per-case)", 0)
            actual = summary["actual"]
            expected = summary["expected"]
            print(
                "  broad oracle: "
                f"{actual['computed']} computed = "
                f"{actual['java_compared']} java-compared + "
                f"{actual['known_diverging']} known-diverging "
                f"({actual['java_gated']} java-gated, "
                f"{actual['data_missing']} data-missing)"
            )
            # The computed denominator must not shrink below the declared
            # cohort expectation, less any explicitly allowed data gaps.
            floor = expected["must_compute"] - expected.get("data_missing_limit", 0)
            if actual["computed"] < floor:
                print(
                    f"  broad oracle denominator {actual['computed']} < expected {floor}"
                )
                stats.add("broad oracle denominator", 1)
            else:
                stats.add("broad oracle denominator", 0)
        else:
            stats.add("broad Java oracle (per-case)", 1)
            if isinstance(summary, dict) and summary.get("failures"):
                for failure in summary["failures"][:20]:
                    print(f"  {failure}")
            else:
                print(result.stdout[-4000:])
                print(result.stderr[-1000:])
    else:
        stats.skip(
            "broad Java oracle (per-case)",
            f"missing {DATA_ROOT} or {cohort} or {baseline}",
        )

    print()
    print(
        "coverage gate summary: "
        f"pass={stats.passed} fail={stats.failed} skip={len(stats.skipped)}"
    )

    if stats.failed:
        print("coverage gate FAILED")
        return 1

    if stats.skipped:
        if not args.allow_skip:
            print(
                "coverage gate FAILED: data-dependent gates were skipped. "
                "This is a hard failure by default."
            )
            print("missing: " + ", ".join(stats.skipped))
            print(
                "run scripts/fetch_testdata.sh, or use --allow-skip for the "
                "explicit no-data smoke gate."
            )
            return 2
        print(
            "coverage gate passed (no-data smoke; "
            f"{len(stats.skipped)} real-data gate(s) skipped)"
        )
        return 0

    print("coverage gate passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
