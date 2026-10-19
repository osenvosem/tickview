//! Hand-painted vector icons. No icon font, no emoji.

use egui::{
    Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Cursor,
    Crosshair,
    TrendLine,
    Ray,
    ExtLine,
    HRay,
    HLine,
    VLine,
    Channel,
    Rect,
    Ellipse,
    Fib,
    FibExt,
    Arrow,
    Text,
    Measure,
    Brush,
    Pitchfork,
    Schiff,
    Regression,
    LongPosition,
    ShortPosition,
    Magnet,
    Search,
    Bell,
    Settings,
    Plus,
    Close,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Trash,
    Camera,
    Sun,
    Moon,
    Table,
    Link,
    Alert,
    Layout,
    Indicators,
    Compare,
    Candles,
    Watchlist,
    Eye,
    EyeOff,
    More,
    Pencil,
    Eraser,
    Undo,
    Redo,
    Lock,
    Unlock,
    Fullscreen,
    Download,
    Refresh,
}

#[allow(dead_code)]
impl Icon {
    pub const ALL: &'static [Self] = &[
        Self::Cursor,
        Self::Crosshair,
        Self::TrendLine,
        Self::Ray,
        Self::ExtLine,
        Self::HRay,
        Self::HLine,
        Self::VLine,
        Self::Channel,
        Self::Rect,
        Self::Ellipse,
        Self::Fib,
        Self::FibExt,
        Self::Arrow,
        Self::Text,
        Self::Measure,
        Self::Brush,
        Self::Pitchfork,
        Self::Schiff,
        Self::Regression,
        Self::LongPosition,
        Self::ShortPosition,
        Self::Magnet,
        Self::Search,
        Self::Bell,
        Self::Settings,
        Self::Plus,
        Self::Close,
        Self::ChevronDown,
        Self::ChevronLeft,
        Self::ChevronRight,
        Self::Trash,
        Self::Camera,
        Self::Sun,
        Self::Moon,
        Self::Table,
        Self::Link,
        Self::Alert,
        Self::Layout,
        Self::Indicators,
        Self::Compare,
        Self::Candles,
        Self::Watchlist,
        Self::Eye,
        Self::EyeOff,
        Self::More,
        Self::Pencil,
        Self::Eraser,
        Self::Undo,
        Self::Redo,
        Self::Lock,
        Self::Unlock,
        Self::Fullscreen,
        Self::Download,
        Self::Refresh,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::Crosshair => "crosshair",
            Self::TrendLine => "trend-line",
            Self::Ray => "ray",
            Self::ExtLine => "extended-line",
            Self::HRay => "horizontal-ray",
            Self::HLine => "horizontal-line",
            Self::VLine => "vertical-line",
            Self::Channel => "channel",
            Self::Rect => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Fib => "fibonacci",
            Self::FibExt => "fibonacci-extension",
            Self::Arrow => "arrow",
            Self::Text => "text",
            Self::Measure => "measure",
            Self::Brush => "brush",
            Self::Pitchfork => "pitchfork",
            Self::Schiff => "schiff-pitchfork",
            Self::Regression => "regression-trend",
            Self::LongPosition => "long-position",
            Self::ShortPosition => "short-position",
            Self::Magnet => "magnet",
            Self::Search => "search",
            Self::Bell => "alerts",
            Self::Settings => "settings",
            Self::Plus => "add",
            Self::Close => "close",
            Self::ChevronDown => "chevron-down",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronRight => "chevron-right",
            Self::Trash => "remove",
            Self::Camera => "screenshot",
            Self::Sun => "light-theme",
            Self::Moon => "dark-theme",
            Self::Table => "data-window",
            Self::Link => "link",
            Self::Alert => "alert",
            Self::Layout => "layout",
            Self::Indicators => "indicators",
            Self::Compare => "compare",
            Self::Candles => "candles",
            Self::Watchlist => "watchlist",
            Self::Eye => "visible",
            Self::EyeOff => "hidden",
            Self::More => "more",
            Self::Pencil => "edit",
            Self::Eraser => "erase",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Fullscreen => "fullscreen",
            Self::Download => "download",
            Self::Refresh => "refresh",
        }
    }

    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32, width: f32) {
        let s = Stroke::new(width, color);
        let fill = color;
        let r = rect.shrink(rect.width().min(rect.height()) * 0.12);
        let px = |x: f32, y: f32| {
            pos2(
                r.min.x + r.width() * x / 24.0,
                r.min.y + r.height() * y / 24.0,
            )
        };
        let line = |x1: f32, y1: f32, x2: f32, y2: f32| {
            painter.line_segment([px(x1, y1), px(x2, y2)], s);
        };
        let dot = |x: f32, y: f32, radius: f32| {
            painter.circle_filled(px(x, y), radius, color);
        };
        let circle = |cx: f32, cy: f32, radius: f32| {
            painter.circle_stroke(px(cx, cy), radius, s);
        };
        let round_rect = |x1: f32, y1: f32, x2: f32, y2: f32, radius: f32| {
            painter.rect_stroke(
                Rect::from_min_max(px(x1, y1), px(x2, y2)),
                CornerRadius::same(radius as u8),
                s,
                StrokeKind::Inside,
            );
        };
        let path = |points: &[(f32, f32)]| {
            let pts: Vec<Pos2> = points.iter().map(|&(x, y)| px(x, y)).collect();
            painter.add(Shape::line(pts, s));
        };
        let fill_path = |points: &[(f32, f32)]| {
            let pts: Vec<Pos2> = points.iter().map(|&(x, y)| px(x, y)).collect();
            painter.add(Shape::convex_polygon(pts, fill, Stroke::NONE));
        };
        let arc = |cx: f32, cy: f32, radius: f32, start: f32, end: f32| {
            let center = px(cx, cy);
            let mut pts = Vec::with_capacity(18);
            for i in 0..=16 {
                let t = start + (end - start) * i as f32 / 16.0;
                pts.push(center + vec2(t.cos(), t.sin()) * radius);
            }
            painter.add(Shape::line(pts, s));
        };

        match self {
            Self::Cursor => {
                fill_path(&[(5.0, 3.0), (19.0, 12.4), (12.8, 13.8), (9.6, 20.0)]);
            }
            Self::Crosshair => {
                circle(12.0, 12.0, 5.0);
                line(12.0, 2.0, 12.0, 6.0);
                line(12.0, 18.0, 12.0, 22.0);
                line(2.0, 12.0, 6.0, 12.0);
                line(18.0, 12.0, 22.0, 12.0);
            }
            Self::TrendLine => {
                line(4.0, 19.0, 20.0, 5.0);
                dot(4.0, 19.0, 2.0);
                dot(20.0, 5.0, 2.0);
            }
            Self::Ray => {
                line(4.0, 20.0, 21.0, 4.0);
                dot(4.0, 20.0, 2.0);
                fill_path(&[(21.0, 4.0), (16.4, 5.2), (19.8, 8.6)]);
            }
            Self::ExtLine => {
                line(3.0, 20.0, 21.0, 4.0);
                fill_path(&[(3.0, 20.0), (8.2, 18.0), (5.6, 15.4)]);
                fill_path(&[(21.0, 4.0), (15.8, 6.0), (18.4, 8.6)]);
            }
            Self::HRay => {
                line(4.0, 12.0, 19.0, 12.0);
                dot(4.0, 12.0, 2.0);
                fill_path(&[(21.0, 12.0), (15.8, 9.8), (15.8, 14.2)]);
            }
            Self::HLine => {
                line(3.0, 12.0, 21.0, 12.0);
                dot(3.0, 12.0, 1.8);
                dot(21.0, 12.0, 1.8);
            }
            Self::VLine => {
                line(12.0, 3.0, 12.0, 21.0);
                dot(12.0, 3.0, 1.8);
                dot(12.0, 21.0, 1.8);
            }
            Self::Channel => {
                line(4.0, 16.0, 18.0, 3.0);
                line(7.0, 21.0, 21.0, 8.0);
                fill_path(&[(9.0, 16.6), (14.0, 16.6), (14.0, 19.0), (9.0, 19.0)]);
            }
            Self::Rect => round_rect(4.0, 6.0, 20.0, 18.0, 1.5),
            Self::Ellipse => {
                painter.add(Shape::circle_stroke(
                    px(12.0, 12.0),
                    r.width().min(r.height()) * 0.30,
                    s,
                ));
            }
            Self::Fib => {
                line(3.0, 7.0, 21.0, 7.0);
                line(3.0, 12.0, 21.0, 12.0);
                line(3.0, 17.0, 21.0, 17.0);
                line(5.0, 4.0, 19.0, 20.0);
                dot(19.0, 20.0, 1.6);
            }
            Self::FibExt => {
                line(3.0, 7.0, 14.0, 7.0);
                line(3.0, 12.0, 18.0, 12.0);
                line(3.0, 17.0, 22.0, 17.0);
                line(6.0, 20.0, 18.0, 4.0);
                dot(6.0, 20.0, 1.6);
                fill_path(&[(19.0, 3.0), (14.0, 4.4), (17.2, 7.6)]);
            }
            Self::Arrow => {
                line(3.0, 20.0, 18.0, 5.0);
                fill_path(&[(20.0, 3.0), (14.2, 4.8), (18.2, 8.8)]);
            }
            Self::Text => {
                line(5.0, 5.0, 19.0, 5.0);
                line(12.0, 5.0, 12.0, 20.0);
                line(9.0, 20.0, 15.0, 20.0);
            }
            Self::Measure => {
                line(3.0, 18.0, 21.0, 6.0);
                line(3.0, 15.0, 6.0, 21.0);
                line(21.0, 3.0, 18.0, 9.0);
                line(7.0, 15.0, 10.0, 17.0);
                line(12.0, 12.0, 15.0, 14.0);
                line(17.0, 9.0, 20.0, 11.0);
            }
            Self::Brush => {
                line(3.0, 17.0, 8.0, 10.0);
                line(8.0, 10.0, 12.0, 14.0);
                line(12.0, 14.0, 16.0, 7.0);
                line(16.0, 7.0, 21.0, 11.0);
            }
            Self::Pitchfork => {
                line(5.0, 19.0, 19.0, 5.0);
                line(3.0, 13.0, 13.0, 3.0);
                line(11.0, 21.0, 21.0, 11.0);
                dot(5.0, 19.0, 1.6);
            }
            Self::Schiff => {
                line(5.0, 18.0, 19.0, 6.0);
                line(3.0, 12.0, 13.0, 2.0);
                line(11.0, 22.0, 21.0, 12.0);
                dot(5.0, 18.0, 1.6);
            }
            Self::Regression => {
                line(3.0, 18.0, 21.0, 8.0);
                line(3.0, 14.0, 21.0, 4.0);
                line(3.0, 22.0, 21.0, 12.0);
            }
            Self::LongPosition => {
                fill_path(&[(4.0, 5.0), (20.0, 5.0), (20.0, 12.0), (4.0, 12.0)]);
                line(4.0, 12.0, 20.0, 12.0);
                round_rect(4.0, 12.0, 20.0, 19.0, 1.0);
            }
            Self::ShortPosition => {
                fill_path(&[(4.0, 12.0), (20.0, 12.0), (20.0, 19.0), (4.0, 19.0)]);
                line(4.0, 12.0, 20.0, 12.0);
                round_rect(4.0, 5.0, 20.0, 12.0, 1.0);
            }
            Self::Magnet => {
                path(&[
                    (5.0, 4.0),
                    (5.0, 13.0),
                    (5.8, 16.5),
                    (8.0, 19.0),
                    (12.0, 20.0),
                ]);
                path(&[(19.0, 4.0), (19.0, 13.0), (18.2, 16.5), (16.0, 19.0)]);
                line(5.0, 4.0, 9.0, 4.0);
                line(15.0, 4.0, 19.0, 4.0);
            }
            Self::Search => {
                circle(10.5, 10.5, 6.0);
                line(15.0, 15.0, 21.0, 21.0);
            }
            Self::Bell => {
                path(&[
                    (6.0, 17.0),
                    (6.0, 10.0),
                    (7.0, 6.8),
                    (10.0, 4.5),
                    (12.0, 4.0),
                ]);
                path(&[
                    (18.0, 17.0),
                    (18.0, 10.0),
                    (17.0, 6.8),
                    (14.0, 4.5),
                    (12.0, 4.0),
                ]);
                line(5.0, 17.0, 19.0, 17.0);
                path(&[(10.2, 20.0), (12.0, 21.0), (13.8, 20.0)]);
            }
            Self::Settings => {
                circle(12.0, 12.0, 3.0);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    let inner = 6.2;
                    let outer = 9.2;
                    line(
                        12.0 + a.cos() * inner,
                        12.0 + a.sin() * inner,
                        12.0 + a.cos() * outer,
                        12.0 + a.sin() * outer,
                    );
                }
            }
            Self::Plus => {
                line(12.0, 4.0, 12.0, 20.0);
                line(4.0, 12.0, 20.0, 12.0);
            }
            Self::Close => {
                line(5.0, 5.0, 19.0, 19.0);
                line(19.0, 5.0, 5.0, 19.0);
            }
            Self::ChevronDown => path(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Self::ChevronLeft => path(&[(15.0, 5.0), (9.0, 12.0), (15.0, 19.0)]),
            Self::ChevronRight => path(&[(9.0, 5.0), (15.0, 12.0), (9.0, 19.0)]),
            Self::Trash => {
                line(4.0, 7.0, 20.0, 7.0);
                line(9.0, 4.0, 15.0, 4.0);
                path(&[
                    (6.0, 7.0),
                    (7.0, 20.0),
                    (10.0, 21.0),
                    (14.0, 21.0),
                    (17.0, 20.0),
                    (18.0, 7.0),
                ]);
                line(10.0, 10.0, 10.5, 17.0);
                line(14.0, 10.0, 13.5, 17.0);
            }
            Self::Camera => {
                round_rect(3.0, 7.0, 21.0, 19.0, 2.0);
                path(&[(8.0, 7.0), (9.2, 4.0), (14.8, 4.0), (16.0, 7.0)]);
                circle(12.0, 13.0, 3.2);
            }
            Self::Sun => {
                circle(12.0, 12.0, 4.0);
                for i in 0..8 {
                    let a = i as f32 * std::f32::consts::TAU / 8.0;
                    line(
                        12.0 + a.cos() * 7.0,
                        12.0 + a.sin() * 7.0,
                        12.0 + a.cos() * 9.5,
                        12.0 + a.sin() * 9.5,
                    );
                }
            }
            Self::Moon => {
                arc(12.0, 12.0, 8.0, -1.9, 1.9);
                arc(16.0, 12.0, 6.4, 1.8, 4.5);
            }
            Self::Table => {
                round_rect(3.0, 4.0, 21.0, 20.0, 1.0);
                line(3.0, 9.0, 21.0, 9.0);
                line(9.0, 9.0, 9.0, 20.0);
                line(15.0, 9.0, 15.0, 20.0);
            }
            Self::Link => {
                path(&[(9.0, 15.0), (15.0, 9.0)]);
                arc(8.0, 16.0, 4.0, -0.8, 1.8);
                arc(16.0, 8.0, 4.0, 2.3, 4.9);
            }
            Self::Alert => {
                path(&[(12.0, 3.0), (22.0, 20.0), (2.0, 20.0), (12.0, 3.0)]);
                line(12.0, 9.0, 12.0, 14.0);
                dot(12.0, 17.2, 1.1);
            }
            Self::Layout => {
                round_rect(3.0, 4.0, 21.0, 20.0, 1.0);
                line(3.0, 9.0, 21.0, 9.0);
                line(10.0, 9.0, 10.0, 20.0);
            }
            Self::Indicators => {
                path(&[
                    (3.0, 17.0),
                    (8.0, 11.0),
                    (12.0, 14.0),
                    (16.0, 7.0),
                    (21.0, 10.0),
                ]);
                dot(16.0, 7.0, 1.7);
            }
            Self::Compare => {
                path(&[
                    (3.0, 16.0),
                    (8.0, 12.0),
                    (12.0, 14.0),
                    (17.0, 8.0),
                    (21.0, 10.0),
                ]);
                path(&[
                    (3.0, 19.0),
                    (8.0, 17.0),
                    (12.0, 18.5),
                    (17.0, 14.0),
                    (21.0, 16.0),
                ]);
            }
            Self::Candles => {
                line(7.0, 3.0, 7.0, 21.0);
                round_rect(5.0, 7.0, 9.0, 17.0, 0.0);
                line(17.0, 4.0, 17.0, 21.0);
                round_rect(15.0, 6.0, 19.0, 14.0, 0.0);
            }
            Self::Watchlist => {
                for y in [7.0, 12.0, 17.0] {
                    dot(5.0, y, 1.4);
                    line(9.0, y, 20.0, y);
                }
            }
            Self::Eye => {
                path(&[
                    (3.0, 12.0),
                    (6.0, 8.0),
                    (9.0, 6.0),
                    (12.0, 5.2),
                    (15.0, 6.0),
                    (18.0, 8.0),
                    (21.0, 12.0),
                ]);
                path(&[
                    (3.0, 12.0),
                    (6.0, 16.0),
                    (9.0, 18.0),
                    (12.0, 18.8),
                    (15.0, 18.0),
                    (18.0, 16.0),
                    (21.0, 12.0),
                ]);
                circle(12.0, 12.0, 2.6);
            }
            Self::EyeOff => {
                path(&[
                    (3.0, 12.0),
                    (6.0, 8.0),
                    (9.0, 6.0),
                    (12.0, 5.2),
                    (15.0, 6.0),
                    (18.0, 8.0),
                    (21.0, 12.0),
                ]);
                path(&[
                    (3.0, 12.0),
                    (6.0, 16.0),
                    (9.0, 18.0),
                    (12.0, 18.8),
                    (15.0, 18.0),
                    (18.0, 16.0),
                    (21.0, 12.0),
                ]);
                line(4.0, 4.0, 20.0, 20.0);
            }
            Self::More => {
                dot(5.0, 12.0, 1.6);
                dot(12.0, 12.0, 1.6);
                dot(19.0, 12.0, 1.6);
            }
            Self::Pencil => {
                path(&[
                    (4.0, 20.0),
                    (5.0, 15.0),
                    (16.0, 4.0),
                    (20.0, 8.0),
                    (9.0, 19.0),
                    (4.0, 20.0),
                ]);
                line(14.0, 6.0, 18.0, 10.0);
            }
            Self::Eraser => {
                path(&[
                    (4.0, 15.0),
                    (12.0, 5.0),
                    (20.0, 12.0),
                    (13.0, 20.0),
                    (7.0, 20.0),
                    (4.0, 17.0),
                    (4.0, 15.0),
                ]);
                line(9.0, 11.0, 16.0, 18.0);
            }
            Self::Undo => {
                path(&[(8.0, 7.0), (4.0, 12.0), (8.0, 17.0)]);
                path(&[(4.0, 12.0), (15.0, 12.0), (18.0, 13.5), (19.0, 17.0)]);
            }
            Self::Redo => {
                path(&[(16.0, 7.0), (20.0, 12.0), (16.0, 17.0)]);
                path(&[(20.0, 12.0), (9.0, 12.0), (6.0, 13.5), (5.0, 17.0)]);
            }
            Self::Lock => {
                round_rect(5.0, 10.0, 19.0, 21.0, 2.0);
                arc(12.0, 10.0, 4.5, std::f32::consts::PI, std::f32::consts::TAU);
            }
            Self::Unlock => {
                round_rect(5.0, 10.0, 19.0, 21.0, 2.0);
                arc(12.0, 10.0, 4.5, std::f32::consts::PI, 5.3);
            }
            Self::Fullscreen => {
                path(&[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)]);
                path(&[(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)]);
                path(&[(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)]);
                path(&[(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]);
            }
            Self::Download => {
                line(12.0, 3.0, 12.0, 16.0);
                path(&[(7.0, 11.0), (12.0, 16.0), (17.0, 11.0)]);
                path(&[(4.0, 19.0), (4.0, 21.0), (20.0, 21.0), (20.0, 19.0)]);
            }
            Self::Refresh => {
                arc(12.0, 12.0, 7.0, -2.4, 1.3);
                path(&[(18.4, 5.8), (19.5, 10.0), (15.3, 10.4)]);
            }
        }
    }

    /// Draw inside a square of `size`.
    pub fn draw(self, painter: &Painter, center: Pos2, size: f32, color: Color32, width: f32) {
        let rect = Rect::from_center_size(center, Vec2::splat(size));
        self.paint(painter, rect, color, width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn icon_names_are_unique() {
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            assert!(
                seen.insert(icon.name()),
                "duplicate icon name {}",
                icon.name()
            );
        }
    }

    #[test]
    fn all_icons_are_constructed() {
        assert_eq!(Icon::ALL.len(), 55);
    }
}
