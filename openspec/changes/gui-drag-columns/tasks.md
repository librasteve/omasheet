# Tasks: GUI: drag and drop columns

## 1. Dragging (`app`)
- [x] 1.1 Dragging from a selected column's header moves the selection, in the header's own `MouseArea` (a `DragHandler` never started: that `MouseArea` holds the press for drag-selecting along the headers); a click on it still selects that column alone
- [x] 1.2 A landing mark between columns while dragging
- [ ] 1.3 The grid scrolls at its edges while dragging
- [x] 1.4 A selection of several columns moves as a block
- [x] 1.5 Drop calls `moveColumns`; a refusal gives its notice and moves nothing
- [x] 1.6 One undo step; the selection follows the columns

## 2. Docs and verify
- [x] 2.1 `docs/app.md`
- [x] 2.2 Run the app: drag a column across three others, drop it, undo; drag a block of two
