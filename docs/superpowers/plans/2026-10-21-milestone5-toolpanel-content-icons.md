# TickView Milestone 5 — Tool-panel containment, content, icons

Spec: `docs/superpowers/specs/2026-10-20-reference-app-full-parity-design.md` §4/§5
(flyout + rail), `research/reference-app-left-toolbar.md` (live reference structure).
Scope driver: user review of the tool panel on 2026-10-21 (six items).

Builder calls I made (no user input needed):
- Scroll containment gates wheel/keyboard pan whenever the pointer is over an
  open overlay (flyout, utility menu, emoji picker); the rail stays live.
- `Post`/`Idea` (bundle `LineToolTweet`/`LineToolIdea`) become local content-card
  drawings (author/title + body); no social links exist offline.
- The Icons group opens an emoji picker; glyphs render via a bundled outline
  emoji font (Noto Emoji, monochrome — egui/ab_glyph cannot render color-bitmap
  emoji). Placement is a one-anchor `Emoji` drawing storing the char.
- Cursor tools `Dot`/`Arrow`/`Demonstration`/`Magic` are crosshair/pointer
  modes (reference semantics), not drawing kinds, so the drawing manifest is unchanged
  by them. `LineToolImage` stays excluded: no file-picker dependency added.

## Task 1 — Overlay containment + reference section dividers

Files: `crates/tickchart/src/chart.rs`, `crates/app/src/toolpanel.rs`,
`crates/app/src/app.rs`.

- [x] `ChartState.hover_block: Option<Rect>`; `over` (zoom/scroll/keyboard pan,
      ~chart.rs:1146-1177) requires the pointer NOT inside `hover_block`.
- [x] `tool_panel` sets `chart.hover_block` to the open flyout / utility menu
      rect each frame and clears it when no overlay is open (including after a
      scrim click).
- [x] `section_rule` draws its 1 px line edge-to-edge across the flyout
      (extend x by `FLYOUT_PAD`), matching the reference's full-bleed dividers.
- [x] Tests: scroll delta ignored while `hover_block` contains pointer; pans
      otherwise; hover_block=None keeps current behavior.

## Task 2 — Section order, CONTENT subcategory, Post/Idea kinds

Files: `crates/app/src/toolpanel.rs`, `crates/app/src/app.rs`,
`crates/tickchart/src/drawings.rs`, `crates/app/src/icons.rs`,
`docs/parity/catalog.json`.

- [x] Forecasting group: VOLUME-BASED section before MEASURERS (reference order).
- [x] Annotation group: TEXT AND NOTES row order Text, Note, Price note, Pin,
      Table, Callout, Comment, Price label, Signpost, Flag mark; add second
      section `CONTENT` = [Post, Idea].
- [x] `Tool::Post`, `Tool::Idea`; `DrawingKind::Post`, `DrawingKind::Idea` —
      one-anchor text cards (Post: avatar circle + bold first line + body;
      Idea: bold title + body + "CHART IDEA" footer), created by click like
      `Text`, editable via the existing text pipeline.
- [x] Icons `Post` (quote card) and `Idea` (lightbulb-card) glyphs; tool→icon
      and label mappings.
- [x] Catalog: `LineToolTweet`/`LineToolIdea` excluded → verified with notes
      (local content-card equivalent); `xtask parity --write`; broken contract
      tests updated.
- [x] Tests: group_of/order assertions; Post/Idea creation + roundtrip serde.

## Task 3 — Icons group + emoji picker + Emoji drawing

Files: `crates/app/src/toolpanel.rs`, `crates/app/src/app.rs`,
`crates/tickchart/src/drawings.rs`, `crates/app/src/icons.rs`,
`crates/app/assets/fonts/`, `Cargo.toml` (no new crate deps).

- [x] Bundle `NotoEmoji-Regular.ttf` (outline emoji, OFL) into
      `crates/app/assets/fonts/`, register in `theme.rs` font fallback chain.
- [x] 8th rail group `Icons` (default `Tool::Emoji`, new `Icon::Icons` smiley
      glyph) — main click and arrow both open the picker (reference
      `openDropdownByClick`); picker is NOT a tool flyout.
- [x] Picker panel: width 354, height min(679, content−8); category tab row
      (Smileys & People, Animals & Nature, Food & Drink, Activity, Travel &
      Places, Objects, Symbols, Flags) + 8-column emoji grid; clicking an
      emoji sets `pending_emoji`, selects `Tool::Emoji`, closes the picker.
- [x] `DrawingKind::Emoji` — one anchor, char in `text`, painted at
      `16 + style.width * 4` px; hit radius = painted half-size + 4.
- [x] Tests: `group_of(Emoji) == Some(7)`; unique-icon sweep includes new
      glyphs; picker category/emoji counts; emoji drawing serde roundtrip.

## Task 4 — Cursor tools: Dot, Arrow, Demonstration, Magic

Files: `crates/app/src/toolpanel.rs`, `crates/app/src/app.rs`,
`crates/tickchart/src/chart.rs`, `crates/app/src/icons.rs`.

- [x] CURSORS section order: Cross, Dot, Arrow, Demonstration, Magic, Eraser
      (reference app). New `Tool::{DotCursor, ArrowCursor, Demonstration, Magic}` +
      labels + new glyphs (`cursor-dot`, `cursor-arrow`, `demonstration`
      spotlight, `magic` wand).
- [x] Chart behavior: `ArrowCursor` = plain pointer, no crosshair lines;
      `DotCursor` = 5 px dot at pointer, no lines; `Demonstration` = dim
      overlay outside a 140 px circle around the pointer; `Magic` = crosshair
      center snapped to the nearest drawing anchor within 14 px, else normal.
- [x] Tests: each mode's crosshair decision (lines suppressed/dot/spotlight
      rects/magic snap picks nearest anchor).

## Task 5 — QA screenshots, parity regeneration, docs

- [x] New scenes: `flyouttrend`, `flyoutfc`, `flyoutannot`, `emojipicker`,
      `cursorsdemo` (open the relevant overlay/seed emoji + Post/Idea
      drawings); capture dark+light on settled frames.
- [x] Visually verify: dividers visible edge-to-edge, volume-before-measurers,
      CONTENT section, emoji grid legible, cursor modes.
- [x] `PARITY.md` rail/tool rows + README M5 paragraph; notes in
      `docs/evidence/tv/drawings-catalog-notes.md` for Post/Idea/emoji
      equivalents; `xtask parity --write` + validate; full gates
      (`fmt`, `test`, `clippy`, `git diff --check`).

## Verification

Every task: focused `cargo test -p tickchart -p tickview` after edits;
final full workspace gates in Task 5; visual proof via the new scene shots.
