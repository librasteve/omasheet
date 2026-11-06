# Tasks: cut, copy and paste of a block behave as a spreadsheet's do

## 1. One rule (`app`)
- [x] 1.1 *(partly: the rule is one entry per case, not one function — `copied_cell`, `moved_cell`, `carried_by` and `turned_cell` in `doc.rs`, chosen by `Clip`)* One function in `doc.rs` rewrites a formula of a block put down elsewhere, from whether it is a copy or a cut, where each cell of the block goes, the sheets it came from and went to, and whether the block is turned about; `carried`, `qualified`, `moved_cell` and `turned_cell` are its callers or are gone
- [x] 1.2 A copy: an inside read goes to the copy, in the column it landed in (D223); an outside name is kept and an outside offset is as written (D219, D220)
- [x] 1.3 A cut: every read keeps reading its cell, the sheet named when it is another (D221); readers elsewhere follow, as `follow_cut` has it *(on the same sheet; a formula on another sheet that reads a cell cut and moved to a third sheet is not re-pointed)*
- [x] 1.4 A transposed paste: an inside read goes to the turned copy; an outside read is fixed to the cell, with the sheet named when it is another (D222); a fixed read, a range and a column are left as they are

## 2. The paste (`app`)
- [x] 2.1 `Clip::paste` and `Clip::paste_transposed` pass the operation and the origin of the block, from the cut or the copy, for the same sheet and for another
- [x] 2.2 A formula pasted into a formula column is still the column's formula (`paste-as-reference` D214, as for a copy)
- [x] 2.3 No status-bar notice for a mixed paste (D225)

## 2b. Names and the circular-reference check (`engine`)
- [x] 2b.1 The circular-reference check is made by cell for a read that names its cell: a fixed row of its own column is circular only if it is the cell itself, or a cell that reads it back (D227)
- [x] 2b.2 The message *(the calculation names the row; the check names the column)*, and its help no longer says "in the same row" where the read is of another row
- [x] 2b.3 A column name inside a copied block is read in the column it landed in, and a numbered column is left as it is (D226)

## 3. Tests
- [x] 3.1 Each scenario of `specs/app/spec.md`, as a clip test
- [x] 3.1b *(`ccp.omx` is not put back in the corpus: it is the owner's, still being worked on, and has no `.expected`)* The seed-in-the-first-row case, the self-read and the frozen read of `specs/app/spec.md`, in the engine; `examples/ccp.omx` calculates without the false `B → B`, `G → G` and `C → C`, and then gets an `.expected` and is put back in the corpus tests
- [x] 3.2 Copy and cut, the same sheet and another, side by side on one block
- [x] 3.3 The tests that hold D214 for a copy are changed to the new result and say why
- [x] 3.4 The existing `a_block_is_pasted_turned_about`, cut and undo tests still pass

## 4. Docs and checks
- [x] 4.1 `docs/app.md`: the rules, with a copy and a cut side by side on another sheet
- [x] 4.2 `paste-as-reference` D214 and its requirement noted as superseded for a copy
- [x] 4.3 The workspace passes
