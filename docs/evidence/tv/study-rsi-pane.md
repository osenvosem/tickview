# RSI pane: guides, band, zone gradients, MA wire, settings groups

Source of truth: `docs/evidence/tv/studies/STD_RSI.json` (`defaults.bands`,
`defaults.filledAreasStyle`, `filledAreas`, `inputs`, `styles`, `plots`) from
TradingView's `all_studies_metadata` payload for `STD;RSI`. Screenshots in
`docs/screenshots/parity/rsi-pane-{dark,light}.png` and
`rsi-editor-{dark,light}.png` are 2880×1800 captures of this app.

## Pane decoration

| TV key | Value | Ours |
|---|---|---|
| `defaults.bands[0]` | 70, `#787B86`, linestyle 2 | `GuideSpec { value: 70, color: GRAY, dash: true }` |
| `defaults.bands[1]` | 50, `rgba(120,123,134,0.5)` | `GuideSpec { value: 50, alpha: 128, dash: true }` |
| `defaults.bands[2]` | 30, `#787B86`, linestyle 2 | `GuideSpec { value: 30, color: GRAY, dash: true }` |
| `filledAreas[0]` `fill_0` | `hline_0`..`hline_2`, `rgba(126,87,194,0.1)` | `FillSpec::Band { lo: 30, hi: 70, color: PURPLE, alpha: 26 }` |
| `fill_1` | gradient, `plot_0`..`plot_1`, 70→100, `rgba(76,175,80,0)`→`rgba(76,175,80,1)` | `FillSpec::Gradient { plot: 0, near: 70, far: 100, color: GREEN, alpha: 255 }` |
| `fill_2` | gradient, `plot_0`..`plot_1`, 0→30, `rgba(242,54,69,1)`→`rgba(242,54,69,0)` | `FillSpec::Gradient { plot: 0, near: 30, far: 0, color: CRIMSON, alpha: 255 }` |
| `fill_3` (+`plot_3`/`plot_4`) | Bollinger band fill, hidden unless the Bollinger smoothing variant is on | the two Bollinger plots exist only for `ma_type = 2`; no separate fill spec |

TV anchors the zone gradients between `plot_0` (RSI) and a hidden `plot_1`
pinned at 70/0; we keep the thresholds as the `near`/`far` constants instead of
materialising a constant series — the visible result is the same, and the
gradient still has to follow the RSI line, not the threshold.

The ramp is per-column: alpha `0` at `near`, `alpha` at `far`, linear in
between (`decor::gradient_alpha`), so at the pane's usual ~80 ceiling the
overbought wash tops out around 33% — TV's subtlety, not a solid slab. A run
breaks on `NaN` or on a value that leaves `[near, far]`, so the wash never bleeds
through the neutral zone.

## Plots and inputs

- `plots`: `plot_0` RSI, `plot_1` hidden threshold anchor, `plot_2` the moving
  average of RSI (`styles.plot_2.display` is patched by the smoothing type),
  `plot_3`/`plot_4` Bollinger bands, plus divergence shapes we do not port.
- `inputs`: `ma_len` (`in_4`) is active while `maTypeInput != 'None'`, and
  `bb_std` (`in_5`) while `isBB` (`maTypeInput == 'SMA + Bollinger Bands'`) —
  `rm_1`/`rm_2` in `TVScriptMetaInfoExprs`. Modelled as
  `ParamSpec { group, legend, active }` with `Active::NotVariant` and
  `Active::IsVariant`, rendered as greyed-but-present controls.
- `legend: false` on `ma_type`/`bb_std`/timeframe inputs reproduces TV's legend
  summary `14 close`, which omits the smoothing type and multiplier.

## Paint path

`crates/tickchart/src/decor.rs` turns a `PaneDecor` into shapes;
`chart.rs` calls `decor::draw` per visible study layer before its plots, clipped
to the pane rect.

Pitfall found while verifying the light theme: egui stores mesh vertex colours
**pre-multiplied**. Building the gradient corners with
`Color32::from_rgba_premultiplied(r, g, b, ramp)` left full-strength channels
next to a small alpha, which blends to white on the light background (wash
invisible) and to a near-solid slab on the dark one. Corners now use
`from_rgba_unmultiplied`; `decor::tests::gradient_vertices_premultiply_the_ramp_alpha`
pins it.

## Verification

- `cargo test -p ta` — `default_params_plot_rsi_and_a_yellow_sma_of_it`,
  `smoothing_menu_selects_the_average_of_the_line` and
  `bollinger_variant_wraps_the_smoothed_line` pin the plot count and which
  smoothing type yields which series; `decor_carries_the_captured_guides_and_fills`
  asserts the table above, while `other_studies_get_plain_dashed_guides` keeps
  every non-RSI study at plain `hline` guides with no fills.
- `cargo test -p tickchart decor` — alpha ramp in both directions, run splitting
  on `NaN`/threshold exits, `gradient_shape_ignores_runs_outside_the_visible_window`
  (regression for the slice-order panic that `tests/hover_future.rs` tripped),
  and vertex premultiplication.
- `cargo test -p app study_toolbar` — `titled_wraps_the_inputs_and_falls_back_to_the_name`
  keeps the legend summary in TV's `14 close` shape.
- Screenshots: `rsi-pane-{dark,light}.png` show the guides, the 30–70 band and
  both zone washes; `rsi-editor-{dark,light}.png` show the `RSI SETTINGS` /
  `SMOOTHING` sections with `BB StdDev` greyed out while `Type = SMA`.
