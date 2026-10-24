//! Regression: hovering into the empty "future" space right of the last bar
//! must not panic. `index_at` intentionally yields indices past the data (TV
//! parity), so every consumer has to treat them as "no value".

use egui::{Context, Event, Pos2, Rect, Vec2};
use ta::registry::{Anchors, Params, def};
use ta::{Bar, Candles, TvId};
use tickchart::chart::{self, ChartState};

const W: f32 = 900.0;
const H: f32 = 700.0;
const N: usize = 500;

fn fixture() -> Candles {
    let bars: Vec<Bar> = (0..N)
        .map(|i| {
            let base = 100.0 + 0.4 * i as f64 + 3.0 * (i as f64 * 0.35).sin();
            Bar {
                t: i as i64 * 3600,
                o: base,
                h: base + 1.5,
                l: base - 1.5,
                c: base + 0.5,
                v: 1000.0,
            }
        })
        .collect();
    Candles::from_bars(&bars)
}

fn run_frames(st: &mut ChartState, cd: &Candles, far_right: Pos2, frames: usize) {
    let ctx = Context::default();
    let mut t = 0.0;
    for k in 0..frames {
        t += 0.016;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(W, H))),
            time: Some(t),
            events: vec![Event::PointerMoved(far_right)],
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let _ = chart::ui(ui, st, cd);
        });
        out.textures_delta.clear();
        let _ = k;
    }
}

#[test]
fn hover_past_last_bar_with_indicator_panes_is_safe() {
    let cd = fixture();
    let mut st = ChartState::new("TEST", "1H");
    // First frame adopts the real view width/count.
    run_frames(&mut st, &cd, Pos2::new(W / 2.0, H / 2.0), 1);
    let ax = Anchors::from_ts(&cd.t);
    st.add_indicator(&cd, &ax, TvId::Rsi, Params::with_defaults(def(TvId::Rsi)));
    st.add_indicator(&cd, &ax, TvId::Macd, Params::with_defaults(def(TvId::Macd)));

    // Empty future space on the right edge, pointer resting long enough to
    // also trip the long-press values tooltip.
    st.view.right_offset = 300.0;
    run_frames(&mut st, &cd, Pos2::new(W - 10.0, 120.0), 40);

    // Scrolled fully past the data.
    st.view.right_offset = 4000.0;
    run_frames(&mut st, &cd, Pos2::new(W - 10.0, 120.0), 2);
    let _ = Vec2::ZERO;
}
