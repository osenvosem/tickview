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


# ------------------------------------------------------------- overlays
FIN = math.isfinite


def tr(h, l, c):
    out = [h[0] - l[0]]
    for i in range(1, len(c)):
        out.append(max(h[i] - l[i], abs(h[i] - c[i - 1]), abs(l[i] - c[i - 1])))
    return out


def rolling_bound(src, n, mx):
    out = [float("nan")] * len(src)
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if all(FIN(x) for x in w):
            out[i] = max(w) if mx else min(w)
    return out


def stdev_pop(w):
    m = sum(w) / len(w)
    return math.sqrt(sum((x - m) ** 2 for x in w) / len(w))


def bollinger(src, n, mult):
    basis = sma(src, n)
    up = [float("nan")] * len(src)
    lo = [float("nan")] * len(src)
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if all(FIN(x) for x in w):
            sd = stdev_pop(w)
            up[i] = basis[i] + mult * sd
            lo[i] = basis[i] - mult * sd
    return up, basis, lo


def keltner(h, l, c, n, mult):
    mid = ema(c, n)
    ra = rma(tr(h, l, c), n)
    up = [float("nan")] * len(c)
    lo = [float("nan")] * len(c)
    for i in range(len(c)):
        if FIN(mid[i]) and FIN(ra[i]):
            up[i] = mid[i] + mult * ra[i]
            lo[i] = mid[i] - mult * ra[i]
    return up, mid, lo


def donchian(h, l, n):
    up = rolling_bound(h, n, True)
    lo = rolling_bound(l, n, False)
    med = [(u + d) / 2 if FIN(u) and FIN(d) else float("nan") for u, d in zip(up, lo)]
    return up, med, lo


def envelope(src, n, pct):
    basis = sma(src, n)
    up = [b * (1 + pct / 100) if FIN(b) else float("nan") for b in basis]
    lo = [b * (1 - pct / 100) if FIN(b) else float("nan") for b in basis]
    return up, basis, lo


def linreg_channel(src, n, mult):
    out_c = [float("nan")] * len(src)
    out_u = [float("nan")] * len(src)
    out_l = [float("nan")] * len(src)
    if n < 2:
        return out_c, out_u, out_l
    mx = (n - 1) / 2.0
    sxx = sum((k - mx) ** 2 for k in range(n))
    for i in range(n - 1, len(src)):
        w = src[i + 1 - n:i + 1]
        if not all(FIN(x) for x in w):
            continue
        my = sum(w) / n
        sxy = sum((k - mx) * (w[k] - my) for k in range(n))
        slope = sxy / sxx
        icpt = my - slope * mx
        last = icpt + slope * (n - 1)
        sd = math.sqrt(sum((w[k] - (icpt + slope * k)) ** 2 for k in range(n)) / n)
        out_c[i] = last
        out_u[i] = last + mult * sd
        out_l[i] = last - mult * sd
    return out_c, out_u, out_l


def psar(h, l, start, inc, max_af):
    n = len(h)
    out = [float("nan")] * n
    if n < 2:
        return out
    bull, af, ep, sar = True, start, h[0], l[0]
    for i in range(1, n):
        s = sar + af * (ep - sar)
        if bull:
            s = min(s, l[i - 1], l[i - 2] if i >= 2 else s)
            if l[i] < s:
                bull, s, ep, af = False, ep, l[i], start
            elif h[i] > ep:
                ep = h[i]
                af = min(af + inc, max_af)
        else:
            s = max(s, h[i - 1], h[i - 2] if i >= 2 else s)
            if h[i] > s:
                bull, s, ep, af = True, ep, h[i], start
            elif l[i] < ep:
                ep = l[i]
                af = min(af + inc, max_af)
        out[i] = s
        sar = s
    return out


def supertrend(h, l, c, atr_len, mult):
    n = len(c)
    atr = rma(tr(h, l, c), atr_len)
    line = [float("nan")] * n
    trend = [float("nan")] * n
    fub = [float("nan")] * n
    flb = [float("nan")] * n
    prev_t = 1.0
    for i in range(n):
        if not FIN(atr[i]):
            continue
        hl2 = (h[i] + l[i]) / 2
        ub, lb = hl2 + mult * atr[i], hl2 - mult * atr[i]
        fub[i] = ub if (i == 0 or not FIN(fub[i - 1]) or ub < fub[i - 1]
                        or c[i - 1] > fub[i - 1]) else fub[i - 1]
        flb[i] = lb if (i == 0 or not FIN(flb[i - 1]) or lb > flb[i - 1]
                        or c[i - 1] < flb[i - 1]) else flb[i - 1]
        if prev_t > 0 and c[i] <= flb[i]:
            t = -1.0
        elif prev_t < 0 and c[i] >= fub[i]:
            t = 1.0
        else:
            t = prev_t
        line[i] = flb[i] if t > 0 else fub[i]
        trend[i] = t
        prev_t = t
    return line, trend


def alligator(h, l, jb, js, tb, ts, lb, ls):
    hl2 = [(a + b) / 2 for a, b in zip(h, l)]

    def sh(arr, k):
        return [float("nan")] * k + arr[:len(arr) - k]
    return (sh(rma(hl2, jb), js), sh(rma(hl2, tb), ts), sh(rma(hl2, lb), ls))


def ichimoku(h, l, conv, base, sb, disp):
    n = len(h)
    ch, cl = rolling_bound(h, conv, True), rolling_bound(l, conv, False)
    bh, bl = rolling_bound(h, base, True), rolling_bound(l, base, False)
    sh, sl = rolling_bound(h, sb, True), rolling_bound(l, sb, False)
    cv = [(a + b) / 2 if FIN(a) and FIN(b) else float("nan") for a, b in zip(ch, cl)]
    bs = [(a + b) / 2 if FIN(a) and FIN(b) else float("nan") for a, b in zip(bh, bl)]
    sa = [float("nan")] * n
    sbv = [float("nan")] * n
    for i in range(disp, n):
        j = i - disp
        if FIN(cv[j]) and FIN(bs[j]):
            sa[i] = (cv[j] + bs[j]) / 2
        if FIN(sh[i]) and FIN(sl[i]):
            sbv[i] = (sh[i] + sl[i]) / 2
    return cv, bs, sa, sbv
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

    ev = json.loads(EVIDENCE.read_text())

    def w(sid, key, fb):
        return ev.get(sid, {}).get("inputs", {}).get(key, fb)

    bb_u, bb_b, bb_l = bollinger(src, w("BollingerBands", "length", 20),
                                 w("BollingerBands", "stdDev", 2.0))
    ku, kb, kl = keltner(highs, lows, closes, w("KeltnerChannels", "length", 20),
                         w("KeltnerChannels", "multiplier", 1.0))
    do_u, do_m, do_l = donchian(highs, lows, w("DonchianChannels", "length", 20))
    en_u, en_b, en_l = envelope(src, w("Envelope", "length", 20),
                                w("Envelope", "percent", 10.0))
    lr_c, lr_u, lr_l = linreg_channel(src, w("LinReg", "count", 100),
                                      abs(w("LinReg", "upper diviation", 2.0)))
    ps = psar(highs, lows, w("Psar", "start", 0.02), w("Psar", "increment", 0.02),
              w("Psar", "max value", 0.2))
    st_line, st_trend = supertrend(highs, lows, closes, 10, 3.0)  # doc default, no wire
    ag = alligator(highs, lows, w("Alligator", "jaw length", 13), 8,
                   w("Alligator", "teeth length", 8), 3,
                   w("Alligator", "lips length", 5), 1)
    ic = ichimoku(highs, lows, w("Ichimoku", "turning line periods", 9),
                  w("Ichimoku", "standard line periods", 26),
                  w("Ichimoku", "leading span 2 periods", 52),
                  w("Ichimoku", "displacement", 26))
    series.update({
        "BB_U": bb_u, "BB_B": bb_b, "BB_L": bb_l,
        "KELT_U": ku, "KELT_B": kb, "KELT_L": kl,
        "DON_U": do_u, "DON_M": do_m, "DON_L": do_l,
        "ENV_U": en_u, "ENV_B": en_b, "ENV_L": en_l,
        "LR_C": lr_c, "LR_U": lr_u, "LR_L": lr_l,
        "PSAR": ps, "ST_UP": [x if tt > 0 else float("nan") for x, tt in zip(st_line, st_trend)],
        "ST_DN": [x if tt < 0 else float("nan") for x, tt in zip(st_line, st_trend)],
        "AG_JAW": ag[0], "AG_TEETH": ag[1], "AG_LIPS": ag[2],
        "ICHI_C": ic[0], "ICHI_B": ic[1], "ICHI_SA": ic[2], "ICHI_SB": ic[3],
    })

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
