#!/usr/bin/env python3
"""Assemble a friend-facing pcgen-rust Web release."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CARGO_HOME = Path(os.environ.get("CARGO_HOME", ROOT / ".cargo-home"))


def workspace_version() -> str:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.MULTILINE)
    return match.group(1) if match else "0.0.0"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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
    parser.add_argument("--version", default=None)
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

    if os.name != "nt" and not args.dry_run:
        print(
            "WARNING: building on a non-Windows host; run-web.bat cannot run "
            "the packaged binary on Windows. Build the release on Windows to "
            "produce a Windows .exe bundle.",
            file=sys.stderr,
        )

    for doc_name in ("M9_PACKAGING.md", "M9_BETA_CHECKLIST.md"):
        doc = ROOT / "docs" / doc_name
        if doc.exists() and not args.dry_run:
            shutil.copy2(doc, out / doc_name)
    notes = ROOT / "RELEASE_NOTES.md"
    if notes.exists() and not args.dry_run:
        shutil.copy2(notes, out / "RELEASE_NOTES.md")

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

    # Include locally curated custom packs. The default launcher prefers
    # data/packs/custom/curated/bahamut-all-35e3e when present.
    custom_curated = ROOT / "data" / "packs" / "custom" / "curated"
    custom_packs: list[str] = []
    if custom_curated.is_dir():
        for pack_dir in sorted(custom_curated.iterdir()):
            if not pack_dir.is_dir() or (pack_dir / "pack.json").exists() is False:
                continue
            if not args.dry_run:
                dst = out / "data" / "packs" / "custom" / "curated" / pack_dir.name
                if dst.exists():
                    shutil.rmtree(dst)
                shutil.copytree(pack_dir, dst)
            custom_packs.append(pack_dir.name)
            print(f"custom pack: {pack_dir.name}")

    version = args.version or workspace_version()
    if not args.dry_run:
        (out / "characters").mkdir(parents=True, exist_ok=True)
        manifest = {
            "name": "pcgen-rust-web",
            "version": version,
            "platform": sys.platform,
            "built_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
            "packs": packs,
            "custom_packs": custom_packs,
            "binary_sha256": sha256(out / binary_name()),
        }
        (out / "release.json").write_text(
            json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
    if args.zip and not args.dry_run:
        archive = Path(
            shutil.make_archive(
                str(ROOT / "dist" / f"pcgen-rust-web-{version}"), "zip", out
            )
        )
        (ROOT / "dist" / "SHA256SUMS").write_text(
            f"{sha256(archive)}  {archive.name}\n", encoding="utf-8"
        )
        print(f"zip: {archive}")

    print()
    print(f"release dir: {out}")
    print(f"version:     {version}")
    print("run: ./run-web.sh  (Windows: run-web.bat)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
