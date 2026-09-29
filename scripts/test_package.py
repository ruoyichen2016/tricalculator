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


def main() -> int:
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
