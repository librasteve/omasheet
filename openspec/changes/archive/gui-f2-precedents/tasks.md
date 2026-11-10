# Tasks: GUI: F2 lights up the predecessors

## 1. Precedents (`app`)
- [x] 1.1 Read a formula's references in order, each with its span in the text and the cells or columns it names, for a cell formula
- [x] 1.2 A colour for each reference, from a palette that repeats after the last
- [x] 1.3 Colour each reference in the entry box and the cell editor, and its cells on the grid
- [x] 1.4 Follow the text as it is typed; clear it when editing ends or is cancelled
- [x] 1.5 Cells picked by arrow or click keep their present highlight, and take the next colour
- [x] 1.6 This sheet's whole columns (`Sales.Cost`) and a column's name alone are lit
- [x] 1.7 Another sheet's references: lit in the text, and that sheet's tab is marked in the same colour
- [x] 1.8 A computed column's formula (and a calculated sheet's), written in the box at the top with no `=`: light its references too, for the row the cursor is in

## 2. Moving a reference
- [x] 2.1 A press on a cell a `[cols; rows]` reference reads takes hold of it (no Ctrl or Alt); the drag rewrites it where it stands, keeping its shape and whether it was relative or absolute
- [x] 2.2 The old cells are dropped from the text, and so from the grid, as the text is rewritten; the reference keeps its colour
- [x] 2.3 A click with the caret inside a written reference replaces it
- [x] 2.4 Only `[cols; rows]` references are grabbed: a bare column name, `Sales.Cost`, or another sheet's reference is not

## 3. Docs and verify
- [x] 3.1 `docs/app.md`
- [x] 3.2 Run the app: F2 on `= [A; *-3] + [B; *]` lights two cells in two colours, and Esc clears them
