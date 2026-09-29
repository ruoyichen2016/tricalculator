#!/usr/bin/env python3
"""Generate and check docs/COHORT_MATRIX.md.

The PCGen test suite contains more PCG/XML/HTM files than the primary
acceptance cohort.  This script classifies every file so that nothing is
silently ignored, and keeps the checked-in matrix in sync with the real
fixture directory.

Usage:
    python3 scripts/cohort_matrix.py --write   # regenerate the matrix
    python3 scripts/cohort_matrix.py --check   # fail if the matrix is stale
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DATA_ROOT = Path(
    os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw" / "pcgen-master")
)
TESTSUITE = DATA_ROOT / "code" / "testsuite"
PCG_DIR = TESTSUITE / "PCGfiles"
CSHEET_DIR = TESTSUITE / "csheets"
PRIMARY_PATH = ROOT / "tests/fixtures/differential/broad_cohort.json"
EXTENDED_PATH = ROOT / "tests/fixtures/differential/broad_cohort_extended.json"
OUT_PATH = ROOT / "docs/COHORT_MATRIX.md"

HTML_SUFFIX = re.compile(r"^(?P<base>.+?)(?:[-_](?:std|cmp|cmb|sb2))?$")


def load_case_names(path: Path) -> set[str]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    return set(payload.get("cases", {}))


def htm_base(stem: str) -> str:
    """Map a csheet HTM stem back to its PCG stem when possible.

    Handles both `3e_BarJack` and `35e_L10(fighter-human-std)`, where the
    variant suffix sits before the closing parenthesis.
    """
    if stem.endswith(")"):
        inner = stem[:-1]
        match = re.match(r"^(?P<base>.*?)(?:[-_](?:std|cmp|cmb|sb2))?$", inner)
        if match:
            return f"{match.group('base')})"
    return stem


def build_inventory() -> dict:
    pcgs = sorted(path.stem for path in PCG_DIR.glob("*.pcg"))
    xmls = sorted(path.stem for path in CSHEET_DIR.glob("*.xml"))
    htm_files = sorted(path.name for path in CSHEET_DIR.glob("*.htm"))

    xml_set = set(xmls)
    html_by_base: dict[str, list[str]] = {}
    for name in htm_files:
        html_by_base.setdefault(htm_base(Path(name).stem), []).append(name)

    return {
        "pcgs": pcgs,
        "xmls": xmls,
        "xml_set": xml_set,
        "htm_files": htm_files,
        "html_by_base": html_by_base,
    }


def classify(stem: str, primary: set[str], extended: set[str]) -> tuple[str, str]:
    if stem in primary:
        return "included-primary", "主验收 cohort（3.0/3.5e）"
    if stem in extended:
        return "included-extended", "MSRD extended visibility，不参与主验收"
    if stem.startswith(("pf_", "sf_")):
        return "out-of-scope", "Pathfinder / Starfinder，产品范围外"
    if "companion" in stem.lower() or stem == "35e_Q-PsiCrystal":
        return "companion", "companion / psicrystal，需要独立 companion 入口"
    if stem.startswith("35e_UT~"):
        return "unit-test", "PCGen 单元测试用 PCG，不是完整角色"
    if stem.startswith(("none_", "Zen_")):
        return "legacy", "legacy PCG / 无完整 campaign 映射"
    return "unclassified", "需要人工裁决"


def sheet_summary(stem: str, inventory: dict) -> str:
    parts: list[str] = []
    if stem in inventory["xml_set"]:
        parts.append("xml")
    if stem in inventory["html_by_base"]:
        parts.append("htm(" + ", ".join(inventory["html_by_base"][stem]) + ")")
    return " ".join(parts) if parts else "-"


def render() -> str:
    inventory = build_inventory()
    primary = load_case_names(PRIMARY_PATH)
    extended = load_case_names(EXTENDED_PATH)

    rows = []
    counts: dict[str, int] = {}
    for stem in inventory["pcgs"]:
        status, reason = classify(stem, primary, extended)
        counts[status] = counts.get(status, 0) + 1
        rows.append(
            {
                "stem": stem,
                "status": status,
                "reason": reason,
                "sheets": sheet_summary(stem, inventory),
            }
        )

    artifacts = sorted(
        base
        for base in inventory["html_by_base"]
        if base not in set(inventory["pcgs"])
    )

    primary_rows = [row for row in rows if row["status"] == "included-primary"]
    extended_rows = [row for row in rows if row["status"] == "included-extended"]
    other_rows = [
        row
        for row in rows
        if row["status"] not in ("included-primary", "included-extended")
    ]

    lines: list[str] = []
    lines.append("# Cohort Matrix")
    lines.append("")
    lines.append(
        "> 由 `scripts/cohort_matrix.py --write` 生成；`--check` 在 full-data CI 中"
        "保证每个 fixture 都被归类。"
    )
    lines.append("")
    lines.append("## Summary")
    lines.append("")
    lines.append(f"- PCG files: **{len(inventory['pcgs'])}**")
    lines.append(f"- XML csheets: **{len(inventory['xmls'])}**")
    lines.append(f"- HTM csheets: **{len(inventory['htm_files'])}**")
    lines.append(f"- Primary cohort: **{len(primary_rows)}**")
    lines.append(f"- Extended cohort: **{len(extended_rows)}**")
    lines.append(
        "- Non-primary: "
        + ", ".join(f"{name}={counts[name]}" for name in sorted(counts) if name not in ("included-primary", "included-extended"))
    )
    lines.append("")
    lines.append("## Primary cohort")
    lines.append("")
    lines.append("| PCG | Status | Sheets | Reason |")
    lines.append("|---|---|---|---|")
    for row in primary_rows:
        lines.append(
            f"| `{row['stem']}` | {row['status']} | {row['sheets']} | {row['reason']} |"
        )
    lines.append("")
    lines.append("## Extended cohort")
    lines.append("")
    lines.append("| PCG | Status | Sheets | Reason |")
    lines.append("|---|---|---|---|")
    for row in extended_rows:
        lines.append(
            f"| `{row['stem']}` | {row['status']} | {row['sheets']} | {row['reason']} |"
        )
    lines.append("")
    lines.append("## Non-primary fixtures")
    lines.append("")
    lines.append("| PCG | Status | Sheets | Reason / next action |")
    lines.append("|---|---|---|---|")
    for row in other_rows:
        lines.append(
            f"| `{row['stem']}` | {row['status']} | {row['sheets']} | {row['reason']} |"
        )
    lines.append("")
    lines.append("## Csheet-only artifacts")
    lines.append("")
    if artifacts:
        for name in artifacts:
            lines.append(f"- `{name}`: 无对应 PCG，保留为测试套件 artifact/长角色表。")
    else:
        lines.append("- 无")
    lines.append("")
    lines.append("## 未纳入裁决")
    lines.append("")
    lines.append(
        "- `35e_Q-PsiCrystal`：companion / psicrystal。Rust 可计算但 HP/BAB 为 0，"
        "需要独立 companion 入口后再纳入，不进入 primary。"
    )
    lines.append(
        "- `none_derth`：PCG v2.4.3 legacy 格式，无 CAMPAIGN 行；当前不映射到"
        "任何验收 PCC，保留为 legacy 审计样本。"
    )
    lines.append(
        "- `pf_*` / `sf_*`：Pathfinder / Starfinder，明确在 PRODUCT_SCOPE 之外。"
    )
    lines.append("")
    return "\n".join(lines) + "\n"


def check() -> int:
    if not TESTSUITE.exists():
        print(f"SKIP cohort matrix: {TESTSUITE} not found", file=sys.stderr)
        return 0
    rendered = render()
    if not OUT_PATH.exists():
        print(f"cohort matrix missing: {OUT_PATH}; run --write", file=sys.stderr)
        return 1
    current = OUT_PATH.read_text(encoding="utf-8")
    if current != rendered:
        print(
            "cohort matrix is stale; run python3 scripts/cohort_matrix.py --write",
            file=sys.stderr,
        )
        return 1
    inventory = build_inventory()
    primary = load_case_names(PRIMARY_PATH)
    extended = load_case_names(EXTENDED_PATH)
    unclassified = [
        stem
        for stem in inventory["pcgs"]
        if classify(stem, primary, extended)[0] == "unclassified"
    ]
    if unclassified:
        print(f"unclassified PCG files: {unclassified}", file=sys.stderr)
        return 1
    print(
        f"cohort matrix OK: {len(inventory['pcgs'])} PCG, "
        f"{len(primary)} primary, {len(extended)} extended"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Cohort matrix generator/checker.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    if args.write:
        OUT_PATH.write_text(render(), encoding="utf-8")
        print(f"wrote {OUT_PATH}")
        return 0
    return check()


if __name__ == "__main__":
    raise SystemExit(main())
