//! Pixel painters: chart types, plot styles, grid. Pure egui drawing.

use egui::epaint::PathShape;
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::bars::View;
use crate::priceaxis::PriceAxis;
#[allow(unused_imports)]
use ta::Candles;
use ta::registry::PlotStyle;

/// Chart type mirroring TradingView's main series styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChartType {
    Candles,
    Bars,
    Line,
    Area,
    HeikinAshi,
}

impl ChartType {
    pub const ALL: [Self; 5] = [
        Self::Candles,
        Self::Bars,
        Self::Line,
        Self::Area,
        Self::HeikinAshi,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Candles => "Candles",
            Self::Bars => "Bars",
            Self::Line => "Line",
            Self::Area => "Area",
            Self::HeikinAshi => "Heikin Ashi",
        }
    }
}

/// TV dark-theme palette.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub up: Color32,
    pub down: Color32,
    pub wick: Color32,
    pub line: Color32,
    pub area_fill: Color32,
    pub grid: Color32,
    pub text: Color32,
    pub crosshair: Color32,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            up: Color32::from_rgb(0x26, 0xA6, 0x9A),
            down: Color32::from_rgb(0xEF, 0x53, 0x50),
            wick: Color32::from_rgb(0x78, 0x7B, 0x86),
            line: Color32::from_rgb(0x29, 0x62, 0xEB),
            area_fill: Color32::from_rgba_premultiplied(0x29, 0x62, 0xEB, 0x33),
            grid: Color32::from_rgb(0x2A, 0x2E, 0x39),
            text: Color32::from_rgb(0xB2, 0xB5, 0xBE),
            crosshair: Color32::from_rgb(0x75, 0x86, 0x96),
        }
    }
}

#[must_use]
pub const fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[must_use]
pub fn rgba(c: u32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied((c >> 16) as u8, (c >> 8) as u8, c as u8, a)
}

#[must_use]
pub fn body_width(spacing: f32) -> f32 {
    if spacing < 1.5 {
        1.0
    } else {
        (spacing - 1.0).max(1.0)
    }
}

/// Draw OHLCV series for the visible window.
pub fn draw_bars(
    p: &Painter,
    v: &View,
    ax: &PriceAxis,
    cd: &Candles,
    kind: ChartType,
    pal: &Palette,
) {
    let (s, e) = v.visible();
    if s >= e {
        return;
    }
    let bw = body_width(v.spacing);
    let ha;
    let data = if kind == ChartType::HeikinAshi {
        ha = ta::heikin_ashi(cd);
        &ha
    } else {
        cd
    };
    match kind {
        ChartType::Candles | ChartType::HeikinAshi => {
            for i in s..e {
                let (o, h, l, c) = (data.o[i], data.h[i], data.l[i], data.c[i]);
                if !h.is_finite() || !l.is_finite() {
                    continue;
                }
                let x = v.x_of(i);
                let color = if c >= o { pal.up } else { pal.down };
                p.line_segment(
                    [Pos2::new(x, ax.y_of(h)), Pos2::new(x, ax.y_of(l))],
                    Stroke::new(1.0f32, pal.wick),
                );
                let y_top = ax.y_of(o.max(c));
                let y_bot = ax.y_of(o.min(c));
                let hgt = (y_bot - y_top).max(1.0);
                p.rect_filled(
                    Rect::from_min_size(Pos2::new(x - bw / 2.0, y_top), Vec2::new(bw, hgt)),
                    CornerRadius::ZERO,
                    color,
                );
            }
        }
        ChartType::Bars => {
            for i in s..e {
                let (o, h, l, c) = (data.o[i], data.h[i], data.l[i], data.c[i]);
                if !h.is_finite() {
                    continue;
                }
                let x = v.x_of(i);
                let color = if c >= o { pal.up } else { pal.down };
                let st = Stroke::new(1.0f32, color);
                p.line_segment([Pos2::new(x, ax.y_of(h)), Pos2::new(x, ax.y_of(l))], st);
                p.line_segment(
                    [
                        Pos2::new(x - bw / 2.0, ax.y_of(o)),
                        Pos2::new(x, ax.y_of(o)),
                    ],
                    st,
                );
                p.line_segment(
                    [
                        Pos2::new(x, ax.y_of(c)),
                        Pos2::new(x + bw / 2.0, ax.y_of(c)),
                    ],
                    st,
                );
            }
        }
        ChartType::Line => {
            let pts = close_path(data, v, ax, s, e);
            if pts.len() > 1 {
                p.line(pts, Stroke::new(1.5f32, pal.line));
            }
        }
        ChartType::Area => {
            let pts = close_path(data, v, ax, s, e);
            if pts.len() > 1 {
                let mut fill = pts.clone();
                let bottom = p.clip_rect().max.y;
                fill.push(Pos2::new(fill[fill.len() - 1].x, bottom));
                fill.push(Pos2::new(fill[0].x, bottom));
                p.add(PathShape {
                    points: fill,
                    closed: true,
                    fill: pal.area_fill,
                    stroke: Stroke::NONE.into(),
                });
                p.line(pts, Stroke::new(1.5f32, pal.line));
            }
        }
    }
}

fn close_path(data: &Candles, v: &View, ax: &PriceAxis, s: usize, e: usize) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(e - s);
    for i in s..e {
        let c = data.c[i];
        if c.is_finite() {
            pts.push(Pos2::new(v.x_of(i), ax.y_of(c)));
        }
    }
    pts
}

/// Draw one indicator series according to its plot style.
pub fn draw_series(
    p: &Painter,
    v: &View,
    ax: &PriceAxis,
    series: &[f64],
    style: PlotStyle,
    color: Color32,
    neg_color: Option<Color32>,
) {
    let (s, e) = v.visible();
    let e = e.min(series.len());
    if s >= e {
        return;
    }
    let bw = body_width(v.spacing);
    let zero_y = ax.y_of(0.0).clamp(p.clip_rect().min.y, p.clip_rect().max.y);
    match style {
        PlotStyle::Line | PlotStyle::Area => {
            let pts: Vec<Pos2> = series[s..e]
                .iter()
                .enumerate()
                .filter(|(_, y)| y.is_finite())
                .map(|(j, y)| Pos2::new(v.x_of(s + j), ax.y_of(*y)))
                .collect();
            if pts.len() > 1 {
                if style == PlotStyle::Area {
                    let mut fill = pts.clone();
                    let bottom = p.clip_rect().max.y;
                    fill.push(Pos2::new(fill[fill.len() - 1].x, bottom));
                    fill.push(Pos2::new(fill[0].x, bottom));
                    let fc = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 0x33);
                    p.add(PathShape {
                        points: fill,
                        closed: true,
                        fill: fc,
                        stroke: Stroke::NONE.into(),
                    });
                }
                p.line(pts, Stroke::new(1.5f32, color));
            }
        }
        PlotStyle::Histogram | PlotStyle::HistogramSigned | PlotStyle::Columns => {
            let col_w = if style == PlotStyle::Columns {
                bw
            } else {
                (bw * 0.6).max(1.0)
            };
            for (j, &val) in series[s..e].iter().enumerate() {
                if !val.is_finite() {
                    continue;
                }
                let i = s + j;
                let x = v.x_of(i);
                let y = ax.y_of(val);
                let c = match (style, neg_color) {
                    (PlotStyle::HistogramSigned, Some(neg)) if val < 0.0 => neg,
                    _ => color,
                };
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x - col_w / 2.0, zero_y.min(y)),
                        Pos2::new(x + col_w / 2.0, zero_y.max(y)),
                    ),
                    CornerRadius::ZERO,
                    c,
                );
            }
        }
        PlotStyle::Circles => {
            let r = (v.spacing * 0.25).clamp(1.5, 4.0);
            for (j, &val) in series[s..e].iter().enumerate() {
                if val.is_finite() {
                    p.circle_filled(Pos2::new(v.x_of(s + j), ax.y_of(val)), r, color);
                }
            }
        }
    }
}

/// Horizontal grid lines + price labels (labels to the right of `clip`).
pub fn draw_price_grid(p: &Painter, clip: Rect, ax: &PriceAxis, font: FontId, pal: &Palette) {
    let stroke = Stroke::new(1.0f32, pal.grid);
    for t in ax.ticks(7) {
        let y = ax.y_of(t.value);
        p.line_segment([Pos2::new(clip.min.x, y), Pos2::new(clip.max.x, y)], stroke);
        p.text(
            Pos2::new(clip.max.x + 4.0, y),
            egui::Align2::LEFT_CENTER,
            crate::fmt_price(t.value).as_str(),
            font.clone(),
            pal.text,
        );
    }
}

/// Vertical grid lines at bar x positions.
pub fn draw_vgrid(p: &Painter, clip: Rect, x_ticks: &[f32], pal: &Palette) {
    let stroke = Stroke::new(1.0f32, pal.grid);
    for x in x_ticks {
        p.line_segment(
            [Pos2::new(*x, clip.min.y), Pos2::new(*x, clip.max.y)],
            stroke,
        );
    }
}
