# Proposal: a quantity column's unit is in its head

## Why

A column of quantities repeats its unit on every row: `5kg`, `0.3kg`, `7.50(20)e-3kg`. The column already has one unit, the one it holds its values in, so the repeat is noise and makes the numbers hard to compare by eye. A spreadsheet puts the unit in the column head and leaves the cells as numbers.

## What Changes

- **file-format**: MODIFIED. A `Quantity` column with a unit, one named in its format or one its kind has (`Mass` is `kg`), shows its cells as numbers and says the unit once, in the head, in full: `Mass[kg]`, `Mass[g]`, whatever width that takes.
- **file-format**: ADDED. The flag `v`, first in a quantity's number format after any `±` or `~`, puts the unit in each row as before: `Mass[g; v.1f]` shows `5000.0g`. The head says the unit either way.
- **file-format**: MODIFIED. `_` spaces the unit in the row, so with a unit it needs `v`: `Mass[g; v_.1f]`. `Quantity[; _.10s]`, which has no unit, is as it was. `v` with no unit and no kind is an error.
- **cli / markdown-integration**: MODIFIED. A printed table or a Markdown table names the unit after the column, `Weight [kg]`. A value read in a sentence, `{{ ... }}`, keeps its unit.
- **app**: MODIFIED. The head's type line is `Mass[kg]` for any quantity column with a unit, `v` or not, set a little larger, and the column is as wide as that needs. `[]` after a type still marks another kind of format.
- A value that cannot be put in the column's unit keeps its own, so it is not mistaken for one that can.
- **Breaking, display only**: files, stored values and exports are unchanged; a format `g; _.1f` that named a unit and `_` is now an error and is written `g; v_.1f`.

## Impact

- `omasheet-omx`: `colfmt.rs` (`SciFormat.unit_down`, `QuantityFormat::lifts`), `check.rs` (`Column::shown_format`, `head_unit`, `head`)
- `omasheet-engine`: `quantity.rs`, `eval.rs`, `doc.rs` (`ColumnSnap.lifted`), `format_help.txt`
- `omasheet-md`, `omasheet-ui` (`json.rs`, `Main.qml`)
- `docs/format.md`, `docs/app.md`, `skill/omasheet/SKILL.md`, `examples/*.expected`

## Out of Scope

- Copy and paste from the grid copy the cells as shown, so a lifted column copies numbers
- A `Quantity` column with no unit, which has a unit per row and no head to put one in
