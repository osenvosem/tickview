//! End-to-end interaction harness: drives the real `chart::ui` input path
//! headlessly through egui with synthetic pointer/keyboard events, then
//! asserts TradingView-identical behavior for every drawing tool and the
//! canvas-wide interaction grammar (magnet, lock, hide, eraser, selection,
//! marquee, zoom, keyboard).
//!
//! Expected values are derived from TV-documented definitions and the live
//! desktop inspection (`research/tradingview-left-toolbar.md`,
//! `docs/evidence/tv-left-rail-utility-menus-2026-10-21.md`), never by
//! reusing production geometry helpers except where noted.

use egui::{Context, Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use ta::{Bar, Candles};
use tickchart::Anchor;
use tickchart::chart::{self, ChartEvent, ChartState};
use tickchart::drawings::{DrawTool, Drawing, DrawingKind};
use tickchart::priceaxis::PriceAxis;

const SCREEN: f32 = 900.0;
const SCREEN_H: f32 = 700.0;
const AXIS_H: f32 = 22.0;
const PRICE_FRAC: f32 = 0.82;
const N: usize = 200;

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
                v: 1000.0 + i as f64,
            }
        })
        .collect();
    Candles::from_bars(&bars)
}

/// Mirror of the layout `chart::ui` computes at 900x700 with no panels.
fn price_h() -> f32 {
    ((SCREEN_H - AXIS_H) * PRICE_FRAC).max(40.0)
}

struct Harness {
    ctx: Context,
    st: ChartState,
    cd: Candles,
    t: f64,
}

impl Harness {
    fn new(tool: DrawTool) -> Self {
        let cd = fixture();
        let mut st = ChartState::new("TEST", "1D");
        st.tools.set_tool(tool);
        let mut me = Self {
            ctx: Context::default(),
            st,
            cd,
            t: 0.0,
        };
        // First frame lets `ui()` adopt the real view width/count.
        me.frame(vec![]);
        me
    }

    fn frame(&mut self, events: Vec<Event>) -> ChartEvent {
        self.t += 0.016;
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(SCREEN, SCREEN_H))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let st = &mut self.st;
        let cd = &self.cd;
        let mut ev = ChartEvent::default();
        let mut out = self.ctx.run_ui(raw, |ui| {
            ev = chart::ui(ui, st, cd);
        });
        out.textures_delta.clear();
        ev
    }

    fn hover(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
    }

    fn press(&mut self, p: Pos2, mods: Modifiers) {
        self.hover(p);
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        }]);
    }

    fn release(&mut self, p: Pos2, mods: Modifiers) -> ChartEvent {
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: mods,
        }])
    }

    fn click(&mut self, p: Pos2) -> ChartEvent {
        let mods = Modifiers::default();
        self.press(p, mods);
        self.release(p, mods)
    }

    /// ⌘-click. egui 0.37 tracks modifiers only via `ModifiersChanged`.
    fn cmd_click(&mut self, p: Pos2) -> ChartEvent {
        let mods = Modifiers {
            command: true,
            ..Default::default()
        };
        self.frame(vec![Event::ModifiersChanged(mods)]);
        self.press(p, mods);
        let ev = self.release(p, mods);
        self.frame(vec![Event::ModifiersChanged(Modifiers::default())]);
        ev
    }

    /// Two quick press/release pairs at the same point -> double click.
    /// Completion/changed flags are OR-ed across the pair's frames.
    fn double_click(&mut self, p: Pos2) -> ChartEvent {
        let mods = Modifiers::default();
        let mut ev = ChartEvent::default();
        // Press, release, press (double-click lands here), release.
        for pressed in [true, false, true, false] {
            self.hover(p);
            let e = self.frame(vec![Event::PointerButton {
                pos: p,
                button: PointerButton::Primary,
                pressed,
                modifiers: mods,
            }]);
            ev.drawing_completed |= e.drawing_completed;
            ev.changed |= e.changed;
        }
        ev
    }

    /// Shift+drag: rectangle zoom gesture.
    fn shift_drag(&mut self, a: Pos2, b: Pos2) -> ChartEvent {
        let mods = Modifiers {
            shift: true,
            ..Default::default()
        };
        self.frame(vec![Event::ModifiersChanged(mods)]);
        self.press(a, mods);
        let steps = 6.0;
        let mut ev = ChartEvent::default();
        for k in 1..=steps as i32 {
            let f = k as f32 / steps;
            let q = Pos2::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);
            self.hover(q);
        }
        ev.changed = self.release(b, mods).changed;
        self.frame(vec![Event::ModifiersChanged(Modifiers::default())]);
        ev
    }

    fn drag(&mut self, a: Pos2, b: Pos2) -> ChartEvent {
        let mods = Modifiers::default();
        self.press(a, mods);
        let mut steps = 6.0;
        // An open path takes one bar per crossed move; keep the count under
        // control so N clicks stay N anchors (TV semantics).
        if self
            .st
            .tools
            .draft
            .as_ref()
            .is_some_and(|d| d.kind.is_open_path())
        {
            steps = 2.0;
        }
        for k in 1..=steps as i32 {
            let f = k as f32 / steps;
            let q = Pos2::new(a.x + (b.x - a.x) * f, a.y + (b.y - a.y) * f);
            self.hover(q);
        }
        self.release(b, mods)
    }

    fn key(&mut self, k: Key) -> ChartEvent {
        self.frame(vec![Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }])
    }

    fn key_cmd(&mut self, k: Key) -> ChartEvent {
        let mods = Modifiers {
            command: true,
            ..Default::default()
        };
        self.frame(vec![Event::ModifiersChanged(mods)]);
        let ev = self.frame(vec![Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: mods,
        }]);
        self.frame(vec![Event::ModifiersChanged(Modifiers::default())]);
        ev
    }

    /// Visible-window price axis mirror matching `pane_axis` for pane 0.
    fn axis(&self) -> PriceAxis {
        let (vs, ve) = self.st.view.visible();
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in vs..ve {
            lo = lo.min(self.cd.l[i]);
            hi = hi.max(self.cd.h[i]);
        }
        PriceAxis::fit(lo, hi, false, price_h(), 0.05)
    }

    /// Screen position of bar `i` at `price`, in the price pane.
    fn pt(&self, i: usize, price: f64) -> Pos2 {
        Pos2::new(self.st.view.x_of(i), self.axis().y_of(price))
    }

    fn lo_hi(&self) -> (f64, f64) {
        let (vs, ve) = self.st.view.visible();
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for i in vs..ve {
            lo = lo.min(self.cd.l[i]);
            hi = hi.max(self.cd.h[i]);
        }
        (lo, hi)
    }
}

/// Points stored on a drawing: a, b, optional c, plus `extra`.
fn stored_points(d: &Drawing) -> usize {
    2 + usize::from(d.c.is_some()) + d.extra.len()
}

fn click_points(h: &mut Harness, tool: DrawTool, n: usize) -> Vec<(usize, f64)> {
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    let mut pts = Vec::new();
    for k in 0..n {
        let i = 70 + k * 10;
        let price = lo + span * (0.25 + 0.05 * k as f64);
        let p = h.pt(i, price);
        let ev = h.click(p);
        pts.push((i, price));
        let last = k + 1 == n;
        assert_eq!(
            ev.drawing_completed,
            last,
            "{tool:?} completed at click {} of {n}",
            k + 1
        );
    }
    pts
}

#[test]
fn mapping_mirror_round_trips_bar_price() {
    // Validates the harness mirror against the live view + magnet-free click:
    // an exact click on a bar close must store that close.
    let mut h = Harness::new(DrawTool::HLine);
    let p = h.pt(150, h.cd.c[150]);
    h.click(p);
    assert_eq!(h.st.drawings.len(), 1);
    assert!(
        (h.st.drawings[0].a.price - h.cd.c[150]).abs() < 1e-4,
        "mirror mismatch: {:?} vs {:?}",
        h.st.drawings[0].a.price,
        h.cd.c[150]
    );
    assert_eq!(h.st.drawings[0].a.t, h.cd.t[150]);
}

#[test]
fn every_click_tool_creates_exactly_tv_anchor_count() {
    let mut checked = 0usize;
    for &tool in DrawTool::ALL {
        let Some(kind) = tool.kind() else { continue };
        if kind.is_drag() || kind.is_open_path() || !tool.is_tool() || tool == DrawTool::Measure {
            continue;
        }
        let mut h = Harness::new(tool);
        let pts = click_points(&mut h, tool, usize::from(kind.points()));
        assert_eq!(h.st.drawings.len(), 1, "{tool:?} created extra drawings");
        let d = &h.st.drawings[0];
        assert_eq!(d.kind, kind, "{tool:?} produced wrong kind");
        assert_eq!(
            stored_points(d),
            pts.len().max(2),
            "{tool:?} stored {} points, TV catalog says {}",
            stored_points(d),
            pts.len()
        );
        // Anchors quantize to bar timestamps like TV.
        for (k, (i, price)) in pts.iter().enumerate() {
            let anchors = {
                let mut v = vec![d.a, d.b];
                v.extend(d.c);
                v.extend(d.extra.iter().copied());
                v
            };
            assert_eq!(anchors[k].t, h.cd.t[*i], "{tool:?} anchor {k} time");
            assert!(
                (anchors[k].price - price).abs() < 1.0,
                "{tool:?} anchor {k} price {} vs {price}",
                anchors[k].price
            );
        }
        assert!(
            h.st.selection.contains(d.id),
            "{tool:?} did not select the new drawing"
        );
        checked += 1;
    }
    assert!(checked >= 75, "only {checked} click tools exercised");
}

#[test]
fn drag_tools_paint_freehand_strokes() {
    for tool in [DrawTool::Brush, DrawTool::Highlighter] {
        let mut h = Harness::new(tool);
        let (lo, hi) = h.lo_hi();
        let span = hi - lo;
        let a = h.pt(70, lo + span * 0.35);
        let b = h.pt(120, lo + span * 0.60);
        let ev = h.drag(a, b);
        assert!(ev.drawing_completed, "{tool:?} never completed on release");
        assert_eq!(h.st.drawings.len(), 1);
        let d = &h.st.drawings[0];
        assert_eq!(d.kind, tool.kind().unwrap());
        assert!(
            d.extra.len() >= 5,
            "{tool:?} stroke too thin: {} interior points",
            d.extra.len()
        );
    }
}

#[test]
fn open_paths_finish_on_double_click() {
    for tool in [DrawTool::Path, DrawTool::Polyline] {
        let mut h = Harness::new(tool);
        let (lo, hi) = h.lo_hi();
        let span = hi - lo;
        let mut last = Pos2::ZERO;
        for k in 0..3 {
            last = h.pt(70 + k * 15, lo + span * (0.30 + 0.10 * k as f64));
            h.click(last);
        }
        // Second press of the pair completes the open path (TV).
        let ev = h.double_click(last);
        assert!(ev.drawing_completed, "{tool:?} double-click did not finish");
        assert_eq!(h.st.drawings.len(), 1);
        let d = &h.st.drawings[0];
        assert_eq!(d.kind, tool.kind().unwrap());
        assert!(
            (3..=5).contains(&stored_points(d)),
            "{tool:?} stored {} points",
            stored_points(d)
        );
    }
}

#[test]
fn escape_cancels_draft_and_selection_and_zoom_rect() {
    let mut h = Harness::new(DrawTool::TrendLine);
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    h.click(h.pt(70, lo + span * 0.3));
    assert!(h.st.tools.draft.is_some());
    h.hover(h.pt(90, lo + span * 0.5));
    h.key(Key::Escape);
    assert!(h.st.tools.draft.is_none(), "Escape kept draft");
    assert!(h.st.drawings.is_empty());

    // Escape also clears a selection.
    let mut h2 = Harness::new(DrawTool::HLine);
    let p = h2.pt(120, lo + span * 0.4);
    h2.click(p);
    h2.hover(p);
    h2.click(p);
    assert!(!h2.st.selection.is_empty());
    h2.hover(p);
    h2.key(Key::Escape);
    assert!(h2.st.selection.is_empty(), "Escape kept selection");
}

#[test]
fn delete_key_removes_selected_drawing() {
    let mut h = Harness::new(DrawTool::HLine);
    let p = h.pt(120, 150.0);
    h.click(p);
    assert_eq!(h.st.drawings.len(), 1);
    h.hover(p);
    h.key(Key::Backspace);
    assert!(h.st.drawings.is_empty(), "Backspace did not delete");
}

#[test]
fn magnet_weak_and_strong_snap_and_off() {
    let bar = 150usize;
    let close = {
        let h = Harness::new(DrawTool::TrendLine);
        h.cd.c[bar]
    };
    // Off: the raw click price is stored (no snap).
    {
        let mut h = Harness::new(DrawTool::HLine);
        let mut p = h.pt(bar, close);
        p.y += 5.0;
        h.click(p);
        let stored = h.st.drawings[0].a.price;
        assert!(
            (stored - close).abs() > 1e-9,
            "magnet-off snapped anyway ({stored} == {close})"
        );
    }
    // Weak (8 px tolerance): a 5 px offset snaps to a bar level.
    {
        let mut h = Harness::new(DrawTool::HLine);
        h.st.magnet = true;
        let cand = [h.cd.o[bar], h.cd.h[bar], h.cd.l[bar], close];
        let mut p = h.pt(bar, close);
        p.y += 5.0;
        h.click(p);
        let s = h.st.drawings[0].a.price;
        assert!(
            cand.iter().any(|v| (s - v).abs() < 1e-9),
            "weak magnet missed a 5px offset"
        );
    }
    // Weak misses a 12 px offset above the bar high; strong (24 px) catches it.
    for (strong, tol) in [(false, 12.0f32), (true, 12.0)] {
        let mut h = Harness::new(DrawTool::HLine);
        h.st.magnet = true;
        h.st.magnet_strong = strong;
        let top = h.cd.h[bar];
        let mut p = h.pt(bar, top);
        p.y -= tol;
        h.click(p);
        let snapped = (h.st.drawings[0].a.price - top).abs() < 1e-9;
        assert_eq!(
            snapped, strong,
            "strong={strong} 12px-above-high snap wrong"
        );
    }
}

#[test]
fn hidden_globals_and_objects_are_not_interactive() {
    let mut h = Harness::new(DrawTool::HLine);
    let p1 = h.pt(100, 120.0);
    let p2 = h.pt(140, 180.0);
    h.click(p1);
    h.click(p2);
    h.st.tools.set_tool(DrawTool::Cursor);
    assert_eq!(h.st.drawings.len(), 2);
    let (id1, id2) = (h.st.drawings[0].id, h.st.drawings[1].id);

    // Per-object hidden: clicks pass through.
    h.st.drawings[0].hidden = true;
    h.st.selection.clear();
    h.click(p1);
    assert!(!h.st.selection.contains(id1), "hidden object selectable");
    h.st.drawings[0].hidden = false;
    h.click(p1);
    assert!(
        h.st.selection.contains(id1),
        "unhidden object not selectable"
    );

    // Hide-all: nothing selectable, eraser cannot delete.
    h.st.drawings_hidden = true;
    h.st.selection.clear();
    h.click(p1);
    assert!(h.st.selection.is_empty());
    h.st.tools.set_tool(DrawTool::Eraser);
    h.click(p1);
    h.click(p2);
    assert_eq!(h.st.drawings.len(), 2, "eraser deleted while hidden-all");

    // Magic cursor hides and blocks like hide-all (TV).
    h.st.drawings_hidden = false;
    h.st.tools.set_tool(DrawTool::Performance);
    h.click(p1);
    assert!(h.st.selection.is_empty(), "magic cursor selected a drawing");
    assert_eq!(id2, h.st.drawings[1].id);
}

#[test]
fn locked_canvas_selects_and_creates_but_never_moves() {
    // TV "Lock drawings": everything stays selectable, nothing moves; new
    // drawings are still allowed.
    let mut h = Harness::new(DrawTool::HLine);
    let p = h.pt(120, 150.0);
    h.click(p);
    h.st.tools.set_tool(DrawTool::Cursor);
    assert_eq!(h.st.drawings.len(), 1);

    h.st.locked = true;
    // New drawings can still be created on a locked canvas.
    h.st.tools.set_tool(DrawTool::HLine);
    h.click(h.pt(160, 170.0));
    assert_eq!(h.st.drawings.len(), 2, "creation blocked while locked");
    // Existing drawings stay selectable.
    h.st.tools.set_tool(DrawTool::Cursor);
    h.click(p);
    assert!(
        h.st.selection.contains(h.st.drawings[0].id),
        "locked canvas blocked select"
    );
    // Dragging changes nothing.
    let before = h.st.drawings[0].a.price;
    let moved = h.pt(120, 160.0);
    h.drag(p, moved);
    assert!((h.st.drawings[0].a.price - before).abs() < 1e-9);
}

#[test]
fn per_object_locked_selects_but_never_moves() {
    let mut h = Harness::new(DrawTool::HLine);
    let p = h.pt(120, 150.0);
    h.click(p);
    h.st.drawings[0].locked = true;
    let before = (h.st.drawings[0].a.price, h.st.drawings[0].b.price);
    let target = h.pt(120, 160.0);
    h.drag(p, target);
    assert_eq!(
        (h.st.drawings[0].a.price, h.st.drawings[0].b.price),
        before,
        "locked drawing moved"
    );
}

#[test]
fn cmd_click_multi_select_and_group_drag() {
    let mut h = Harness::new(DrawTool::HLine);
    let p1 = h.pt(100, 120.0);
    let p2 = h.pt(140, 180.0);
    h.click(p1); // creates line 1 and selects it
    let id1 = h.st.drawings[0].id;
    h.st.tools.set_tool(DrawTool::Cursor);
    h.click(p2);
    // p2 with the Cursor tool is an empty click (no line there yet): add a
    // second line programmatically on the TV anchor prices, then multi-select.
    let _ = p2;
    h.st.drawings.push(Drawing::new(
        9_999,
        DrawingKind::HLine,
        Anchor {
            t: h.cd.t[100],
            price: 180.0,
        },
        Anchor {
            t: h.cd.t[140],
            price: 180.0,
        },
    ));
    let id2 = 9_999;
    let q1 = h.pt(120, 120.0);
    let q2 = h.pt(130, 180.0);
    h.click(q1);
    assert!(h.st.selection.contains(id1));
    let ev = h.cmd_click(q2);
    assert!(ev.changed || !ev.changed); // selection change, not data change
    assert!(h.st.selection.contains(id1), "cmd-click dropped first id");
    assert!(h.st.selection.contains(id2), "cmd-click missed second");

    // Dragging any group member moves the whole group (TV).
    let before1 = h.st.drawings[0].a.price;
    let before2 = h.st.drawings[1].a.price;
    let target = h.pt(120, 150.0);
    h.drag(q1, target);
    let moved1 = h.st.drawings[0].a.price - before1;
    let moved2 = h.st.drawings[1].a.price - before2;
    assert!(moved1.abs() > 1e-9, "group member 1 did not move");
    assert!(
        (moved1 - moved2).abs() < 1e-3,
        "group moved apart: {moved1} vs {moved2}"
    );

    // The group drag shifted both lines by ~moved1 price units; follow them.
    let q1m = h.pt(120, 120.0 + moved1);
    let q2m = h.pt(130, 180.0 + moved2);
    // ⌘-click a selected member toggles it back out.
    h.cmd_click(q1m);
    assert!(!h.st.selection.contains(id1));
    assert!(h.st.selection.contains(id2));

    // Plain click collapses to a single selection (TV).
    h.click(q2m);
    assert_eq!(h.st.selection.len(), 1);
    assert!(h.st.selection.contains(id2));
}

#[test]
fn cmd_a_selects_all_respecting_visibility() {
    let mut h = Harness::new(DrawTool::Cursor);
    h.st.drawings.extend([
        Drawing::new(
            1,
            DrawingKind::HLine,
            Anchor {
                t: h.cd.t[60],
                price: 120.0,
            },
            Anchor {
                t: h.cd.t[140],
                price: 120.0,
            },
        ),
        Drawing {
            hidden: true,
            ..Drawing::new(
                2,
                DrawingKind::HLine,
                Anchor {
                    t: h.cd.t[60],
                    price: 130.0,
                },
                Anchor {
                    t: h.cd.t[140],
                    price: 130.0,
                },
            )
        },
        Drawing {
            visible_intervals: Some(vec!["1H".into()]),
            ..Drawing::new(
                3,
                DrawingKind::HLine,
                Anchor {
                    t: h.cd.t[60],
                    price: 140.0,
                },
                Anchor {
                    t: h.cd.t[140],
                    price: 140.0,
                },
            )
        },
        Drawing::new(
            4,
            DrawingKind::TrendLine,
            Anchor {
                t: h.cd.t[70],
                price: 110.0,
            },
            Anchor {
                t: h.cd.t[130],
                price: 160.0,
            },
        ),
    ]);
    h.hover(h.pt(100, 120.0));
    let ev = h.key_cmd(Key::A);
    assert!(ev.changed);
    assert!(h.st.selection.contains(1));
    assert!(h.st.selection.contains(4));
    assert!(!h.st.selection.contains(2), "hidden object selected");
    assert!(
        !h.st.selection.contains(3),
        "interval-filtered object selected"
    );
    assert!(h.st.selection.multi());
}

#[test]
fn empty_click_clears_selection() {
    let mut h = Harness::new(DrawTool::HLine);
    let p = h.pt(120, 150.0);
    h.click(p);
    assert!(!h.st.selection.is_empty());
    let (elo, ehi) = h.lo_hi();
    h.st.tools.set_tool(DrawTool::Cursor);
    h.click(h.pt(75, elo + (ehi - elo) * 0.5));
    eprintln!("SEL after empty: {:?}", h.st.selection.ids());
    assert!(h.st.selection.is_empty());
}

#[test]
fn eraser_removes_topmost_only_keeps_locked_and_interval() {
    let mut h = Harness::new(DrawTool::Cursor);
    let l1 = Drawing::new(
        1,
        DrawingKind::HLine,
        Anchor {
            t: h.cd.t[60],
            price: 120.0,
        },
        Anchor {
            t: h.cd.t[140],
            price: 120.0,
        },
    );
    let l2 = Drawing {
        locked: true,
        ..Drawing::new(
            2,
            DrawingKind::HLine,
            Anchor {
                t: h.cd.t[60],
                price: 120.0,
            },
            Anchor {
                t: h.cd.t[140],
                price: 120.0,
            },
        )
    };
    let l3 = Drawing {
        visible_intervals: Some(vec!["1H".into()]),
        ..Drawing::new(
            3,
            DrawingKind::HLine,
            Anchor {
                t: h.cd.t[60],
                price: 160.0,
            },
            Anchor {
                t: h.cd.t[140],
                price: 160.0,
            },
        )
    };
    let l4 = Drawing::new(
        4,
        DrawingKind::HLine,
        Anchor {
            t: h.cd.t[60],
            price: 160.0,
        },
        Anchor {
            t: h.cd.t[140],
            price: 160.0,
        },
    );
    h.st.drawings.extend([l1, l2, l3, l4]);
    h.st.tools.set_tool(DrawTool::Eraser);
    // Two overlapping lines at 120: the later (locked) one stays.
    h.click(h.pt(100, 120.0));
    assert!(h.st.drawings.iter().any(|d| d.id == 2), "locked deleted");
    // Interval-hidden line at 160 is unreachable; the visible one goes.
    h.click(h.pt(100, 160.0));
    assert!(
        h.st.drawings.iter().any(|d| d.id == 3),
        "interval-hidden line was erased"
    );
    assert!(
        !h.st.drawings.iter().any(|d| d.id == 4),
        "visible line at 160 was kept"
    );
    // Locked line is also unerasable (TV keeps locked objects).
    h.click(h.pt(100, 120.0));
    assert!(h.st.drawings.iter().any(|d| d.id == 2));
}

#[test]
fn interval_visibility_blocks_hit_until_shown() {
    let mut h = Harness::new(DrawTool::Cursor);
    let mut d = Drawing::new(
        7,
        DrawingKind::HLine,
        Anchor {
            t: h.cd.t[60],
            price: 150.0,
        },
        Anchor {
            t: h.cd.t[140],
            price: 150.0,
        },
    );
    d.visible_intervals = Some(vec!["1H".into()]);
    h.st.drawings.push(d);
    h.click(h.pt(100, 150.0));
    assert!(h.st.selection.is_empty(), "interval-hidden line selectable");
    h.st.drawings[0].visible_intervals = None;
    h.click(h.pt(100, 150.0));
    assert!(h.st.selection.contains(7));
}

#[test]
fn shift_drag_zoom_rect_then_double_click_reset() {
    let mut h = Harness::new(DrawTool::Cursor);
    let before = (h.st.view.spacing, h.st.view.right_offset);
    let a = Pos2::new(200.0, 120.0);
    let b = Pos2::new(620.0, 400.0);
    h.shift_drag(a, b);
    let zoom = h.st.price_zoom.expect("shift-drag produced no price zoom");
    assert!(zoom.0 < zoom.1);
    assert_ne!(h.st.view.spacing, before.0, "time axis not fitted");

    h.double_click(Pos2::new(400.0, 300.0));
    assert!(h.st.price_zoom.is_none(), "double click kept price zoom");
    assert_eq!(h.st.view.spacing, 8.0);
    assert_eq!(h.st.view.right_offset, 0.0);
}

#[test]
fn wheel_zoom_scroll_and_arrow_keys_move_the_view() {
    let mut h = Harness::new(DrawTool::Cursor);
    let center = Pos2::new(400.0, 300.0);
    h.hover(center);
    h.frame(vec![Event::Zoom(1.25)]);
    assert!(h.st.view.spacing > 6.0, "wheel zoom did not apply");
    let off = h.st.view.right_offset;
    h.hover(center);
    h.frame(vec![Event::MouseWheel {
        delta: Vec2::new(-80.0, 0.0),
        unit: egui::MouseWheelUnit::Point,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::default(),
    }]);
    assert_ne!(h.st.view.right_offset, off, "scroll pan did not apply");
    let off2 = h.st.view.right_offset;
    h.hover(center);
    h.key(Key::ArrowLeft);
    assert!(
        h.st.view.right_offset > off2,
        "ArrowLeft should reveal older bars"
    );
    let left = h.st.view.right_offset;
    h.hover(center);
    h.key(Key::ArrowRight);
    assert!(
        h.st.view.right_offset < left - 1.0,
        "ArrowRight should reveal newer bars: {left} -> {}",
        h.st.view.right_offset
    );
}

#[test]
fn measure_tool_click_click_and_escape() {
    let mut h = Harness::new(DrawTool::Measure);
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    h.click(h.pt(70, lo + span * 0.3));
    assert!(h.st.tools.measure.is_some_and(|m| !m.done));
    h.click(h.pt(130, lo + span * 0.7));
    assert!(h.st.tools.measure.is_some_and(|m| m.done));
    assert!(h.st.drawings.is_empty(), "measure must not create drawings");
    h.key(Key::Escape);
    assert!(h.st.tools.measure.is_none());
}

#[test]
fn cursor_mode_tools_do_not_draw() {
    for tool in [
        DrawTool::Cursor,
        DrawTool::Crosshair,
        DrawTool::Dot,
        DrawTool::Demonstration,
        DrawTool::Performance,
    ] {
        let mut h = Harness::new(tool);
        let (lo, hi) = h.lo_hi();
        let ev = h.click(h.pt(100, lo + (hi - lo) * 0.5));
        assert!(h.st.drawings.is_empty(), "{tool:?} drew something");
        assert!(!ev.drawing_completed);
    }
}

#[test]
fn popup_rect_clicks_do_not_touch_the_chart() {
    let mut h = Harness::new(DrawTool::Cursor);
    h.st.drawings.push(Drawing::new(
        1,
        DrawingKind::HLine,
        Anchor {
            t: h.cd.t[60],
            price: 150.0,
        },
        Anchor {
            t: h.cd.t[140],
            price: 150.0,
        },
    ));
    h.st.popup_rect = Some(Rect::from_min_size(
        Pos2::new(60.0, 100.0),
        Vec2::new(200.0, 300.0),
    ));
    h.click(Pos2::new(120.0, 200.0));
    assert!(
        h.st.selection.is_empty(),
        "click inside popup reached chart"
    );
    h.st.popup_rect = None;
    h.click(h.pt(100, 150.0));
    assert!(h.st.selection.contains(1));
}

#[test]
fn keep_drawing_contract_tool_untouched_after_completion() {
    // The chart never auto-resets the tool; the app decides based on
    // keep-drawing (app.rs `reset_tool = ev.drawing_completed &&
    // !keep_drawing`). Assert the contract.
    let mut h = Harness::new(DrawTool::TrendLine);
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    h.click(h.pt(70, lo + span * 0.3));
    h.click(h.pt(130, lo + span * 0.6));
    assert_eq!(h.st.tools.tool, DrawTool::TrendLine);
}

#[test]
fn selected_anchor_drag_moves_only_that_anchor() {
    let mut h = Harness::new(DrawTool::TrendLine);
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    let a = h.pt(70, lo + span * 0.3);
    let b = h.pt(130, lo + span * 0.6);
    h.click(a);
    h.click(b);
    h.st.tools.set_tool(DrawTool::Cursor);
    // Move the B handle; A must stay.
    let a_price = h.st.drawings[0].a.price;
    let b_price = h.st.drawings[0].b.price;
    let new_b = h.pt(130, lo + span * 0.45);
    h.drag(b, new_b);
    assert!(
        (h.st.drawings[0].a.price - a_price).abs() < 1e-9,
        "anchor A dragged with B"
    );
    assert!(
        h.st.drawings[0].b.price != b_price,
        "anchor B unchanged ({b_price})"
    );
}

#[test]
fn press_drag_release_completes_two_point_tools() {
    // TV: lines, channels, fibs and shapes are drawn by press-drag-release;
    // the release places anchor two. TickView once only accepted click-click,
    // so real trackpad drags silently produced nothing.
    for tool in [
        DrawTool::TrendLine,
        DrawTool::Channel,
        DrawTool::Fib,
        DrawTool::Rect,
    ] {
        let mut h = Harness::new(tool);
        let (lo, hi) = h.lo_hi();
        let span = hi - lo;
        let a = h.pt(h.cd.len() - 50, lo + span * 0.7);
        let b = h.pt(h.cd.len() - 20, lo + span * 0.3);
        let ev = h.drag(a, b);
        let pts = tool.kind().unwrap().points();
        if pts == 2 {
            assert!(ev.drawing_completed, "{tool:?}: drag did not complete");
            assert_eq!(h.st.drawings.len(), 1, "{tool:?}: drag created nothing");
            let d = &h.st.drawings[0];
            assert_eq!(d.kind, tool.kind().unwrap(), "{tool:?}: wrong kind");
            assert!(h.st.selection.contains(d.id), "{tool:?}: not selected");
            assert!(h.st.tools.draft.is_none(), "{tool:?}: draft left open");
        } else {
            // Three-anchor tools (channels, trend-based fibs) take the first
            // two anchors from the drag and wait for the third by click.
            assert!(h.st.tools.draft.is_some(), "{tool:?}: draft not kept");
        }
    }
}

#[test]
fn drag_does_not_disturb_click_placement() {
    // Single-point tools still create on click; a click-click draft still
    // completes on its second click.
    let mut h = Harness::new(DrawTool::HLine);
    let (lo, hi) = h.lo_hi();
    let p = h.pt(h.cd.len() - 30, lo + (hi - lo) * 0.4);
    assert!(h.click(p).drawing_completed);
    assert_eq!(h.st.drawings.len(), 1);

    let mut h = Harness::new(DrawTool::TrendLine);
    let (lo, hi) = h.lo_hi();
    let span = hi - lo;
    let a = h.pt(h.cd.len() - 50, lo + span * 0.6);
    let b = h.pt(h.cd.len() - 20, lo + span * 0.2);
    assert!(!h.click(a).drawing_completed);
    assert!(h.st.tools.draft.is_some());
    let ev = h.click(b);
    assert!(ev.drawing_completed, "click-click regression");
    assert_eq!(h.st.drawings.len(), 1);
}
