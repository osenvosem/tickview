# Table (`LineToolTable`)

Source: the reference app (version 3.4.1, captured 2026-10-20), live inspection.

## Behavior

- Two anchors define the table rect; the reference inserts a default 3x3 grid
  (`tickchart::default_grid()`).
- Settings popover: rows/cols steppers (1..=12, the reference allows larger; clamped)
  and per-cell text fields; cell index = `row * cols + col`, row-major.
- Resizing the grid keeps existing cell texts and pads empties
  (`StyleEdit::Grid` resizes `cells`).
- Empty cells render empty; text wider than the cell is ellipsized
  (`truncate_to_width`).

## Persistence

- New `Drawing` fields `grid: Option<[u8; 2]>` and `cells: Vec<String>`,
  serde-defaulted and skipped when absent - configs saved before this field
  load unchanged (`v2_payload_without_table_fields_loads`).

## Interaction

- Selection hits anywhere inside the table rect (the reference selects the whole table).

## Open questions (validated on the live reference 2026-10-20)

- Cell text is WYSIWYG-editable on-canvas in the reference; our edit UX is the settings
  popover (functionally equivalent, different affordance).
- Header-row styling (bold first row) in the reference not modeled.

## Verification

- `store::tests::v2_payload_without_table_fields_loads` - old payloads load
  with defaults, no data loss.
- `store::tests::table_cells_round_trip` - grid resize + cell edit survive
  JSON round-trip.
- `drawing_goldens::table_case_hits_rect_and_misses_outside` - center,
  corner-inset hits; 15 px outside misses.
- `docs/screenshots/parity/table-dark.png` - 3x3 table with sample cells and
  toolbar.
