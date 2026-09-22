#!/usr/bin/env python3
"""Locate/build the pcgen CLI once and invoke the binary directly.

Gate scripts used to call ``cargo run -q -p pcgen-cli`` for every case, which
added process overhead and made it hard to point the gates at an already-built
binary.  This helper builds the CLI once per Python process and then executes
``target/debug/pcgen`` directly.

Set ``PCGEN_BIN`` to use an existing binary without running cargo.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_BIN: Path | None = None


def cargo_env() -> dict[str, str]:
    env = os.environ.copy()
    local_home = ROOT / ".cargo-home"
    if local_home.exists() and "CARGO_HOME" not in env:
        env["CARGO_HOME"] = str(local_home)
    return env


def _default_binary() -> Path:
    for name in ("pcgen", "pcgen.exe"):
        candidate = ROOT / "target" / "debug" / name
        if candidate.exists():
            return candidate
    return ROOT / "target" / "debug" / "pcgen"


def pcgen_bin() -> Path:
    """Return the CLI binary path, building it once if needed."""
    global _BIN
    if _BIN is not None:
        return _BIN

    explicit = os.environ.get("PCGEN_BIN")
    if explicit:
        binary = Path(explicit)
        if not binary.exists():
            raise FileNotFoundError(f"PCGEN_BIN points to a missing binary: {binary}")
        _BIN = binary
        return _BIN

    env = cargo_env()
    subprocess.check_call(
        ["cargo", "build", "-q", "-p", "pcgen-cli"],
        cwd=ROOT,
        env=env,
    )
    binary = _default_binary()
    if not binary.exists():
        raise FileNotFoundError(f"pcgen CLI binary not found at {binary}")
    _BIN = binary
    return _BIN


def run_pcgen(args: list[str], *, parse_json: bool = False):
    """Run the built CLI and optionally parse stdout as JSON."""
    env = cargo_env()
    output = subprocess.check_output(
        [str(pcgen_bin()), *map(str, args)],
        cwd=ROOT,
        env=env,
        text=True,
    )
    if parse_json:
        return json.loads(output)
    return output
