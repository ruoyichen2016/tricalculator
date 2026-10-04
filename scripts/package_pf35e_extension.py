#!/usr/bin/env python3
"""Materialise the PF-as-3.5e extension so its `PCC:` include resolves.

The extension source lives in `extensions/pf35e/` (committed). Because PCGen
include resolution is relative to the data tree, this script copies the
extension under `<pcgen-data>/data/` and writes the entry `.pcc` with the
absolute path to the chosen RSRD entry, then prints the ready-to-use path.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = Path(os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw" / "pcgen-master"))
DEFAULT_RSRD = "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc"
EXT_SRC = ROOT / "extensions" / "pf35e"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    ap.add_argument("--rsrd", default=DEFAULT_RSRD)
    ap.add_argument("--out", type=Path, default=None,
                    help="destination dir under <source>/data (default: _pf35e_extension)")
    args = ap.parse_args()

    rsrd = args.source / args.rsrd
    if not rsrd.exists():
        print(f"RSRD entry not found: {rsrd}", file=sys.stderr)
        return 2

    out = args.out or (args.source / "data" / "_pf35e_extension")
    out.mkdir(parents=True, exist_ok=True)
    shutil.copytree(EXT_SRC / "data", out / "data", dirs_exist_ok=True)

    template = (EXT_SRC / "rsrd_pf35e.pcc.in").read_text(encoding="utf-8")
    entry = template.replace("@@RSRD_PCC@@", str(rsrd.resolve()))
    (out / "rsrd_pf35e.pcc").write_text(entry, encoding="utf-8")
    print(out / "rsrd_pf35e.pcc")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
