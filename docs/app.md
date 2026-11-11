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
  Right-click a column's header to **Set type…**, a list in a box beside the
  column's head, which is marked while the box is open; drag the box by its
  handle. The list starts on the type the column has now, or **Clear type**; a
  cell that does not fit a declared type is an error. Typing in the box
  narrows the list, Tab completes what is typed, Up and Down move along it,
  a click marks an entry, and **Apply**, Enter or a double click takes it
  and leaves the box open to try another; **Close**, Esc or a click
  elsewhere closes it. The types are filed under Numbers, Text, Dates and times and Other, each
  heading opening and closing its group, with a caret beside the search box for
  all of them. A quantity is just `Quantity` here: its kind comes from
  **Set unit…**. Undo and redo (Ctrl+Z, Ctrl+Y) work on what was applied while
  either box is still open, when nothing is typed in its search box.
- **Set unit…** is the same kind of box for the units, filed under SI Base,
  Dimensional, Mechanical, Electromagnetic, Thermodynamic, Radiation and
  Other, each heading opening and closing its group, with a caret beside the
  search box that closes every group or opens them all. It lists `kg`, `N`, `m^2`, or any unit typed,
  `mg` or `kN.m`. It is there for every column: the column
  becomes a column of the kind of quantity the unit is of (`mg` makes it
  `Mass[mg]`), and a type it had before goes.
- The same menu has **Set format…** for a column of numbers, dates, times
  or quantities, which shows its cells in a format without changing them, and
  **Clear format**. It is a box like the others, listing formats the column's
  type takes, `'.2f` for a `Decimal`, `.3g` for a `Float`, `±.3e-3` for a
  `Scientific`, `%d %b %Y` for a `Date`, each with what it shows; a format
  typed is taken as typed, and one that does not read, or that the type does
  not take, is refused under the list, with what to write instead. **No format**
  is first in the list. A `Scientific` shows `10.5(3)` by default, and `±` in
  its format asks for `10.5 ± 0.3`; see
  [Expressions](expressions.md#scientific-numbers).
- The boxes of **Set type…**, **Set unit…** and **Set format…** are kept
  as the filter box is: Down past the last entry goes on to **Apply**, then
  **Close**, Left and Right move between them and Up goes back to the list.
  Drag the handle at the top to move a box and the one at the bottom edge to
  stretch it down. Where a column has a unit set, **Set type…** ticks
  `Quantity`.
- The bar at the foot says what was done, and its text, like the error
  dialogs, can be selected and copied with Ctrl+C. Where a calculation
  would give an uncertain complex number, which Omasheet does not have, the
  value becomes a `Float` and the bar says `uncertain complex not supported`
  in amber while that cell is selected.
- Copying cells puts their values on the clipboard, so another program
  pastes the results; pasting into Omasheet again brings back the formulas.
  To copy a formula as text, select it in the box at the top.
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
  digits than fit is cut and marked, `1.8446744073709551·e+19`. A `Decimal`,
  a `Float` and the parts of a `Complex` are shown to three places after the
  point, `0.333·` and `1.414·`, and a `Percent` is rounded, `41.67%`; pointing at it shows the value in full, as
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
| Ctrl+Shift+T | Paste transposed: rows as columns |
| Ctrl+Shift+R | Paste reference: a live link to the cells copied |
| Ctrl+Enter | Insert row/col after (Shift: before) |
| Delete | Clear |
| Ctrl+Delete | Delete the selected rows, columns or sheet |
| Ctrl+O / S | Open / save |
| Ctrl+Shift+S | Save as |
| Ctrl+E | Export as an XLSX Workbook, CSV or JSON |
| Ctrl+K | New constant |
| Ctrl+Shift+F | Filter this column |
| Ctrl+D | Duplicate sheet |
| Ctrl+N | New sheet |
| Ctrl+Alt+Shift+Left/Right | Move sheet left / right |
| Ctrl+Shift+N | New window on this file |
| Ctrl+Alt+N | New file |
| F1 | View functions |
| F2 | Edit |
| F3 | View source |
| F11 | Fullscreen |

### Read-only views, and pasting a reference

The data columns of a `view` are its Parquet file's, and nothing is written back to it. Typing on one, Enter, Delete and a paste do nothing and say why in the status bar, before an editor opens. A cut from one is a copy. A formula column appended to a view, a decorator, is edited like any other.

A sheet that reads a big file shows the first 1,000 rows at once, with "Showing the first N rows — loading the rest…" at the foot of the grid while its Parquet copy is written. **Copy into the document**, on its tab's menu, copies its rows into the file as a sheet of your own to edit. It refuses a table with formula columns, which would be lost, and one too big to hold.

Ctrl+Shift+R, **Paste reference**, pastes a live link to the cells last copied or cut in the app, not their values (Ctrl+V and Ctrl+Shift+V still paste those):

- into data cells, a formula for each cell: `= Passengers[Fare; 3]`
- into a formula column, its formula, read row by row: `Passengers[Fare; *]` for a column, `Passengers[Fare; *+5]` where the rows copied start five below the rows pasted onto, and `Passengers[Fare; 3]` for one cell
- from the same sheet, without its name: `Fare`, `[Fare; 3]`

The status bar says what was written. A reference to a row of a view sorted in the grid is to the row it has in the file, not the row it is shown in; rows in a sorted order cannot be pasted into a formula column, since the order is not a shift. A reference to a row is to its place, as one written by hand is: if rows are inserted above it in a sheet of the file's own, it reads what is now there. What reaches the system clipboard is still the values as tab-separated text, so another program pastes those: a reference is for a paste into this app.

### Copying, cutting and pasting formulas

A copy and a cut treat a formula differently, as a spreadsheet's do, and a paste on another sheet is no different from one on the same sheet.

A **copy** keeps a formula's relative reads relative: `[A; *-1]` is the row above wherever it lands, and a column named alone, `A`, is that column of its row. A fixed read, a row by number or a sheet by name (`[A; 5]`, `Sales[A; 5]`), is left as it is. What it read inside the block you copied it reads at the copy: copy `A`, `B` and `C = A + B` together and the pasted `C` adds the pasted `A` and `B`, in the columns they landed in. A column given by number, `[0; *]`, is fixed and stays on the first column. What it read outside the block it reads from where it is pasted: copy `C` alone and the pasted one adds the `A` and `B` of the sheet it is on.

A **cut** is a move: every read keeps reading the same cell. Cut `C = A + B` and paste it three rows down and it becomes `= [A; *-3] + [B; *-3]`; on another sheet, `= Sales[A; *] + Sales[B; *]`. A cell inside the block moves with it. A range or a whole column counts as outside whatever you copy or cut.

**Paste transposed** puts the cells down turned about, so an offset has no meaning and cannot be kept. What a formula read inside the block it reads at the turned copy. What it read outside is fixed to the cell it read, as a reference: `= [A; 0]`, or `= Sales[A; 0]` from another sheet. The status bar says nothing about it.

A copy or a cut of cells on a filtered sheet takes the rows that are shown and leaves out those the filter hides; a cut clears only the cells it took, and what was taken is pasted as plain cells, since the rows are not a block the sheet holds.

Pasted into a formula column, a formula is the column's one formula. Paste values pastes what the cells showed, and Paste reference a link to them.

### Sorting and filtering

The circle at the right of a column's header opens its popup: **Sort
Ascending**, **Sort Descending**, and the column's filter. A filter is an
expression in which `*` is the cell, such as `* >= 3`; the rows it is not
true of are hidden. **Apply**, lit while the expression is valid and not
yet applied, applies it and leaves the popup open, to try another. Enter
does the same, and Enter again, with nothing left to apply, closes the
popup. **Close**, Esc or a click outside closes it and leaves the filter as
last applied. **Clear filter** removes it and leaves the popup open. The arrow
keys move between the sort entries, the box, Clear filter and the buttons
(Left and Right between the buttons); in the box they move the caret, and
leave it from the first line going up or the last going down. It is checked
as it is typed, before anything is read: the box turns red while it is
wrong, and holding the pointer over it says why. So the popup opens at once
on a view of millions of rows. The circle is solid while the column is
filtered, and filters on several columns all apply. Ctrl+Shift+F opens the
popup of the column the cursor is in.

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
too, and `* != 3 or * == ''` keeps them. On a view every filter is one
Polars pass over the column. The [expressions](expressions.md#functions)
lists the functions.

A filter is a view and nothing more: the file, a save and an export still
hold every row. It is forgotten with the file.

### Seeing what a formula reads

Press F2 on a cell holding a formula, or type one, and each thing it reads is lit: the reference in the text, `[Revenue; *-1]`, `Sales.Cost`, or a column's name alone, is marked in a colour, and the cell or column it names is outlined in the same colour on the grid. Press on a lit cell and drag to move that reference: it keeps its shape, one cell or a range, and is rewritten in the formula where it stands, so the cells it left are no longer lit. Ctrl or Alt on the press adds another reference instead, as before. Clicking another cell with the caret inside a written reference replaces it. The colours go in the order the references are written, and follow the text as it is changed; they go when the edit ends, and nothing is written to the file. A column's formula, written in the box at the top, is lit too, for the row the cursor is in. A reference to another sheet is lit in the text, and that sheet's tab is marked in the same colour.

### Moving columns

Click a column's header to select it, then drag the header to move it: a mark shows where it will land, and the drop is one step to undo. Held near either edge, the grid scrolls sideways to reach a column off screen. Select several columns first (drag along the headers, or Shift-click) and drag one of them to move them together. Dragging from a header that is not selected selects along the headers instead. The circle at the right of a header opens its popup instead, and does not drag.

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
| `--exact` | show every number in full: `18446744073709551616`, not `1.8446744073709551·e+19`, and `125/3%`, not `41.67%` |

From the source tree, `omasheet` is `cargo run --`.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
