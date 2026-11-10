# Proposal: cut, copy and paste of a block behave as a spreadsheet's do

## Why

The cells of a block can be pasted four ways: as they are (formulas), as values, as references and transposed. Each way treats the formulas in the block, and what they read outside it, by its own rule, and the rules do not agree. A copy to another column on the same sheet pastes a formula as written, so a column name in it does not follow the copy, while the same copy to another sheet maps it (D214). A copy to another sheet sends what it read outside the block back to the source, where a spreadsheet would read the sheet it is pasted on. A transposed paste kept a relative offset that means nothing once the cells are turned about, so it read the wrong cell. There is no one rule to explain, test or document.

## What Changes

- **app**: MODIFIED. A copy keeps relative reads relative, on the same sheet and on another (D219, D220). What a formula read inside the block it reads at the copy, in the column it landed in; what it read outside, by name or offset, it reads on the sheet it is pasted on. This reverses D214 for a copy.
- **app**: MODIFIED. A cut is a move (D221). Every read of a formula cut keeps reading the same cell, with its sheet named where that is another, and the formulas elsewhere that read a cell cut follow it. D214 stands for a cut.
- **app**: MODIFIED. A transposed paste, copy or cut, reads inside the block at the turned copy, and fixes what it read outside the block to the cell it read, as a reference (D222).
- **app**: ADDED. Inside a copied block a column name is read in the column it landed in, and a numbered column is fixed (D226).
- **engine**: MODIFIED. A formula that reads a fixed row of its own column is circular only if that row is the cell itself or reads it back; the check is by cell, and the message names it (D227). Without it a frozen read or a pasted reference to the same column is reported as a cycle.
- **app**: UNCHANGED. Paste Values pastes values; Paste Reference pastes references (D211, D212). What reaches the system clipboard is still tab-separated text.

## Impact

- `omasheet-engine/src/doc.rs`: `carried`, `qualified`, `moved_cell` and `turned_cell` become one rule, taking whether the paste is a copy or a cut, and where each cell of the block goes
- `omasheet-ui/src/clip.rs`: `paste`, `paste_transposed` and `put_block` pass the operation and the block's origin to it
- `docs/app.md`
- `openspec/changes/paste-as-reference`: D214 and its requirement are superseded for a copy

## Out of Scope

- Writing anything back to a view
- Paste Values and Paste Reference, which are as `paste-as-reference` has them
- Moving a column name by distance, as a spreadsheet moves a column letter: a column name is a column, and is kept
- A status-bar notice that a paste mixed reads at the copy with reads of the original (D225)
