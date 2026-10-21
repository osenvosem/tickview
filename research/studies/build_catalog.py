#!/usr/bin/env python3
"""Build docs/evidence/tv/studies-catalog.json from captured TV evidence.

Sources (spec precedence):
  1. wire `all_studies_metadata` payload -> docs/evidence/tv-studies-defaults.json
     (66 studies TickView already mirrors; wire_id carries the pack suffix).
  2. per-study metaInfo fetches -> research/studies/meta/*.json
     (scriptIdPart, readable input defaults, is_price_study, stub flags).

Rows: tv_id, name, scope, exclusion_reason, default_inputs, is_price_study,
source. Pane placement per TickView id is asserted separately against the
registry Def (xtask), since the fetched metaInfo omits panes for pine stubs.
"""
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
META = ROOT / "research" / "studies" / "meta"
WIRE = ROOT / "docs" / "evidence" / "tv-studies-defaults.json"
OUT = ROOT / "docs" / "evidence" / "tv" / "studies-catalog.json"
COPY = ROOT / "docs" / "evidence" / "tv" / "studies"

# TV meta ids that describe a study already present in the wire payload.
# value = wire payload key (TickView id) it collapses onto.
DEDUP = {
    "STD;EMA": "Ema",
    "STD;SMA": "Sma",
    "STD;WMA": "Wma",
    "STD;DEMA": "Dema",
    "STD;TEMA": "Tema",
    "STD;VWMA": "Vwma",
    "STD;Hull_MA": "Hma",
    "STD;Williams_Alligator": "Alligator",
    "STD;Bollinger_Bands_B": "PercentB",
    "STD;Money_Flow": "MoneyFlowIndex",
    "STD;On_Balance_Volume": "Obv",
    "STD;Price_Volume_Trend": "VolumePriceTrend",
    "STD;EOM": "EaseOfMovement",
    "STD;Chande_Momentum_Oscillator": "Cmo",
    "STD;Willams_R": "WilliamsR",
    "STD;Chaikin_Money_Flow": "ChaikinMoneyFlow",
    "STD;Chaikin_Oscillator": "ChaikinOscillator",
    "STD;Historical_Volatility": "HistoricalVolatility",
    "STD;Accumulation_Distribution": "AccumulationDistribution",
    "STD;Awesome_Oscillator": "AwesomeOscillator",
    "STD;Bollinger_Bands": "BollingerBands",
    "STD;Donchian_Channels": "DonchianChannels",
    "STD;Keltner_Channels": "KeltnerChannels",
    "STD;Linear_Regression": "LinReg",
    "STD;Momentum": "Momentum",
    "STD;Pivot_Points_Standard": "Pivots",
    "STD;Relative_Vigor_Index": "Rvi",
    "STD;Stochastic_RSI": "StochasticRsi",
    "STD;Supertrend": "Supertrend",
    "STD;TRIX": "Trix",
    "STD;Ultimate_Oscillator": "UltimateOscillator",
    "STD;DPO": "Dpo",
    "STD;CCI": "Cci",
    "STD;EFI": "ElderForce",
    "STD;ENV": "Envelope",
    "STD;PSAR": "Psar",
    "STD;ROC": "Roc",
    "STD;RSI": "Rsi",
    "STD;Stochastic": "Stochastic",
    "STD;VWAP": "Vwap",
}

EXCLUDED = {
    "STD;Advance_Decline_Ratio_Bars":
        "market-breadth study requires an external index feed; out of scope per spec §4",
}


def base_inputs(defs: dict) -> dict:
    out = {}
    for k, v in defs.items():
        if re.fullmatch(r"in_\d+", k) and v != "" and v is not None:
            out[k] = v
    return out


def main() -> None:
    wire = json.loads(WIRE.read_text())
    rows = {}
    for tickview_id, e in sorted(wire.items()):
        if "wire_id" in e:
            pack_id = e["wire_id"].split("-")[0]  # drop the version suffix
            source = "wire"
        else:
            # Fetch could not resolve a wire_id for this study; assign the
            # provisional STD;<Key> id per TV's STD naming convention and
            # record the gap (spec §7.1 evidence-gap rule).
            pack_id = f"STD;{tickview_id}"
            source = "derived"
        rows[pack_id] = {
            "tv_id": pack_id,
            "name": e["name"],
            "tickview_id": tickview_id,
            "scope": "in-scope",
            "exclusion_reason": None,
            "default_inputs": e.get("inputs", {}),
            "is_price_study": None,
            "source": source,
        }

    metas = {}
    for f in sorted(META.glob("*.json")):
        d = json.loads(f.read_text())
        metas[d["scriptIdPart"]] = d

    for sid, d in sorted(metas.items()):
        mi = d.get("metaInfo", {})
        if sid in DEDUP:
            continue
        if sid in rows:  # derived wire row: enrich with fetched meta facts
            rows[sid]["default_inputs"] = base_inputs(mi.get("defaults", {}).get("inputs", {}))
            rows[sid]["is_price_study"] = mi.get("is_price_study")
            continue
        if sid in EXCLUDED:
            rows[sid] = {
                "tv_id": sid,
                "name": d["name"],
                "tickview_id": None,
                "scope": "excluded",
                "exclusion_reason": EXCLUDED[sid],
                "default_inputs": base_inputs(mi.get("defaults", {}).get("inputs", {})),
                "is_price_study": mi.get("is_price_study"),
                "source": "meta",
            }
            continue
        rows[sid] = {
            "tv_id": sid,
            "name": d["name"],
            "tickview_id": None,  # filled by seed_manifest.py via the dedup table
            "scope": "in-scope",
            "exclusion_reason": None,
            "default_inputs": base_inputs(mi.get("defaults", {}).get("inputs", {})),
            "is_price_study": mi.get("is_price_study"),
            "source": "meta",
            "is_stub": bool(mi.get("isTVScriptStub")),
        }

    out = {
        "schema_version": 1,
        "source": {
            "target": "TradingView Desktop 3.4.1",
            "wire": "docs/evidence/tv-studies-defaults.json",
            "meta_dir": "docs/evidence/tv/studies",
        },
        "tools": sorted(rows.values(), key=lambda r: r["tv_id"].lower()),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=1) + "\n")

    COPY.mkdir(parents=True, exist_ok=True)
    for sid, d in metas.items():
        safe = re.sub(r"[^A-Za-z0-9_.-]", "_", sid)
        (COPY / f"{safe}.json").write_text(json.dumps(d, indent=1) + "\n")

    tools = out["tools"]
    excl = [r for r in tools if r["scope"] == "excluded"]
    ids = [r["tv_id"] for r in tools]
    assert len(ids) == len(set(ids)), "duplicate tv_id"
    assert len(excl) == 1, f"expected 1 excluded, got {len(excl)}"
    assert all(r["exclusion_reason"] for r in excl), "excluded without reason"
    print(f"rows={len(tools)} excluded={len(excl)} copied_meta={len(metas)}")


if __name__ == "__main__":
    main()
