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
            "data/homebrew/conversion_support",
            "data/_universal",
        ],
    },
    "srd30": {
        "name": "D&D 3.0 SRD",
        "entry": "data/3e/wizards_of_the_coast/srd/basics/srdbasic.pcc",
        "paths": [
            "data/3e/wizards_of_the_coast/srd",
            "data/homebrew/conversion_support",
            "data/_universal",
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


def write_pack_notice(out: Path, pack_id: str, spec: dict, source: Path) -> None:
    """Drop a top-level notice into the pack.

    The OGL text travels inside the copied data tree, but a redistributor
    unpacking a data pack should not have to go looking for it.
    """
    ref = data_ref(source) or "unknown"
    # Each SRD tree ships its own OGL.txt; point at the one actually included.
    ogl_path = {
        "rsrd35": "data/35e/wizards_of_the_coast/rsrd/OGL.txt",
        "srd30": "data/3e/wizards_of_the_coast/srd/OGL.txt",
    }.get(pack_id, "OGL.txt")
    notice = f"""# {spec['name']} — 数据包许可

本数据包（pack id `{pack_id}`）是 **游戏数据**，不是 pcgen-rust 的源码，
因此**不适用** pcgen-rust 的 LGPL-2.1 许可。

## 许可

Open Game License v1.0a。完整条文随包提供：`{ogl_path}`。

## 必须保留的署名（OGL §15）

```text
Open Game License v 1.0a Copyright 2000, Wizards of the Coast, Inc.

System Reference Document Copyright 2000-2003, Wizards of the Coast, Inc.;
Authors Jonathan Tweet, Monte Cook, Skip Williams, Rich Baker, Andy Collins,
David Noonan, Rich Redman, Bruce R. Cordell, John D. Rateliff, Thomas Reid,
James Wyatt, based on original material by E. Gary Gygax and Dave Arneson.

PCGen dataset conversion for the System Reference Document
Copyright 2001-2003, PCgen Data team (Including, but not limited to
Bryan McRoberts, Rob Reed (Mynex), Eric Beaudoin, Andrew McDougall
(Tir Gwaith), Ian Dale, Paul W. King, Chris Chandler (Barak),
Sigurdur H. Olafsson)
```

## 来源

- 上游：PCGen project (`PCGen/pcgen`)
- ref：`{ref}`
- 本项目源码的许可与完整说明见仓库根目录 `NOTICE.md`。

## 商标

`Dungeons & Dragons`、`d20 System` 是 Wizards of the Coast, Inc. 的商标。
本数据包与 Wizards of the Coast 无隶属关系，未获其背书。
"""
    (out / "NOTICE.md").write_text(notice, encoding="utf-8")


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
    write_pack_notice(out, pack_id, spec, source)
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
