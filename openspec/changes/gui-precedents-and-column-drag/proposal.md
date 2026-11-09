# Proposal: GUI: precedents lit by F2, and dragging columns

## Why

Two small gaps in the grid.

1. **F2 on a formula shows only text.** A formula such as `= [A; *-3] + Sales[B; *]` names cells by position, and nothing on screen says which cells those are. The app already highlights cells while they are *picked* into a formula; it should light up the cells a formula already reads when it is opened for editing.
2. **Columns can be moved, but only by cut and paste or a menu.** Sheets can be dragged to a new place in the tab row, with a landing mark. Columns should be dragged by their headers in the same way.

## What Changes

- **app** — ADDED: pressing F2 on a cell with a formula (or typing in one) colours each reference it holds, in the formula text and on the grid, one colour per reference, the cell or range in the same colour as its text. The colouring follows the text as it is edited and goes when editing ends
- **app** — ADDED: a column is dragged by its header to a new place, with a landing mark between columns as the sheet tabs have. A selection of several columns moves as a block. It makes the same edit as `move_columns` and one undo step

## Impact

- `omasheet-ui/qml/Main.qml`: the entry box and the grid overlays (references picked are already highlighted), the column header, a `DragHandler` beside the one in `tabRow`
- A way to read the references of a formula in order, with their positions in the text, from `omasheet-omx`/`omasheet-engine` to the UI. The picker already writes them; this reads them back
- `docs/app.md`: the editing and column sections, and the keys table if a key changes

## Open Questions

- A reference to another sheet or to a constant: mark the sheet's tab in the same colour, or only colour the text. Suggest the tab
- A reference to a whole column (`Sales.Revenue`): colour the whole column, or its header only. Suggest the header and a light wash down the column
- Dragging columns of a view, or of a sheet of formulas alone: allowed if `move_columns` allows it, otherwise the drag is refused with the same notice

## Out of Scope

- Dependents: lighting the cells that read the one being edited
- Dragging rows
- Colours in the file: this is a display of what the formula says, and nothing is written
