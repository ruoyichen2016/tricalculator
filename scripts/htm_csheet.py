#!/usr/bin/env python3
"""HTM csheet field extractor.

PCGen's test suite ships some character sheets as HTML (`.htm`) rather than
XML.  Those sheets are still generated from the same data, so the same
numeric fields can be extracted and used for Java differential checks.

The extractor is intentionally conservative: it looks for the sheet's
labelled tables (`Hit Points`, `Armour Class`, `INITIATIVE`, `BASE ATTACK`,
`SAVING THROWS`) and pulls the first bolded numeric cell after each label.
"""

from __future__ import annotations

import re
from pathlib import Path

TAG_RE = re.compile(r"<[^>]+>")
NUM_RE = re.compile(r"([+-]?\d[\d,\-/]*)")


def _plain(text: str) -> str:
    return " ".join(TAG_RE.sub(" ", text).split())


def _first_number(text: str) -> int | None:
    match = NUM_RE.search(text)
    if not match:
        return None
    raw = match.group(1).split("/")[0].replace(",", "")
    try:
        return int(raw)
    except ValueError:
        return None


def _bold_number_after(text: str, marker: str, span: int = 320) -> int | None:
    index = text.find(marker)
    if index < 0:
        return None
    segment = text[index : index + span]
    match = re.search(r"<b>\s*([+-]?\d[\d,\-/]*)\s*", segment)
    if match:
        return _first_number(match.group(1))
    return _first_number(_plain(segment))


def _save_total(text: str, marker: str) -> int | None:
    index = text.find(marker)
    if index < 0:
        return None
    segment = text[index : index + 700]
    match = re.search(r"<b>([+-]?\d+)</b>", segment)
    return int(match.group(1)) if match else None


_JS_CHECK_RE = re.compile(
    r'new Check\("([^"]+)",\s*"[^"]*",\s*\+?(-?\d+)'
)


def _js_values(text: str) -> dict:
    """Extract fields from the JavaScript `-cmb` sheet variant."""
    values: dict = {}
    checks = {name.lower(): int(total) for name, total in _JS_CHECK_RE.findall(text)}
    if checks:
        values["save_fortitude"] = checks.get("fortitude")
        values["save_reflex"] = checks.get("reflex")
        values["save_will"] = checks.get("willpower", checks.get("will"))
    # `new ATK("Melee","+13/+8","+9/+4", ...)`: third value is base attack.
    atk = re.search(r'new ATK\("Melee",\s*"[^"]*",\s*"([+-]?\d+)', text)
    if atk:
        values["bab"] = int(atk.group(1))
    # `new ACObject(nFlat, nTouch, nTotal, nBase, nArmor, nShield, ...)`.
    ac = re.search(r'new ACObject\((-?\d+),(-?\d+),(-?\d+)', text)
    if ac:
        values["flat_footed_ac"] = int(ac.group(1))
        values["touch_ac"] = int(ac.group(2))
        values["armor_class"] = int(ac.group(3))
    # `var Init = new InitObject(+3,+0)`: first value is the total.
    init = re.search(r'new InitObject\(\+?(-?\d+)', text)
    if init:
        values["initiative"] = int(init.group(1))
    # Hit points still appear as a plain number near the HP label.
    return values


def parse_htm_csheet(path: str | Path) -> dict:
    """Extract comparable fields from a PCGen HTML csheet.

    Two sheet variants exist:

    * `-std` / `-cmp`: server-rendered tables with labelled numeric cells;
    * `-cmb`: JavaScript-driven sheet that carries the values in `CheckArr` /
      `ATKArr` constructor calls.
    """
    text = Path(path).read_text(errors="ignore")
    values = {
        "hit_points": _bold_number_after(text, "Hit Points"),
        "armor_class": _bold_number_after(text, "Armour Class"),
        "touch_ac": None,
        "flat_footed_ac": None,
        "initiative": _bold_number_after(text, "INITIATIVE", span=420),
        "bab": _bold_number_after(text, "BASE ATTACK", span=420),
        "save_fortitude": _save_total(text, "Fortitude"),
        "save_reflex": _save_total(text, "Reflex"),
        "save_will": _save_total(text, "Will"),
    }
    js = _js_values(text)
    for key, value in js.items():
        if values.get(key) is None:
            values[key] = value
    return values
