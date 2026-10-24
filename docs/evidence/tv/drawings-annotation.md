# Annotation family — TV evidence

Source: TradingView 3.4.1 left toolbar (captured 2026-10-20) + TV docs
(<https://www.tradingview.com/support/solutions/43000501972-notes/>).
Golden harness: `crates/tickchart/tests/drawing_goldens.rs`
(`run_annotation_family`).

## Anchors + mapping (TV catalog = TickView, all confirmed)

| Kind | TV id | Anchors | TV interaction |
|---|---|---|---|
| Text | `LineToolText` | 1 | click + type (plain text, no bubble) |
| Note | `LineToolTextNote` | 1 | click + type (bubble note) |
| PriceNote | `LineToolPriceNote` | 1 | horizontal price line + label |
| Pin | `LineToolNote` | 1 | map-pin glyph |
| Callout | `LineToolCallout` | 1 | bubble with tail |
| Comment | `LineToolComment` | 1 | comment bubble |
| PriceLabel | `LineToolPriceLabel` | 1 | axis-style price tag |
| Signpost | `LineToolSignpost` | 1 | flag-on-pole glyph |
| FlagMark | `LineToolFlagMark` | 1 | flag glyph |

TV ships **two distinct single-anchor text tools**: plain `Text` (bare glyph
run at the anchor) and `Note` (rounded bubble). TickView previously collapsed
both into the bubble `Note`; split here — new `DrawingKind::Text` /
`DrawTool::Text` / `Tool::Text` paint plain text, new `Tool::Note` maps to the
existing bubble kind. Toolbar order matches TV: Text, Note, Price note, Pin,
Callout, Comment, Price label, Signpost, Flag mark.

`LineToolTable` is audited in Task 13; `LineToolImage`/`LineToolTweet`/
`LineToolIdea` are excluded media tools.

## Selection semantics pinned

- Every annotation selects on its rendered bounds: the text/bubble rect
  (`note_rect`) for Text/Note/PriceNote/Callout/Comment/PriceLabel; the glyph
  footprint for Pin/Signpost/FlagMark. Interior + corners hit; 22–24 px
  outside misses.

## Tests

- `text_kind_is_plain_and_note_kind_bubbles` — both 1 anchor, both
  annotations, labels "Text"/"Note".
- `annotation_family_geometry_hits_and_misses` / `..._holds_on_log_axis` —
  bounds interior/corners hit + exterior miss for all 9 kinds.
- App side: `every_rail_tool_maps_to_a_chart_tool` pins
  `Tool::Text→DrawTool::Text`, `Tool::Note→DrawTool::Note`;
  `icon_names_are_unique` covers the new `note` glyph (139 icons).

Visual: `docs/screenshots/parity/annot-dark.png`
(`TICKVIEW_SCENE=annot`: plain text, selected note bubble, price note, pin).
