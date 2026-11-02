# The app and the command line

## The app

```sh
omasheet                # a new file
omasheet file.omx       # open a file, an XLSX Workbook, a CSV file or Parquet
```

`omasheet` with no command opens the app. Needs Qt 6 (`qt6-base`, `qt6-declarative`). The text is the file: every edit
rewrites the text, and saving writes exactly that.

- One tab per sheet, plus the constants.
- Two sheets can be shown [side by side](#two-sheets-side-by-side), each in
  a window of its own.
- The box at the top shows the source of the current cell; the grid shows
  the value.
- Under its name a column shows its type: muted if it comes from the
  cells, plain if it is declared, and after `:=` in a formula column.
  Right-click a column's header to **Set type…**, which starts on the type
  the column has now, or **Clear type**; a cell that does not fit a
  declared type is an error.
- F3 shows the source of the whole file, to read or to edit. An edit is
  taken when the source is left, saved or closed, and undone as one step.
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
- A number is shown in at most 23 characters: plain from `1e-15` to `1e15`,
  `19.99` or `1/3`, and in e notation otherwise, `1e+60`. One with more
  digits than fit is cut and marked, `1.8446744073709551…e+19`, and a
  `Percent` is rounded, `41.67%`; pointing at it shows the value in full, as
  does pointing at any text cut off at the cell's edge. A plain number of
  five digits or more is grouped in threes, and numbers follow your locale:
  `12,345.678` in Britain, `12.345,678` in Germany. The file does not: a
  number is typed as a file writes it, `12345.678`, or `12_345.678` to group
  it, and copying a cell copies it that way.
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
| Ctrl+E | Export as an XLSX Workbook, CSV or JSON |
| Ctrl+K | New constant |
| Ctrl+N | New sheet after this one |
| Ctrl+Shift+N | New window on this file |
| Ctrl+Alt+N | New file |
| F1 | View functions |
| F2 | Edit |
| F3 | View source |
| F11 | Fullscreen |

### Sorting and filtering

The circle at the right of a column's header opens its popup: **Sort
Ascending**, **Sort Descending**, and the column's filter. A filter is an
expression in which `*` is the cell, such as `* >= 3`; the rows it is not
true of are hidden. **Apply**, lit while the expression is valid and not
yet applied, applies it and leaves the popup open, to try another; **OK** or
Enter applies it and closes. **Clear filter** removes it and leaves the popup
open. It is checked as it is
typed, before anything is read: the box turns red while it is wrong, and
holding the pointer over it says why. So the popup opens at once on a table
of millions of rows. The circle is solid
while the column is filtered, and filters on several columns all apply.

| Filter | Shows the rows where the cell |
|---|---|
| `* == 3`, `* != 'x'`, `* < 2025-01-01` | is, is not, or is before that |
| `0 < * <= 10` | is in the range: comparisons chain |
| `* in [1, 2]`, `*.is_in(['UK', 'US'])`, `* in 1..5` | is one of those |
| `*.starts_with('A')`, `*.starts_with('A'..'C')` | starts so, or with a letter A to C |
| `*.ends_with('son')`, `*.contains('an')` | ends so, or holds it, exactly as written |
| `*.matches('^A.*n$')` | matches the regular expression |
| `*.to_lowercase().contains('an')` | holds it, case aside |
| `*.len_chars() > 3` | is longer than three characters |
| `* == ''`, `*.is_null()`, `* != ''` | is empty, or is not |
| `*.is_nan()`, `*.is_finite()` | holds NaN, or a finite number |
| `*.year == 2025`, `*.weekday >= 6` | falls in 2025, or on a weekend |
| `*.is_duplicated()`, `*.is_first_distinct()` | is in the column more than once, or is the first of its value |
| `* > 10 and * != 15`, `not *.contains('x')` | both, or not |

A filter compares values, not what is shown, so `* == 3` finds a cell shown
as `3.00`. An empty cell fails every comparison: `* != 3` hides empty cells
too, and `* != 3 or * == ''` keeps them. On a table every filter is one
Polars pass over the column. The [expressions](expressions.md#functions)
lists the functions.

A filter is a view and nothing more: the file, a save and an export still
hold every row. It is forgotten with the file.

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
| `--exact` | show every number in full: `18446744073709551616`, not `1.8446744073709551…e+19`, and `125/3%`, not `41.67%` |

From the source tree, `omasheet` is `cargo run --`.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
