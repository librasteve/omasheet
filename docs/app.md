# The app and the command line

## The app

```sh
omasheet                # a new file
omasheet file.omx       # open a file, a workbook (.xlsx) or a CSV file
```

`omasheet` with no command opens the app. Needs Qt 6 (`qt6-base`, `qt6-declarative`). The text is the file: every edit
rewrites the text, and saving writes exactly that.

- One tab per sheet, plus the constants.
- Two sheets can be shown [side by side](#two-sheets-side-by-side), each in
  a window of its own.
- The box at the top shows the source of the current cell; the grid shows
  the value.
- Selecting several cells shows their count, sum and average in the footer.
- Beside that box is the selection, written as a formula would write it:

  | Selected | Shown |
  |---|---|
  | One cell | `Sales[Revenue; 3]` |
  | Cells of one column | `Sales[Revenue; 3..5]` |
  | Cells of one row | `Sales[2..3; 3]` |
  | A block of cells | `Sales[2..3; 3..5]` |
  | A whole row, or rows | `Sales[; 3]`, `Sales[; 3..5]` |
  | A whole column, or columns | `Sales[Revenue]`, `Sales[2..3]` |
  | The whole sheet | `Sales` |

  Several columns are given by position, counting from 0.
- Cells that are copied stay highlighted through as many pastes and inserts
  as you like, until Esc or the next copy. Cells that are cut stay where
  they are, highlighted, until they are pasted.
- An exact number is shown in full, `19.99` or `1/3`. A `Num` is shown to
  five digits after the point; `1.41421…` has more, and pointing at it shows
  the value in full.
- Dates and times follow your locale; the file stays ISO.
- A file in another time zone is shown on your clocks.

| Keys | |
|---|---|
| Ctrl+. | Context menu |
| Ctrl+? | Keyboard shortcuts |
| Ctrl++ / Ctrl+- | Zoom in / out (Ctrl+0 resets) |
| Ctrl+Z / Ctrl+Y | Undo / redo |
| Ctrl+Arrows | Go to end |
| Shift+Arrows | Select block |
| Ctrl+Alt+Left/Right | Previous / next sheet |
| Ctrl+C / X / V | Copy / Cut / Paste |
| Ctrl+Shift+V | Paste values: not formulas |
| Ctrl+Alt+V | Paste transposed: rows as columns |
| Ctrl+Enter | Insert row below (Shift: above) |
| Delete | Clear |
| Ctrl+Delete | Delete the selected rows, columns or sheet |
| Ctrl+O / S | Open / save |
| Ctrl+Shift+S | Save as |
| Ctrl+E | Export as a workbook, CSV or JSON |
| Ctrl+K | New constant |
| Ctrl+N | New sheet after this one |
| Ctrl+Shift+N | New window on this file |
| F1 | View functions |
| F2 | Edit in the formula box |
| F3 | View source |
| F11 | Fullscreen |

### Two sheets side by side

1. Open the file and go to the first sheet.
2. Right-click the tab of the second sheet and choose **Open in new window**.
   Or press Ctrl+Shift+N for a second window on the same sheet, and pick the
   other tab there.
3. On Omarchy the new window tiles beside the first. Elsewhere, place the two
   windows next to each other yourself.

Both windows show the same file, so an edit in one shows in the other at
once, and undo and save work from either. Each window has its own sheet,
cursor and text size. Cells cut or copied in one can be pasted in the other.

Closing one window leaves the file open in the other; the last one asks
about unsaved changes. Opening another file, or a new file, changes every
window.

Starting the app twice on the same file is not the same thing: those are two
separate copies, and the one saved last overwrites the other.

## The command line

```sh
omasheet print file.omx               # print the calculated file
omasheet eval '1/3 + 1/6'             # one expression
omasheet eval file.omx 'Sales.Revenue.sum()'
omasheet lint file.omx                # check without calculating
```

| Option | |
|---|---|
| `--locale` | show dates and times as this machine's locale writes them |
| `--now 2025-01-31T09:30` | fix what `today()` and `now()` give |
| `--zone Europe/London` | stand in for this machine's time zone |
| `--exact` | show a `Num` in full, `1.4142135623730951`, not rounded, `1.41421…` |

From the source tree, `omasheet` is `cargo run --`.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
