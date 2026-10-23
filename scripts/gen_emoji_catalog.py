#!/usr/bin/env python3
"""Generate crates/app/assets/openmoji/catalog.json from an OpenMoji checkout.

Filters data/openmoji.json down to the grid catalog: base emoji only (no
skin-tone variants), CLDR groups mapped to the eight reference-style tabs
(Smileys merges smileys-emotion + people-body). `component` swatches and the
OpenMoji/Unicode extras groups stay out of the grid. Copies the matching color
SVGs into crates/app/assets/openmoji/color/ keeping upstream filenames.

Usage: scripts/gen_emoji_catalog.py /path/to/openmoji
Exits non-zero when a catalog entry has no vendored SVG.
"""

import json
import shutil
import sys
from pathlib import Path

GROUPS = {
    "smileys-emotion": "Smileys",
    "people-body": "Smileys",
    "animals-nature": "Animals",
    "food-drink": "Food",
    "activities": "Activity",
    "travel-places": "Travel",
    "objects": "Objects",
    "symbols": "Symbols",
    "flags": "Flags",
}
GROUP_ORDER = ["Smileys", "Animals", "Food", "Activity", "Travel", "Objects", "Symbols", "Flags"]

SKINTONE_SELECTORS = {"1F3FB", "1F3FC", "1F3FD", "1F3FE", "1F3FF"}

REPO = Path(__file__).resolve().parent.parent
OUT_DIR = REPO / "crates" / "app" / "assets" / "openmoji"


def main() -> int:
    src = Path(sys.argv[1])
    entries = json.loads((src / "data" / "openmoji.json").read_text(encoding="utf-8"))
    svg_dir = src / "color" / "svg"
    catalog = []
    missing = []
    for e in entries:
        group = GROUPS.get(e["group"])
        if group is None:
            continue
        hexcode = e["hexcode"]
        # Drop skin-tone VARIANTS only (hexcode contains a tone selector).
        # Bases stay in: skintone_combination "single" marks the tone-capable
        # base itself, and excluding it removed every hand, person, couple and
        # family from the picker.
        if any(part in SKINTONE_SELECTORS for part in hexcode.split("-")):
            continue
        if not (svg_dir / f"{hexcode}.svg").is_file():
            missing.append(hexcode)
            continue
        catalog.append(
            {
                "glyph": e["emoji"],
                "hex": hexcode,
                "group": group,
                "name": e["annotation"],
                "tags": [t.strip() for t in e["tags"].split(",") if t.strip()] if e["tags"] else [],
                "order": e["order"],
            }
        )
    if missing:
        print(f"missing SVGs for {len(missing)} entries: {missing[:10]}", file=sys.stderr)
        return 1
    catalog.sort(key=lambda c: c["order"])
    assert [g for g in GROUP_ORDER] == GROUP_ORDER
    for g in GROUP_ORDER:
        n = sum(1 for c in catalog if c["group"] == g)
        assert n > 0, f"group {g} empty"
    (OUT_DIR / "catalog.json").write_text(
        json.dumps(catalog, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8"
    )
    color_out = OUT_DIR / "color"
    color_out.mkdir(parents=True, exist_ok=True)
    copied = 0
    for c in catalog:
        dst = color_out / f"{c['hex']}.svg"
        if not dst.exists():
            shutil.copy2(svg_dir / f"{c['hex']}.svg", dst)
            copied += 1
    print(f"catalog entries: {len(catalog)}, svgs copied: {copied}, total files: {len(list(color_out.glob('*.svg')))}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
