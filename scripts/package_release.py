#!/usr/bin/env python3
"""Assemble a friend-facing pcgen-rust Web release."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CARGO_HOME = Path(os.environ.get("CARGO_HOME", ROOT / ".cargo-home"))


def run(command: list[str], dry_run: bool) -> None:
    print("+", " ".join(command))
    if dry_run:
        return
    subprocess.run(command, cwd=ROOT, check=True)


def binary_name() -> str:
    return "pcgen-web.exe" if os.name == "nt" else "pcgen-web"


def main() -> int:
    parser = argparse.ArgumentParser(description="Build a pcgen-rust Web release.")
    parser.add_argument("--out", type=Path, default=ROOT / "dist" / "pcgen-rust-web")
    parser.add_argument("--packs", default="rsrd35", help="comma-separated pack ids")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--zip", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)
    if not args.skip_build:
        env = os.environ.copy()
        env["CARGO_HOME"] = str(CARGO_HOME)
        print("+ cargo build --release -p pcgen-web")
        if not args.dry_run:
            subprocess.run(
                ["cargo", "build", "--release", "-p", "pcgen-web"],
                cwd=ROOT,
                env=env,
                check=True,
            )

    binary = ROOT / "target" / "release" / binary_name()
    if not args.dry_run:
        if not binary.exists():
            print(f"missing release binary: {binary}", file=sys.stderr)
            return 1
        shutil.copy2(binary, out / binary_name())

    for script in ("run-web.sh", "run-web.bat"):
        source = ROOT / "scripts" / script
        if source.exists():
            if not args.dry_run:
                shutil.copy2(source, out / script)
                if script.endswith(".sh"):
                    (out / script).chmod(0o755)

    doc = ROOT / "docs" / "M9_PACKAGING.md"
    if doc.exists() and not args.dry_run:
        shutil.copy2(doc, out / "M9_PACKAGING.md")

    packs = [pack.strip() for pack in args.packs.split(",") if pack.strip()]
    for pack in packs:
        command = [
            sys.executable,
            str(ROOT / "scripts" / "package_datapack.py"),
            "--pack",
            pack,
            "--out",
            str(out / "data" / "packs" / pack),
        ]
        run(command, args.dry_run)

    if not args.dry_run:
        (out / "characters").mkdir(parents=True, exist_ok=True)
    if args.zip and not args.dry_run:
        archive = shutil.make_archive(str(out), "zip", out)
        print(f"zip: {archive}")

    print()
    print(f"release dir: {out}")
    print("run: ./run-web.sh  (Windows: run-web.bat)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
