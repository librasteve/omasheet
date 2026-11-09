# Proposal: GUI: drag and drop columns

## Why

Columns can be moved, but only by cut and paste or a menu. Sheets are dragged to a new place in the tab row, with a landing mark. Columns should be dragged by their headers in the same way.

## What Changes

- **app** — ADDED: a selected column is dragged by its header to a new place (a drag from a header that is not selected still selects along the headers), with a landing mark between columns as the sheet tabs have. A selection of several columns moves as a block. It makes the same edit as `move_columns`, in one undo step. A drop the sheet does not allow moves nothing and says why

## Impact

- `omasheet-ui/qml/Main.qml`: a `DragHandler` on the column header, `beginColumnDrag`, `hoverColumnDrag`, `dropColumns`, and the landing mark in the header row
- `docs/app.md`: a paragraph on moving columns

## Open Questions

- Whether the grid should scroll at its edges while a column is dragged to a place off screen
- Whether the drag fights the grid's own horizontal scrolling, to be seen by hand

## Out of Scope

- Dragging rows
- Starting a drag from the popup circle at the right of a header, which opens the popup
