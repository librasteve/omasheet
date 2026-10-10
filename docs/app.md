# The app and the command line

## The app

```sh
cargo run -p omasheet-ui -- sheet.omx
```

Needs Qt 6 (`qt6-base`, `qt6-declarative`). The file is the sheet: every edit
rewrites the text, and saving writes exactly that.

- One tab per table, plus the constants.
- Two tables can be shown [side by side](#two-tables-side-by-side), each in
  a window of its own.
- The box at the top shows the source of the current cell; the grid shows
  the value.
- Selecting several cells shows their count, sum and average in the footer.
- An exact number is shown in full, `19.99` or `1/3`. A `Num` is shown to
  five digits after the point; `1.41421…` has more, and pointing at it shows
  the value in full.
- Dates and times follow your locale; the file stays ISO.
- A sheet in another time zone is shown on your clocks.

| Keys | |
|---|---|
| Ctrl+Arrows | go to the first / last row or column |
| Enter, typing | edit the cell |
| Ctrl+C / X / V | copy / cut / paste |
| Ctrl+Shift+V | paste values, not formulas |
| Ctrl+Alt+V | paste transposed: rows as columns |
| Ctrl+X on a row or column | pick it up; Ctrl+V on another moves it there: a formula column can go among the data columns |
| Ctrl+Enter | insert a row below (Shift: above) |
| Ctrl+Delete | delete the selected rows |
| Ctrl+Z / Ctrl+Y | undo / redo |
| Ctrl+PgUp / PgDn | previous / next table |
| Ctrl+Shift+N | another window on this sheet |
| Ctrl+O / S, Ctrl+Shift+S | open / save, save as |
| Ctrl+E | export: then X for a workbook, C for the table shown as CSV |
| F1 | functions |
| F2 | view the source |
| Ctrl+? | all shortcuts |

### Two tables side by side

1. Open the sheet and go to the first table.
2. Right-click the tab of the second table and choose **Open in new window**.
   Or press Ctrl+Shift+N for a second window on the same table, and pick the
   other tab there.
3. On Omarchy the new window tiles beside the first. Elsewhere, place the two
   windows next to each other yourself.

Both windows show the same sheet, so an edit in one shows in the other at
once, and undo and save work from either. Each window has its own table,
cursor and text size. Cells cut or copied in one can be pasted in the other.

Closing one window leaves the sheet open in the other; the last one asks
about unsaved changes. Opening another file, or a new sheet, changes every
window.

Starting the app twice on the same file is not the same thing: those are two
separate copies, and the one saved last overwrites the other.

## The command line

```sh
omasheet sheet.omx                    # print the calculated sheet
omasheet eval '1/3 + 1/6'             # one expression
omasheet eval sheet.omx 'Sales.Revenue.sum()'
omasheet lint sheet.omx               # check without calculating
```

| Option | |
|---|---|
| `--locale` | show dates and times as this machine's locale writes them |
| `--now 2025-01-31T09:30` | fix what `today()` and `now()` give |
| `--zone Europe/London` | stand in for this machine's time zone |
| `--exact` | show a `Num` in full, `1.4142135623730951`, not rounded, `1.41421…` |

From the source tree, `omasheet` is `cargo run -p omasheet-cli --`.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
