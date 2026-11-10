# Proposal: GUI: F2 lights up the predecessors

## Why

A formula such as `= [A; *-3] + Sales[B; *]` names cells by position, and nothing on screen says which cells those are. The app already outlines the cells a formula refers to while it is typed, but in one colour, so with two references there is no telling which is which.

## What Changes

- **app** — MODIFIED: pressing F2 on a cell with a formula (or typing one) colours each reference it holds, in the formula text and on the grid, one colour per reference, the cell or range in the same colour as its text. The colours follow the text as it is edited and go when editing ends
- References found: `[cols; rows]` selections, this sheet's whole column (`Sales.Cost`), and a column's name alone (this row's cell in it). A reference to another sheet is lit in the text and marks that sheet's tab. A column's formula, which has no leading `=`, is read the same way. Quoted text is not read for references

- **app** — ADDED: a coloured reference is grabbed on the grid and dragged to other cells: the press on a cell a `[cols; rows]` reference reads takes hold of that reference, and the drag moves it, keeping its shape (one cell, or a range), by rewriting it in the formula where it stands. The old cells are gone from the text and from the grid, and the colour stays with the reference. Ctrl or Alt on the press asks for another reference as before
- **app** — MODIFIED: a click on a cell, with the caret inside a reference already written, replaces that reference with the cell's

## Impact

- `omasheet-ui/qml/Main.qml`: `refsIn` (now returns each reference's span and colour), `refColorAt`, a `RefMarks` component in the entry box and the cell editor, and the cell highlight
- `docs/app.md`: a section on seeing what a formula reads

## Out of Scope

- Dependents: lighting the cells that read the one being edited
- Colours in the file: this is a display of what the formula says, and nothing is written
