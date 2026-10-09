# The app and the command line

## The app

```sh
cargo run -p omasheet-ui -- sheet.omx
```

Needs Qt 6 (`qt6-base`, `qt6-declarative`). The file is the sheet: every edit
rewrites the text, and saving writes exactly that.

- One tab per table, plus the constants.
- The box at the top shows the source of the current cell; the grid shows
  the value.
- Selecting several cells shows their count, sum and average in the footer.
- Dates and times follow your locale; the file stays ISO.
- A sheet in another time zone is shown on your clocks.

| Keys | |
|---|---|
| Enter, F2, typing | edit the cell |
| Ctrl+C / X / V | copy / cut / paste |
| Ctrl+X on a row or column | pick it up; Ctrl+V on another moves it there |
| Ctrl+Enter | insert a row below (Shift: above) |
| Ctrl+Delete | delete the selected rows |
| Ctrl+Z / Ctrl+Y | undo / redo |
| Ctrl+PgUp / PgDn | previous / next table |
| Ctrl+O / S, Ctrl+Shift+S | open / save, save as |
| F1 | functions |
| Ctrl+? | all shortcuts |

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

From the source tree, `omasheet` is `cargo run -p omasheet-cli --`.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
