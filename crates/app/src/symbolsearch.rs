//! TradingView-style symbol search dialog (reverse-engineered from the live
//! site 2026-10-22): fixed card, category chips, an exchange ("sources")
//! dropdown, and single-line rows with an accent highlight on the matched
//! ticker substring.
//!
//! All the logic that can be checked headlessly — filtering, selection
//! cycling, match spans, row labels, avatar color — lives in
//! [`SymbolSearch`]/free functions; [`show`] only paints and forwards choices.

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, Direction, Key, Layout, Margin, Rect, Response,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use marketdata::{SearchIndex, Symbol, SymbolKind};

use crate::icons::Icon;
use crate::theme::{self, Palette, font_regular, font_semibold};
use crate::widgets;

/// Card size from the 1120-wide capture; clamped to the viewport on small
/// screens.
const CARD_W: f32 = 840.0;
const CARD_H: f32 = 680.0;
const PAD: f32 = 20.0;
const TITLE_H: f32 = 56.0;
/// 40px field plus 4px breathing room above and below.
const INPUT_ROW_H: f32 = 48.0;
const CHIP_ROW_H: f32 = 44.0;
const DROPDOWN_ROW_H: f32 = 44.0;
const FOOTER_H: f32 = 40.0;
const ROW_H: f32 = 40.0;
const PILL_H: f32 = 28.0;
const AVATAR: f32 = 24.0;
const RIGHT_PAD: f32 = 24.0;
const MAX_RESULTS: usize = 40;

const PLACEHOLDER: &str = "Symbol, name, or exchange";
const FOOTER_TEXT: &str = "Index updates on startup: SEC, Binance, Kraken, Coinbase, CoinGecko";
const NO_RESULTS_TITLE: &str = "No matching symbols";
const NO_RESULTS_BODY: &str = "Try another ticker, name, or category.";

/// Deterministic avatar fills for the letter fallback (TV ships logos; we have
/// no logo assets, so every row gets a brand-ish color derived from the ticker).
const AVATAR_COLORS: [Color32; 8] = [
    Color32::from_rgb(0xF7, 0x93, 0x1A),
    Color32::from_rgb(0x62, 0x7E, 0xEA),
    Color32::from_rgb(0x26, 0xA1, 0x7B),
    Color32::from_rgb(0xF3, 0xBA, 0x2F),
    Color32::from_rgb(0x27, 0x75, 0xCA),
    Color32::from_rgb(0xE8, 0x41, 0x42),
    Color32::from_rgb(0x00, 0xD1, 0xB2),
    Color32::from_rgb(0x9B, 0x59, 0xB6),
];

/// Category chip. Only categories the local index actually holds data for —
/// TV's Bonds/Economy/Options tabs have no backing set here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chip {
    #[default]
    All,
    Stocks,
    Funds,
    Futures,
    Forex,
    Crypto,
    Indices,
}

impl Chip {
    pub const ALL: [Chip; 7] = [
        Chip::All,
        Chip::Stocks,
        Chip::Funds,
        Chip::Futures,
        Chip::Forex,
        Chip::Crypto,
        Chip::Indices,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Stocks => "Stocks",
            Self::Funds => "Funds",
            Self::Futures => "Futures",
            Self::Forex => "Forex",
            Self::Crypto => "Crypto",
            Self::Indices => "Indices",
        }
    }

    /// Kinds this chip filters to (`None` = no filter).
    #[must_use]
    pub fn kinds(self) -> Option<&'static [SymbolKind]> {
        match self {
            Self::All => None,
            Self::Stocks => Some(&[SymbolKind::Stock]),
            Self::Funds => Some(&[SymbolKind::Etf]),
            Self::Futures => Some(&[SymbolKind::Futures, SymbolKind::Commodity]),
            Self::Forex => Some(&[SymbolKind::Fx]),
            Self::Crypto => Some(&[SymbolKind::Crypto]),
            Self::Indices => Some(&[SymbolKind::Index]),
        }
    }
}

/// Dialog state: query, category/exchange filters, results and selection.
#[derive(Clone, Debug, Default)]
pub struct SymbolSearch {
    pub q: String,
    pub open: bool,
    pub focus_req: bool,
    pub sel: usize,
    pub chip: Chip,
    /// Exchange filter; reset to `None` whenever the chip changes.
    pub source: Option<String>,
    /// Filtered results. Empty query → the caller's watchlist.
    pub results: Vec<Symbol>,
    /// Distinct exchanges for the current query + chip.
    pub options: Vec<String>,
    pub dropdown_open: bool,
}

impl SymbolSearch {
    /// Re-run filtering. `watchlist` backs the empty-query state (TV shows its
    /// recommended list; we show what the user already follows).
    pub fn refresh(&mut self, index: &SearchIndex, watchlist: &[Symbol]) {
        if self.q.trim().is_empty() {
            self.results = watchlist.to_vec();
            self.options.clear();
        } else {
            self.results = index.search_filtered(
                &self.q,
                self.chip.kinds(),
                self.source.as_deref(),
                MAX_RESULTS,
            );
            self.options = index.exchange_options(&self.q, self.chip.kinds());
        }
        self.sel = self.sel.min(self.results.len().saturating_sub(1));
    }

    pub fn set_chip(&mut self, c: Chip) {
        self.chip = c;
        self.source = None;
        self.dropdown_open = false;
        self.sel = 0;
    }

    pub fn set_source(&mut self, s: Option<String>) {
        self.source = s;
        self.dropdown_open = false;
        self.sel = 0;
    }

    /// Move the selection, wrapping at both ends (TV wraps); no-op when empty.
    pub fn cycle(&mut self, delta: i32) {
        let Ok(len) = i64::try_from(self.results.len()) else {
            return;
        };
        if len == 0 {
            return;
        }
        let cur = i64::try_from(self.sel).unwrap_or(0);
        self.sel = usize::try_from((cur + i64::from(delta)).rem_euclid(len)).unwrap_or(0);
    }
}

/// Row type label, mirroring TV's `"<type> <domain>"` literals.
#[must_use]
pub fn type_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Stock => "stock",
        SymbolKind::Etf => "etf fund",
        SymbolKind::Index => "index",
        SymbolKind::Futures | SymbolKind::Commodity => "futures",
        SymbolKind::Crypto => "spot crypto",
        SymbolKind::Fx => "forex",
        SymbolKind::Cfd => "cfd",
        SymbolKind::Other => "other",
    }
}

/// Byte range of the first case-insensitive occurrence of the trimmed query in
/// `text` (`None` when empty or absent).
#[must_use]
pub fn highlight_spans(q: &str, text: &str) -> Option<(usize, usize)> {
    let needle = q.trim().to_uppercase();
    if needle.is_empty() {
        return None;
    }
    let start = text.to_uppercase().find(&needle)?;
    let end = start + needle.len();
    // Case folding can change byte length (e.g. 'İ'); only highlight when the
    // range still lands on char boundaries of the original text.
    if text.is_char_boundary(start) && text.is_char_boundary(end) {
        Some((start, end))
    } else {
        None
    }
}

#[must_use]
pub fn avatar_color(ticker: &str) -> Color32 {
    let sum: usize = ticker.bytes().map(usize::from).sum();
    AVATAR_COLORS[sum % AVATAR_COLORS.len()]
}

/// Paint the dialog; returns the symbol the user picked (click or Enter).
#[must_use]
pub fn show(
    ctx: &egui::Context,
    p: &Palette,
    s: &mut SymbolSearch,
    index: &SearchIndex,
    watchlist: &[Symbol],
    anchor: Option<Vec2>,
) -> Option<Symbol> {
    if !s.open {
        return None;
    }
    let vp = ctx.viewport_rect();
    // TV's card is fixed 840x680; small viewports clamp to viewport - gutter.
    let w = CARD_W.min((vp.width() - 80.0).max(360.0));
    let card_h = CARD_H.min((vp.height() - 60.0).max(320.0));
    let mut win = egui::Window::new("Symbol search")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .movable(false)
        .scroll([false, false])
        .auto_sized()
        .frame(
            theme::menu_frame(p)
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(0)),
        );
    if let Some(off) = anchor {
        win = win.anchor(Align2::CENTER_TOP, off);
    }
    let mut pick: Option<Symbol> = None;
    win.show(ctx, |ui| {
        // The window's default layout centers on the main axis, which lets rows
        // drift onto each other; pin it to top-down/min so the ring math holds.
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.set_width(w);
            ui.set_height(card_h);
            pick = body(ui, p, s, index, watchlist, w, card_h);
        });
    });
    pick
}

fn body(
    ui: &mut Ui,
    p: &Palette,
    s: &mut SymbolSearch,
    index: &SearchIndex,
    watchlist: &[Symbol],
    w: f32,
    card_h: f32,
) -> Option<Symbol> {
    let mut pick: Option<Symbol> = None;

    title_row(ui, p, s, w);
    if input_row(ui, p, s, w) {
        s.refresh(index, watchlist);
    }
    if !s.open {
        return None;
    }
    if let Some(c) = chips_row(ui, p, w, s.chip)
        && c != s.chip
    {
        s.set_chip(c);
        s.refresh(index, watchlist);
    }
    let dropdown = s.chip != Chip::All && !s.options.is_empty();
    if dropdown {
        let opts = s.options.clone();
        let (src, open) = source_dropdown(ui, p, &opts, s.source.as_deref());
        s.dropdown_open = open;
        if let Some(src) = src {
            s.set_source(src);
            s.refresh(index, watchlist);
        }
    } else {
        s.dropdown_open = false;
    }

    let chrome = TITLE_H + INPUT_ROW_H + CHIP_ROW_H + if dropdown { DROPDOWN_ROW_H } else { 0.0 };
    let list_h = (card_h - chrome - FOOTER_H).max(ROW_H);
    // Enter picks the ringed row; the caller owns closing.
    if ui.input(|i| i.key_pressed(Key::Enter))
        && let Some(sym) = s.results.get(s.sel)
    {
        pick = Some(sym.clone());
    }
    if let Some(sym) = results_list(ui, p, s, list_h, w) {
        pick = Some(sym);
    }
    footer(ui, p, w);
    pick
}

/// Title + close glyph; sets `open = false` on click.
fn title_row(ui: &mut Ui, p: &Palette, s: &mut SymbolSearch, w: f32) {
    let (row, _) = ui.allocate_exact_size(vec2(w, TITLE_H), Sense::hover());
    ui.painter().text(
        pos2(row.left() + PAD, row.center().y),
        Align2::LEFT_CENTER,
        "Symbol search",
        font_semibold(20.0),
        p.text,
    );
    let close = Rect::from_center_size(
        pos2(row.right() - PAD - 10.0, row.center().y),
        vec2(20.0, 20.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(close)
            .layout(Layout::centered_and_justified(Direction::TopDown)),
    );
    let (rect, resp) = child.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), 13.0, p.hover);
    }
    let color = if resp.hovered() { p.text } else { p.muted };
    Icon::Close.paint(ui.painter(), rect.shrink(3.0), color, 1.5);
    if resp.clicked() {
        s.open = false;
    }
}

/// The query field. Returns `true` when the text changed.
fn input_row(ui: &mut Ui, p: &Palette, s: &mut SymbolSearch, w: f32) -> bool {
    let (row, _) = ui.allocate_exact_size(vec2(w, INPUT_ROW_H), Sense::hover());
    let field = Rect::from_min_max(
        pos2(row.left() + PAD, row.top() + 4.0),
        pos2(row.right() - PAD, row.bottom() - 4.0),
    );
    ui.painter()
        .rect_filled(field, CornerRadius::same(8), p.panel);
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(field.shrink2(vec2(12.0, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    let resp = child.add(
        egui::TextEdit::singleline(&mut s.q)
            .hint_text(
                RichText::new(PLACEHOLDER)
                    .font(font_regular(14.0))
                    .color(p.faint),
            )
            .desired_width(field.width() - 24.0)
            .min_size(vec2(field.width() - 24.0, INPUT_ROW_H - 8.0))
            .align(Align2::LEFT_CENTER)
            .frame(egui::Frame::NONE)
            .text_color(p.text)
            .font(font_regular(14.0)),
    );
    if s.focus_req {
        resp.request_focus();
        s.focus_req = false;
    }
    let focused = resp.has_focus();
    ui.painter().rect_stroke(
        field,
        CornerRadius::same(8),
        Stroke::new(1.0, if focused { p.accent } else { p.hairline }),
        StrokeKind::Inside,
    );
    if focused {
        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
            s.cycle(1);
        }
        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
            s.cycle(-1);
        }
    }
    resp.changed()
}

/// Category chips. Returns the clicked chip.
fn chips_row(ui: &mut Ui, p: &Palette, w: f32, active: Chip) -> Option<Chip> {
    let (row, _) = ui.allocate_exact_size(vec2(w, CHIP_ROW_H), Sense::hover());
    let inner = Rect::from_min_max(
        pos2(row.left() + PAD, row.center().y - PILL_H / 2.0),
        pos2(row.right() - PAD, row.center().y + PILL_H / 2.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 8.0;
    let mut hit: Option<Chip> = None;
    for c in Chip::ALL {
        if pill(&mut child, p, c.label(), c == active, None).clicked() {
            hit = Some(c);
        }
    }
    hit
}

/// The "All sources" exchange dropdown. Returns the picked source (`None` =
/// clear the filter) plus whether the popup is open after this frame.
fn source_dropdown(
    ui: &mut Ui,
    p: &Palette,
    opts: &[String],
    current: Option<&str>,
) -> (Option<Option<String>>, bool) {
    let (row, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), DROPDOWN_ROW_H), Sense::hover());
    let inner = Rect::from_min_max(
        pos2(row.left() + PAD, row.center().y - PILL_H / 2.0),
        pos2(row.right() - PAD, row.center().y + PILL_H / 2.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let label = current.map_or_else(|| "All sources".to_owned(), |s| s.to_owned());
    let resp = pill(
        &mut child,
        p,
        &label,
        current.is_some(),
        Some(Icon::ChevronDown),
    );
    let mut chosen: Option<Option<String>> = None;
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(180.0);
        if widgets::menu_item(ui, p, None, "All sources", None, current.is_none()).clicked() {
            chosen = Some(None);
        }
        for o in opts {
            if widgets::menu_item(ui, p, None, o, None, current == Some(o.as_str())).clicked() {
                chosen = Some(Some(o.clone()));
            }
        }
    });
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    (chosen, open)
}

fn results_list(
    ui: &mut Ui,
    p: &Palette,
    s: &mut SymbolSearch,
    list_h: f32,
    w: f32,
) -> Option<Symbol> {
    let q = s.q.clone();
    let results = s.results.clone();
    let sel = s.sel;
    let mut pick: Option<Symbol> = None;
    ScrollArea::vertical()
        .id_salt("tickview-symbol-search")
        .max_height(list_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(w);
            // Contiguous rows: the theme's 6px item spacing would push the pitch
            // past `ROW_H` and desync the selection ring.
            ui.spacing_mut().item_spacing.y = 0.0;
            if results.is_empty() {
                widgets::empty_state(ui, p, NO_RESULTS_TITLE, NO_RESULTS_BODY);
            }
            for (i, sym) in results.iter().enumerate() {
                if row(ui, p, &q, sym, i == sel) {
                    pick = Some(sym.clone());
                }
            }
        });
    pick
}

/// One result row: avatar, ticker (with accent match), description, and the
/// right-aligned type + exchange cluster.
fn row(ui: &mut Ui, p: &Palette, q: &str, sym: &Symbol, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());

    if resp.hovered() && !selected {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), p.hover);
    }
    if selected {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(8),
            Stroke::new(1.5, p.text),
            StrokeKind::Inside,
        );
    }

    let cy = rect.center().y;
    let avatar_center = pos2(rect.left() + PAD + AVATAR / 2.0, cy);
    ui.painter()
        .circle_filled(avatar_center, AVATAR / 2.0, avatar_color(&sym.ticker));
    let letter = sym
        .ticker
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    ui.painter().text(
        avatar_center,
        Align2::CENTER_CENTER,
        &letter,
        font_semibold(11.0),
        Color32::WHITE,
    );

    let tx = rect.left() + PAD + AVATAR + 8.0;
    let desc_x = tx + text_width(ui, &sym.ticker, &font_regular(16.0)) + 16.0;
    let type_text = type_label(sym.kind);
    let cluster_right = rect.right() - RIGHT_PAD;

    // Ticker first: the description and type label may drop, never the ticker
    // or exchange.
    draw_ticker(ui, p, q, &sym.ticker, tx, cy);

    if !sym.description.is_empty() {
        let avail = (cluster_right - 8.0 - desc_x).max(0.0);
        if avail > 8.0 {
            // Painted text, not a `Label`: a widget here would sit on top of the
            // row and swallow its clicks.
            ui.painter().text(
                pos2(desc_x, cy),
                Align2::LEFT_CENTER,
                truncate(ui, &sym.description, &font_regular(14.0), avail),
                font_regular(14.0),
                p.muted,
            );
        }
    }

    let ex_w = text_width(ui, &sym.exchange, &font_regular(14.0));
    let type_w = text_width(ui, type_text, &font_regular(12.0));
    let ex_x = cluster_right - ex_w;
    if ex_x - 8.0 - type_w > desc_x + 8.0 {
        ui.painter().text(
            pos2(ex_x - 8.0, cy),
            Align2::RIGHT_CENTER,
            type_text,
            font_regular(12.0),
            p.muted,
        );
    }
    ui.painter().text(
        pos2(cluster_right, cy),
        Align2::RIGHT_CENTER,
        &sym.exchange,
        font_regular(14.0),
        p.text,
    );
    resp.clicked()
}

/// Draw the ticker, colouring the matched span with the accent.
fn draw_ticker(ui: &Ui, p: &Palette, q: &str, ticker: &str, x: f32, cy: f32) {
    let font = font_regular(16.0);
    let painter = ui.painter();
    let Some((a, b)) = highlight_spans(q, ticker) else {
        painter.text(pos2(x, cy), Align2::LEFT_CENTER, ticker, font, p.text);
        return;
    };
    let (pre, mid, post) = (&ticker[..a], &ticker[a..b], &ticker[b..]);
    let mut cursor = x;
    if !pre.is_empty() {
        cursor += text_width(ui, pre, &font);
    }
    painter.text(
        pos2(cursor, cy),
        Align2::LEFT_CENTER,
        mid,
        font.clone(),
        p.accent,
    );
    cursor += text_width(ui, mid, &font);
    if !post.is_empty() {
        painter.text(pos2(cursor, cy), Align2::LEFT_CENTER, post, font, p.text);
    }
}

/// Laid-out pixel width of a single-line string.
fn text_width(ui: &Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER)
        .size()
        .x
}

/// Ellipsize `text` so it fits `max_w`, snapping to char boundaries.
fn truncate(ui: &Ui, text: &str, font: &egui::FontId, max_w: f32) -> String {
    if text_width(ui, text, font) <= max_w {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + "…";
        if text_width(ui, &candidate, font) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut out: String = chars[..lo].iter().collect();
    out.push('…');
    out
}

/// A rounded pill button (chips + the sources dropdown).
fn pill(ui: &mut Ui, p: &Palette, label: &str, active: bool, glyph: Option<Icon>) -> Response {
    let font = font_regular(14.0);
    let mut width = 24.0 + text_width(ui, label, &font);
    if glyph.is_some() {
        width += 18.0;
    }
    let (rect, resp) = ui.allocate_exact_size(vec2(width, PILL_H), Sense::click());
    let hover = resp.hovered();
    let fill = if active {
        p.text
    } else if hover {
        p.hover
    } else {
        p.panel_alt
    };
    ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
    let fg = if active { p.bg } else { p.text };
    ui.painter().text(
        pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );
    if let Some(icon) = glyph {
        let ir =
            Rect::from_center_size(pos2(rect.right() - 19.0, rect.center().y), vec2(14.0, 14.0));
        icon.paint(ui.painter(), ir, if active { fg } else { p.muted }, 1.3);
    }
    resp
}

fn footer(ui: &mut Ui, p: &Palette, w: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, FOOTER_H), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, p.panel_alt);
    ui.painter().line_segment(
        [
            pos2(rect.left(), rect.top() + 0.5),
            pos2(rect.right(), rect.top() + 0.5),
        ],
        Stroke::new(1.0, p.hairline),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        FOOTER_TEXT,
        font_regular(12.0),
        p.faint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(ticker: &str) -> Symbol {
        Symbol::new(ticker, "NASDAQ", SymbolKind::Stock, "USD")
    }

    #[test]
    fn chip_kind_mapping() {
        assert_eq!(Chip::All.kinds(), None);
        assert_eq!(
            Chip::Futures.kinds(),
            Some(&[SymbolKind::Futures, SymbolKind::Commodity][..])
        );
        assert_eq!(Chip::Funds.kinds(), Some(&[SymbolKind::Etf][..]));
        assert_eq!(Chip::Crypto.kinds(), Some(&[SymbolKind::Crypto][..]));
        assert_eq!(Chip::default(), Chip::All);
        assert_eq!(Chip::ALL.len(), 7);
    }

    #[test]
    fn cycle_wraps_and_noop_empty() {
        let mut s = SymbolSearch {
            results: vec![sym("A"), sym("B"), sym("C")],
            ..Default::default()
        };
        assert_eq!(s.sel, 0);
        s.cycle(1);
        assert_eq!(s.sel, 1);
        s.cycle(1);
        s.cycle(1);
        assert_eq!(s.sel, 0, "down wraps past the last row");
        s.cycle(-1);
        assert_eq!(s.sel, 2, "up wraps before the first row");

        let mut empty = SymbolSearch::default();
        empty.cycle(1);
        empty.cycle(-3);
        assert_eq!(empty.sel, 0, "no-op when there are no results");
    }

    #[test]
    fn highlight_spans_prefix_and_middle() {
        assert_eq!(highlight_spans("sol", "SOLUSDT"), Some((0, 3)));
        assert_eq!(highlight_spans("sasol", "SASOL LTD"), Some((0, 5)));
        assert_eq!(highlight_spans(" sol ", "SOLUSDT"), Some((0, 3)));
        assert_eq!(highlight_spans("sol", "SASOL LTD"), Some((2, 5)));
        assert_eq!(highlight_spans("xyz", "AAPL"), None);
        assert_eq!(highlight_spans("  ", "AAPL"), None);
    }

    #[test]
    fn type_label_covers_all_kinds() {
        assert_eq!(type_label(SymbolKind::Crypto), "spot crypto");
        assert_eq!(type_label(SymbolKind::Commodity), "futures");
        assert_eq!(type_label(SymbolKind::Futures), "futures");
        assert_eq!(type_label(SymbolKind::Fx), "forex");
        assert_eq!(type_label(SymbolKind::Etf), "etf fund");
        assert_eq!(type_label(SymbolKind::Index), "index");
        assert_eq!(type_label(SymbolKind::Stock), "stock");
        assert_eq!(type_label(SymbolKind::Cfd), "cfd");
        assert_eq!(type_label(SymbolKind::Other), "other");
    }

    #[test]
    fn set_chip_resets_source_and_selection() {
        let mut s = SymbolSearch {
            source: Some("BINANCE".into()),
            sel: 4,
            dropdown_open: true,
            ..Default::default()
        };
        s.set_chip(Chip::Crypto);
        assert_eq!(s.chip, Chip::Crypto);
        assert_eq!(s.source, None);
        assert_eq!(s.sel, 0);
        assert!(!s.dropdown_open);

        s.set_source(Some("KRAKEN".into()));
        s.sel = 2;
        s.set_source(None);
        assert_eq!(s.source, None);
        assert_eq!(s.sel, 0);
    }

    #[test]
    fn refresh_filters_and_clamps_selection() {
        let index = marketdata::default_index();
        let watch = [sym("AAPL")];
        let mut s = SymbolSearch {
            q: "sol".into(),
            sel: 9,
            ..Default::default()
        };
        s.refresh(&index, &watch);
        assert_eq!(s.results[0].ticker, "SOLUSDT");
        assert_eq!(s.sel, s.results.len() - 1, "sel clamped into range");

        s.chip = Chip::Stocks;
        s.q = "app".into();
        s.refresh(&index, &watch);
        assert_eq!(s.results[0].ticker, "AAPL");
        assert!(
            s.results.iter().all(|r| r.kind == SymbolKind::Stock),
            "stocks chip leaked: {:?}",
            s.results
        );
        assert!(
            s.options.iter().any(|o| o == "NASDAQ"),
            "sources cache: {:?}",
            s.options
        );

        s.q = "  ".into();
        s.refresh(&index, &watch);
        assert_eq!(s.results, watch.to_vec(), "empty query shows the watchlist");
        assert!(s.options.is_empty());
    }

    /// Headless dialog driver: one egui context on a fixed 1440x900 viewport,
    /// so pointer and keyboard paths can run without a window system.
    struct Harness {
        ctx: egui::Context,
        t: f64,
        pick: Option<Symbol>,
    }

    fn press(at: egui::Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn tap(key: Key, down: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: down,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Rect of the 1.5px selection ring painted for `sel`.
    fn ring(shapes: &[egui::Shape]) -> Rect {
        shapes
            .iter()
            .find_map(|sh| match sh {
                egui::Shape::Rect(r) if r.stroke.width == 1.5 => Some(r.rect),
                _ => None,
            })
            .expect("selection ring painted")
    }

    impl Harness {
        fn new() -> Self {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(1.0);
            Self {
                ctx,
                t: 0.1,
                pick: None,
            }
        }

        /// `auto_sized` windows report their content on the second pass, so
        /// warm up before reading shapes or sending input.
        fn warm(
            &mut self,
            s: &mut SymbolSearch,
            index: &marketdata::SearchIndex,
            watch: &[Symbol],
        ) -> Vec<egui::Shape> {
            self.send(s, index, watch, vec![]);
            self.send(s, index, watch, vec![])
        }

        fn send(
            &mut self,
            s: &mut SymbolSearch,
            index: &marketdata::SearchIndex,
            watch: &[Symbol],
            events: Vec<egui::Event>,
        ) -> Vec<egui::Shape> {
            let ctx = self.ctx.clone();
            let p = Palette::dark();
            self.t += 0.05;
            let mut pick = None;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_max(
                        egui::Pos2::ZERO,
                        egui::pos2(1440.0, 900.0),
                    )),
                    time: Some(self.t),
                    events,
                    ..Default::default()
                },
                |ui| {
                    if let Some(sym) = show(ui.ctx(), &p, s, index, watch, Some(vec2(0.0, 92.0))) {
                        pick = Some(sym);
                    }
                },
            );
            if pick.is_some() {
                self.pick = pick;
            }
            let shapes = out.shapes.iter().map(|sh| sh.shape.clone()).collect();
            out.textures_delta.clear();
            shapes
        }

        fn click_at(
            &mut self,
            s: &mut SymbolSearch,
            index: &marketdata::SearchIndex,
            watch: &[Symbol],
            at: egui::Pos2,
        ) {
            self.send(s, index, watch, vec![egui::Event::PointerMoved(at)]);
            self.send(s, index, watch, vec![press(at, true)]);
            self.send(s, index, watch, vec![press(at, false)]);
        }

        fn key(
            &mut self,
            s: &mut SymbolSearch,
            index: &marketdata::SearchIndex,
            watch: &[Symbol],
            k: Key,
        ) {
            self.send(s, index, watch, vec![tap(k, true)]);
            self.send(s, index, watch, vec![tap(k, false)]);
        }
    }

    #[test]
    fn clicking_the_row_under_the_ring_picks_its_symbol() {
        let index = marketdata::default_index();
        let expected = index.search_filtered("a", None, None, MAX_RESULTS);
        let mut h = Harness::new();
        let mut s = SymbolSearch {
            open: true,
            q: "a".into(),
            sel: 2,
            ..Default::default()
        };
        s.refresh(&index, &[]);
        let shapes = h.warm(&mut s, &index, &[]);
        let marked = ring(&shapes);
        assert_eq!(marked.height() + 2.0, ROW_H, "ring wraps one row");
        h.click_at(&mut s, &index, &[], marked.center());
        assert_eq!(
            h.pick.map(|sym| sym.ticker),
            Some(expected[2].ticker.clone()),
            "clicking the marked row must pick that row"
        );
        assert!(s.open, "the caller closes the dialog, not `show`");
    }

    #[test]
    fn arrows_move_the_ring_and_enter_picks() {
        let index = marketdata::default_index();
        let expected = index.search_filtered("a", None, None, MAX_RESULTS);
        assert!(expected.len() > 1, "fixture too small");
        let mut h = Harness::new();
        let mut s = SymbolSearch {
            open: true,
            focus_req: true,
            q: "a".into(),
            ..Default::default()
        };
        s.refresh(&index, &[]);
        let first = ring(&h.warm(&mut s, &index, &[])).center().y;
        h.key(&mut s, &index, &[], Key::ArrowDown);
        let second = ring(&h.send(&mut s, &index, &[], vec![])).center().y;
        assert_eq!(s.sel, 1, "down moves the ring");
        assert!(
            (second - first - ROW_H).abs() < 0.5,
            "ring moved one row pitch: {first} -> {second}"
        );
        h.key(&mut s, &index, &[], Key::Enter);
        assert_eq!(
            h.pick.map(|sym| sym.ticker),
            Some(expected[1].ticker.clone()),
            "Enter picks the ringed row"
        );
    }

    #[test]
    fn truncate_ellipsizes_only_when_it_must() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = font_regular(14.0);
            assert_eq!(truncate(ui, "Apple Inc", &font, 400.0), "Apple Inc");
            let tight = truncate(ui, "Apple Inc Common Stock", &font, 60.0);
            assert!(tight.ends_with('…'), "must ellipsize: {tight}");
            assert!(
                text_width(ui, &tight, &font) <= 60.0,
                "ellipsized text still overflows: {tight}"
            );
        });
        out.textures_delta.clear();
    }

    #[test]
    fn avatar_color_is_stable_and_in_set() {
        assert_eq!(avatar_color("BTCUSDT"), avatar_color("BTCUSDT"));
        assert!(AVATAR_COLORS.contains(&avatar_color("AAPL")));
    }

    /// The query TextEdit must own the whole field interior and centre galley
    /// + caret in it, not ride the top of the row.
    #[test]
    fn query_field_caret_is_vertically_centered() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
        let p = Palette::dark();
        let mut s = SymbolSearch {
            open: true,
            focus_req: true,
            q: "AAPL".to_owned(),
            ..Default::default()
        };
        let idx = SearchIndex::default();
        let wl = vec![sym("AAPL")];
        s.refresh(&idx, &wl);
        let mut out = egui::FullOutput::default();
        for i in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0))),
                time: Some(0.1 * f64::from(i + 1)),
                focused: true,
                ..Default::default()
            };
            out = ctx.run_ui(raw, |ui| {
                let _ = show(ui.ctx(), &p, &mut s, &idx, &wl, None);
            });
            out.textures_delta.clear();
        }
        let ime = out
            .platform_output
            .ime
            .expect("focused TextEdit reports IME output");
        // The field is the rounded `p.panel` fill `input_row` paints behind the
        // TextEdit; egui's IME rect is the galley/caret rect, so the field rect
        // is what tells us whether the text is optically centred.
        let field = out
            .shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::Shape::Rect(r)
                    if r.fill == p.panel
                        && r.corner_radius == CornerRadius::same(8)
                        && r.rect.width() > 500.0 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .next()
            .expect("query field background is painted");
        assert!(
            field.height() >= INPUT_ROW_H - 8.0,
            "query field should be 40px tall: {field:?}"
        );
        assert!(
            (ime.cursor_rect.center().y - field.center().y).abs() <= 1.5,
            "caret should sit at the field centre: field={field:?} caret={:?}",
            ime.cursor_rect,
        );
        assert!(
            (ime.rect.center().y - field.center().y).abs() <= 1.5,
            "text should sit at the field centre: field={field:?} text={:?}",
            ime.rect,
        );
        assert!(
            ime.rect.height() >= 12.0,
            "galley should occupy a full text row: {:?}",
            ime.rect,
        );
    }
}
