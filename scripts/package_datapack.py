#!/usr/bin/env python3
"""Package a distributable PCGen data pack for pcgen-rust.

Only the player-facing data directories are copied; the Java test suite is not
part of a data pack.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SOURCE = Path(
    os.environ.get("PCGEN_TESTDATA_DIR", ROOT.parent / "raw" / "pcgen-master")
)

PACKS = {
    "rsrd35": {
        "name": "D&D 3.5e RSRD",
        "entry": "data/35e/wizards_of_the_coast/rsrd/basics/_rsrd_basics.pcc",
        "paths": [
            "data/35e/wizards_of_the_coast/rsrd",
        ],
    },
    "srd30": {
        "name": "D&D 3.0 SRD",
        "entry": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "paths": [
            "data/3e/wizards_of_the_coast/srd",
        ],
    },
}


def data_ref(source: Path) -> str | None:
    lock = ROOT / "scripts" / "testdata.lock.json"
    if lock.exists():
        try:
            return json.loads(lock.read_text(encoding="utf-8")).get("ref")
        except json.JSONDecodeError:
            return None
    marker = source / ".pcgen-rust-testdata"
    if marker.exists():
        text = marker.read_text(encoding="utf-8", errors="ignore")
        for line in text.splitlines():
            if line.startswith("ref="):
                return line.split("=", 1)[1].strip()
    return None


def build_pack(pack_id: str, source: Path, out: Path, dry_run: bool, make_zip: bool) -> int:
    spec = PACKS[pack_id]
    entry = source / spec["entry"]
    if not entry.exists():
        print(f"entry not found: {entry}", file=sys.stderr)
        return 1
    missing = [path for path in spec["paths"] if not (source / path).exists()]
    if missing:
        print(f"source paths missing under {source}: {missing}", file=sys.stderr)
        return 1

    version = data_ref(source) or "unknown"
    manifest = {
        "id": pack_id,
        "name": spec["name"],
        "entry": spec["entry"],
        "version": version,
        "data_ref": version,
        "source": str(source),
        "license": "OGL-1.0a / PCGen data terms",
    }
    print(f"pack {pack_id}: {spec['name']}")
    print(f"  source: {source}")
    print(f"  out:    {out}")
    print(f"  entry:  {spec['entry']}")
    for path in spec["paths"]:
        print(f"  copy:   {path}")
    if dry_run:
        return 0

    out.mkdir(parents=True, exist_ok=True)
    for path in spec["paths"]:
        src = source / path
        dst = out / path
        dst.parent.mkdir(parents=True, exist_ok=True)
        if dst.exists():
            shutil.rmtree(dst)
        shutil.copytree(src, dst)
    (out / "pack.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    if make_zip:
        archive = shutil.make_archive(str(out), "zip", out)
        print(f"  zip:    {archive}")
    print(f"wrote {out}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Package a pcgen-rust data pack.")
    parser.add_argument("--pack", choices=sorted(PACKS), required=True)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--zip", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    return build_pack(args.pack, args.source, args.out, args.dry_run, args.zip)


if __name__ == "__main__":
    raise SystemExit(main())
