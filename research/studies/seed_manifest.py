#!/usr/bin/env python3
"""Seed docs/parity/catalog.json study entries from studies-catalog.json.

tickview_id: existing wire studies keep their key; new studies use the
snake->Pascal names from the milestone-3 plan dedup table. pane: existing
studies read from the PARITY.md studies table; new studies use the plan's
pane assignment (price overlays vs separate panes).
"""
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "docs" / "parity" / "catalog.json"
CATALOG = ROOT / "docs" / "evidence" / "tv" / "studies-catalog.json"
PARITY = ROOT / "PARITY.md"

NEW_IDS = {
    "STD;Aroon": ("Aroon", "separate"),
    "STD;Average_True_Range": ("Atr", "separate"),
    "STD;Bollinger_Bands_Width": ("BollingerBandsWidth", "separate"),
    "STD;Chandelier_Exit": ("ChandelierExit", "price"),
    "STD;Choppiness_Index": ("ChoppinessIndex", "separate"),
    "STD;Connors_RSI": ("ConnorsRsi", "separate"),
    "STD;Correlation_Coeff": ("CorrelationCoeff", "separate"),
    "STD;DMI": ("Dmi", "separate"),
    "STD;Fisher_Transform": ("FisherTransform", "separate"),
    "STD;Know_Sure_Thing": ("KnowSureThing", "separate"),
    "STD;Price_Oscillator": ("PriceOscillator", "separate"),
    "STD;Rank_Correlation_Index": ("RankCorrelationIndex", "separate"),
    "STD;Relative_Volatility_Index": ("RelativeVolatilityIndex", "separate"),
    "STD;SMI_Ergodic_Indicator_Oscillator": ("SmiErgodic", "separate"),
    "STD;Volatility_Stop": ("VolatilityStop", "price"),
    "STD;Whilliams_Fractals": ("WilliamsFractals", "price"),
    "STD;Zig_Zag": ("ZigZag", "price"),
}


def parity_panes() -> dict:
    text = PARITY.read_text()
    panes = {}
    for m in re.finditer(r"\|[^|]+\| `([A-Za-z]+)` \| (price|volume|separate) \|", text):
        panes[m.group(1)] = m.group(2)
    return panes


def main() -> None:
    manifest = json.loads(MANIFEST.read_text())
    catalog = json.loads(CATALOG.read_text())
    panes = parity_panes()

    studies = []
    for row in catalog["tools"]:
        tv_id = row["tv_id"]
        excluded = row["scope"] == "excluded"
        if excluded:
            tickview_id, pane = "", ""
        elif row["tickview_id"]:
            tickview_id = row["tickview_id"]
            pane = panes.get(tickview_id, "separate")
            assert pane, f"no pane for {tickview_id}"
        else:
            tickview_id, pane = NEW_IDS[tv_id]
        studies.append({
            "kind": "study",
            "tv_id": tv_id,
            "tickview_id": tickview_id,
            "name": row["name"],
            "pane": pane,
            "status": "excluded" if excluded else "in-progress",
            "tests": [],
            "evidence": [],
            "notes": "",
            "exclusion_reason": row["exclusion_reason"],
        })

    manifest["studies"] = studies
    MANIFEST.write_text(json.dumps(manifest, indent=1) + "\n")
    print(f"seeded {len(studies)} study entries")


if __name__ == "__main__":
    main()
