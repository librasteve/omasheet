# Proposal: a read-only view is guarded, and a copy can be pasted as a reference

## Why

A `view` reads its rows from a Parquet file and nothing is ever written back to it (D49). The app already refuses an edit of one, but only when the edit is committed: the editor opens, the user types, and the bottom bar says no. And cut, copy and paste carry only text. A formula pasted into another sheet keeps the names it had and breaks, and the cells of a view are plain values with nothing live to bring across, so there is no way to build on a view from another sheet or from a column appended to it.

## What Changes

- **app**: ADDED. A cell edit on a read-only sheet is refused when it is started, not when it is committed, with the reason in the bottom bar. The same for clear, delete and paste. A cut from a read-only sheet is a copy, and the bar says so.
- **app**: ADDED. A copy remembers where it came from. **Paste as Reference** puts a live reference to the copied cells where it is pasted.
  - In data cells it writes a formula for each cell: `= Passengers[Fare; 3]`.
  - In a formula column it sets the column's formula, aligned by row: `Passengers[Fare; *]` for a whole column, `Passengers[Fare; *+5]` where the copied rows start five below the pasted ones, and `Passengers[Fare; 3]` for one cell.
  - From the same sheet the sheet's name is left out.
  - The status bar says what was written.
- **app**: ADDED. A formula copied to another sheet has the names in it qualified with the sheet it came from, so it still reads what it read.
- **app**: MODIFIED. The keys: Paste as Reference is Ctrl+Shift+R, and Paste Transposed moves from Ctrl+Alt+V to Ctrl+Shift+T, beside Paste Values on Ctrl+Shift+V.
- **app**: UNCHANGED. Ctrl+V pastes values and cells as it does, and so do Paste Values and Paste Transposed. What reaches the system clipboard is still tab-separated text, so other programs paste as before.

## Terms

A **decorator** is a formula column appended to a view sheet, such as `Profit := Revenue - Cost` under a `view`.

## Impact

- `omasheet-ui/src/clip.rs`: `Copied` keeps its source sheet, columns, rows and the sheet's read-only state; a reference is made from it
- `omasheet-ui/src/file.rs`, `omasheet-ui/qml/Main.qml`: the guard at the start of an edit; the Paste as Reference command, its menu entry and key
- `omasheet-engine/src/doc.rs`: the reference text for a block, and the names of a formula qualified for another sheet
- `docs/app.md`

## Out of Scope

- Writing anything back to a Parquet file
- Putting references on the system clipboard for other programs; only this app reads them, from the memory beside the clipboard text, as Paste Values does
- References that follow a row through a sort or an insert: a reference is to a row by its place in the source, and says so
