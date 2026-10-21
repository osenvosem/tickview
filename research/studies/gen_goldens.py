#!/usr/bin/env python3
"""Generate gold_mis.rs: shared fixture2 + full-series goldens for the
moving-average family (milestone 3 task 4).

Reference formulas mirror TradingView Pine semantics (see series.rs docs and
official TV docs); defaults come from the wire evidence with documented doc
fallbacks. Deterministic: re-running must reproduce gold_mis.rs byte-identically.
"""
import json
import math
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "crates" / "ta" / "src" / "gold_mis.rs"
EVIDENCE = ROOT / "docs" / "evidence" / "tv-studies-defaults.json"

N = 260

# ---------------------------------------------------------------- fixture
t = [1_700_000_000 + i * 86_400 for i in range(N)]
closes = [
    round(100 + 8 * math.sin(i / 7.0) + 0.15 * i + (-1.2 if i % 5 == 0 else 0.0), 6)
    for i in range(N)
]
opens = [closes[0]] + closes[:-1]
highs = [
    round(max(o, c) + 0.6 + 0.3 * math.sin(i / 3.0), 6)
    for i, (o, c) in enumerate(zip(opens, closes))
]
lows = [
    round(min(o, c) - 0.6 - 0.3 * math.sin(i / 5.0), 6)
    for i, (o, c) in enumerate(zip(opens, closes))
]
vols = [int(1000 + 500 * math.sin(i / 4.0) + (i % 7) * 137) for i in range(N)]

# ---------------------------------------------------------------- primitives
def sma(src, n):
    out = [float("nan")] * len(src)
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if all(math.isfinite(x) for x in w):
            out[i] = sum(w) / n
    return out

def rec_alpha(src, alpha, n, seed="sma"):
    out = [float("nan")] * len(src)
    prev = float("nan")
    fin = math.isfinite
    for i, cur in enumerate(src):
        if i == 0:
            if seed == "first" and fin(cur):
                out[i] = cur
            elif seed == "sma" and n == 1 and fin(cur):
                out[i] = cur
        elif (seed == "sma" and not fin(prev) and i + 1 >= n
              and all(fin(x) for x in src[i + 1 - n:i + 1])):
            out[i] = sum(src[i + 1 - n:i + 1]) / n  # lazy SMA seed
        elif fin(cur) and fin(prev):
            out[i] = alpha * cur + (1 - alpha) * prev
        elif seed == "first" and fin(cur) and not fin(prev):
            out[i] = cur
        else:
            out[i] = prev
        if fin(out[i]):
            prev = out[i]
    return out

def ema(src, n):
    return rec_alpha(src, 2.0 / (n + 1.0), n)

def rma(src, n):
    return rec_alpha(src, 1.0 / n, n)

def wma(src, n):
    out = [float("nan")] * len(src)
    den = n * (n + 1) / 2
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if all(math.isfinite(x) for x in w):
            out[i] = sum((j + 1) * x for j, x in enumerate(w)) / den
    return out

def hma(src, n):
    half = round(math.sqrt(n))
    short = wma(src, max(1, n // 2))  # Pine math.floor(length / 2)
    long = wma(src, n)
    d = [2 * s - l if math.isfinite(s) and math.isfinite(l) else float("nan")
         for s, l in zip(short, long)]
    return wma(d, max(1, half))

def dema(src, n):
    e, e2 = ema(src, n), None
    e2 = ema(e, n)
    return [2 * a - b if math.isfinite(a) and math.isfinite(b) else float("nan")
            for a, b in zip(e, e2)]

def tema(src, n):
    e1 = ema(src, n); e2 = ema(e1, n); e3 = ema(e2, n)
    return [3 * a - 3 * b + c
            if all(math.isfinite(x) for x in (a, b, c)) else float("nan")
            for a, b, c in zip(e1, e2, e3)]

def zlema(src, n):
    lag = (n - 1) // 2
    adj = [src[i] + (src[i] - src[i - lag])
           if i >= lag and math.isfinite(src[i]) and math.isfinite(src[i - lag])
           else float("nan") for i in range(len(src))]
    return ema(adj, n)

def alma(src, n, offset, sigma):
    m = offset * (n - 1)
    w = [math.exp(-((j - m) ** 2) / (2 * sigma * sigma)) for j in range(n)]
    sw = sum(w)
    out = [float("nan")] * len(src)
    for i in range(n - 1, len(src)):
        win = src[i + 1 - n:i + 1]
        if all(math.isfinite(x) for x in win):
            out[i] = sum(wj * x for wj, x in zip(w, win)) / sw
    return out

def lsma(src, n):
    out = [float("nan")] * len(src)
    sx = n * (n - 1) / 2
    sxx = (n - 1) * n * (2 * n - 1) / 6
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if all(math.isfinite(x) for x in w):
            sy, sxy = sum(w), sum(j * y for j, y in enumerate(w))
            den = n * sxx - sx * sx
            b = (n * sxy - sx * sy) / den
            a = (sy - b * sx) / n
            out[i] = a + b * (n - 1)
    return out

def vwma(src, v, n):
    out = [float("nan")] * len(src)
    for i in range(n - 1, len(src)):
        ws, wv = src[i + 1 - n:i + 1], v[i + 1 - n:i + 1]
        if all(math.isfinite(x) for x in ws):
            out[i] = sum(x * y for x, y in zip(ws, wv)) / sum(wv)
    return out

def kama(src, er_len, fast, slow):
    out = [float("nan")] * len(src)
    af, as_ = 2 / (fast + 1), 2 / (slow + 1)
    prev = float("nan")
    for i in range(er_len, len(src)):
        chg = abs(src[i] - src[i - er_len])
        vol = sum(abs(src[j] - src[j - 1]) for j in range(i + 1 - er_len, i + 1))
        er = chg / vol if vol != 0 else 0.0
        sc = (er * (af - as_) + as_) ** 2
        base = prev if math.isfinite(prev) else src[i - er_len]
        prev = base + sc * (src[i] - base)
        out[i] = prev
    return out

# ---------------------------------------------------------------- defaults
DOC_FALLBACK = {
    "Sma": {"length": 9},
    "Ema": {"length": 9},
    "Wma": {"length": 9},
    "Hma": {"length": 21},
    "Dema": {"length": 9},
    "Tema": {"length": 9},
    "Zlema": {"length": 9},
    "Rma": {"length": 9},
    "Vwma": {"length": 20},
    "Lsma": {"length": 9},  # no wire evidence; registry default kept (open question)
    "Alma": {"length": 9, "offset": 0.85, "sigma": 6},
    "Kama": {"er_len": 10, "fast": 2, "slow": 30},
}

def default_inputs(key):
    ev = json.loads(EVIDENCE.read_text()).get(key, {})
    ins = dict(DOC_FALLBACK[key])
    ins.update({k: v for k, v in ev.get("inputs", {}).items() if k in ins})
    return ins

def fmt(name, arr):
    cells = ", ".join("f64::NAN" if math.isnan(x) else repr(float(x)) for x in arr)
    return f"    pub(super) const {name}: [f64; {N}] = [{cells}];\n"
def main():
    out = ["// Generated by research/studies/gen_goldens.py — do not edit by hand.\n",
           "// Shared fixture2 plus moving-average full-series goldens.\n"]
    src = closes
    series = {
        "SMA": sma(src, default_inputs("Sma")["length"]),
        "EMA": ema(src, default_inputs("Ema")["length"]),
        "WMA": wma(src, default_inputs("Wma")["length"]),
        "HMA": hma(src, default_inputs("Hma")["length"]),
        "DEMA": dema(src, default_inputs("Dema")["length"]),
        "TEMA": tema(src, default_inputs("Tema")["length"]),
        "ZLEMA": zlema(src, default_inputs("Zlema")["length"]),
        "RMA": rma(src, default_inputs("Rma")["length"]),
        "VWMA": vwma(src, vols, default_inputs("Vwma")["length"]),
        "LSMA": lsma(src, default_inputs("Lsma")["length"]),
        "ALMA": alma(src, default_inputs("Alma")["length"],
                     default_inputs("Alma")["offset"], default_inputs("Alma")["sigma"]),
        "KAMA": kama(src, **default_inputs("Kama")),
    }
    for L in (8, 13, 21, 34, 55, 89, 144, 169):
        series[f"RIBBON_E{L}"] = ema(src, L)

    out.append(f"    pub(super) const FIX_T: [i64; {N}] = [{', '.join(map(str, t))}];\n")
    for name, arr in (("FIX_O", opens), ("FIX_H", highs), ("FIX_C", closes), ("FIX_L", lows)):
        out.append(f"    pub(super) const {name}: [f64; {N}] = [{', '.join(repr(float(x)) for x in arr)}];\n")
    out.append(f"    pub(super) const FIX_V: [f64; {N}] = [{', '.join(repr(float(x)) for x in vols)}];\n")
    for name, arr in series.items():
        out.append(fmt(name, arr))
    OUT.write_text("".join(out))
    print(f"wrote {OUT.name}: {len(series)} golden series, fixture {N} bars")

if __name__ == "__main__":
    main()
