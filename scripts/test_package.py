#!/usr/bin/env python3
"""Dry-run smoke for data-pack and release tooling."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(*args: str) -> None:
    command = [sys.executable, *args]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if result.returncode != 0:
        print(result.stdout)
        print(result.stderr, file=sys.stderr)
        raise SystemExit(f"failed: {' '.join(command)}")


def check_licensing() -> None:
    """Licensing files must exist and the packer must emit a notice.

    A redistributor needs the OGL terms with the data, and the repo needs its
    own LICENSE; neither is covered by a dry-run.
    """
    for name in ("LICENSE", "NOTICE.md"):
        if not (ROOT / name).exists():
            raise SystemExit(f"missing {name}")
    license_text = (ROOT / "LICENSE").read_text(encoding="utf-8", errors="ignore")
    if "Version 2.1, February 1999" not in license_text:
        raise SystemExit("LICENSE is not the LGPL-2.1 text")
    if 'license = "LGPL-2.1"' not in (ROOT / "Cargo.toml").read_text(encoding="utf-8"):
        raise SystemExit("Cargo.toml license does not match LICENSE")

    packer = (ROOT / "scripts" / "package_datapack.py").read_text(encoding="utf-8")
    for needle in ("write_pack_notice", "NOTICE.md", "Open Game License v 1.0a"):
        if needle not in packer:
            raise SystemExit(f"packer does not emit a license notice ({needle})")

    # Every pack must point at the OGL.txt its own tree actually carries.
    for pack, ogl in (
        ("rsrd35", "data/35e/wizards_of_the_coast/rsrd/OGL.txt"),
        ("srd30", "data/3e/wizards_of_the_coast/srd/OGL.txt"),
    ):
        if f'"{pack}": "{ogl}"' not in packer:
            raise SystemExit(f"pack {pack} does not reference {ogl}")


def main() -> int:
    check_licensing()
    for pack in ("rsrd35", "srd30"):
        run(
            str(ROOT / "scripts" / "package_datapack.py"),
            "--pack",
            pack,
            "--out",
            f"/tmp/pcgen-pack-{pack}",
            "--dry-run",
        )
    run(
        str(ROOT / "scripts" / "package_release.py"),
        "--packs",
        "rsrd35",
        "--dry-run",
    )
    for name in ("run-web.sh", "run-web.bat"):
        if not (ROOT / "scripts" / name).exists():
            raise SystemExit(f"missing launcher: {name}")
    print("package tooling dry-run OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
