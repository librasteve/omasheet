# Tasks: a read-only view is guarded, and a copy can be pasted as a reference

## 1. The guard (`app`)
- [ ] 1.1 The snapshot of a sheet says whether it is read-only and why (`Document::read_only`)
- [ ] 1.2 `startEditing`, delete, clear and paste on a read-only sheet do nothing and put the reason in the bottom bar; the editor does not open (D207, D208)
- [ ] 1.3 A cut from a read-only sheet is a copy, and the bar says so (D208)
- [ ] 1.4 Tests: typing on a view cell, Enter, double-click, delete, paste and cut each leave the sheet unchanged and give the message

## 2. References (`app`)
- [ ] 2.1 `Copied` keeps the source sheet, the columns, the rows (in source order for a sorted view, D215) and whether it came from a formula column
- [ ] 2.2 A reference for a block: a cell, a column span, a whole column (D211, D212)
- [ ] 2.3 Paste as Reference into data cells: a formula in each (D211)
- [ ] 2.4 Paste as Reference into a formula column: the column's formula, aligned by row with the offset of D212; one formula for each column of a block
- [ ] 2.5 The sheet's name left out from the same sheet (D213)
- [ ] 2.6 Names in a copied formula qualified for another sheet (D214)
- [ ] 2.7 The status bar says what was written (D216)
- [ ] 2.8 Tests: a view column pasted into a decorator column, into another sheet's formula column and into data cells; rows 5 to 9 onto row 0; one cell; a sorted view; a formula copied across sheets

## 3. The command (`app`)
- [ ] 3.1 Paste as Reference in the cell menu, enabled where a reference can be made, and its key, Ctrl+Shift+R (D217)
- [ ] 3.2 Paste Transposed moves to Ctrl+Shift+T (D218) in `Main.qml`, the cell menu and `docs/app.md`
- [ ] 3.3 `docs/app.md`: the guard, Paste as Reference, the rule for a sorted view, and that a reference does not leave the app

## 4. Checks
- [ ] 4.1 The workspace passes
