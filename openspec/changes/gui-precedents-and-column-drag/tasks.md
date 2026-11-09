# Tasks: GUI: precedents lit by F2, and dragging columns

## 1. Precedents (`app`)
- [ ] 1.1 Read a formula's references in order, each with its span in the text and the cells or columns it names, for a cell formula and a `:=` column formula
- [ ] 1.2 A colour for each reference, from the theme's palette, repeating after the last; legible in the light and dark themes
- [ ] 1.3 Colour each reference in the entry box, and its cells on the grid, while a formula is edited
- [ ] 1.4 Follow the text as it is typed; clear it when editing ends or is cancelled
- [ ] 1.5 Cells picked by arrow or click keep their present highlight, and take the next colour
- [ ] 1.6 Other sheets and whole columns, as settled in the open questions
- [ ] 1.7 A formula that does not read: colour what does

## 2. Dragging columns
- [ ] 2.1 A `DragHandler` on the column header, as the sheet tabs have one, not starting from the popup circle
- [ ] 2.2 A landing mark between columns while dragging, and the grid scrolls at its edges
- [ ] 2.3 A selection of several columns moves as a block
- [ ] 2.4 Drop calls `moveColumns`; a refusal gives its notice and moves nothing
- [ ] 2.5 One undo step; the selection follows the columns

## 3. Docs and verify
- [ ] 3.1 `docs/app.md`
- [ ] 3.2 Run the app: F2 on `= [A; *-3] + [B; *]` lights two cells in two colours; drag a column across three others and undo
