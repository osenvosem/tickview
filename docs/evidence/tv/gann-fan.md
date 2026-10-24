# Gann fan parity evidence

- Target: TradingView Desktop 3.4.1.
- Ratio labels: `1/8 1/4 1/3 1/2 1/1 2/1 3/1 4/1 8/1`.
- The `1/1` ray is the drag line between the two placed anchors.
- Rays extend forward in the drag direction; the opposite direction is not drawn.
- Price slope formula: `r × (p1 − p0) / (b1 − b0)`, where `r` is the ray ratio,
  `p0` is the origin price, `p1` is the drag-line end price, and `b1 − b0` is
  the drag width in bars.
- Computed prices are mapped through `PriceAxis`, including log scale.
- Official help reference:
  <https://www.tradingview.com/support/solutions/43000518151-gann-fan-drawing-tool/>
- Live-TV screenshot: `docs/screenshots/tv/gann-fan.png` — still pending; it
  requires a manual capture from the TradingView desktop app. The in-app proof
  for this slice is `docs/screenshots/parity/gann-fan-dark.png`
  (`TICKVIEW_SCENE=gannf TICKVIEW_THEME=dark`).
- Regression tests:
  - `drawings::tests::gann_price_uses_tv_ratios`
  - `drawings::tests::gann_fan_has_nine_tv_rays_and_labels`
  - `chart::tests::gann_fan_hit_tests_follow_ratio_rays`
- Known limitation: labels are nudged downward to avoid overlap; this changes
  label placement only and does not affect ray geometry.
