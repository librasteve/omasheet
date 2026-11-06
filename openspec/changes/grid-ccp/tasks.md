# Tasks: cut, copy and paste of a block behave as a spreadsheet's do

## 1. One rule (`app`)
- [ ] 1.1 One function in `doc.rs` rewrites a formula of a block put down elsewhere, from whether it is a copy or a cut, where each cell of the block goes, the sheets it came from and went to, and whether the block is turned about; `carried`, `qualified`, `moved_cell` and `turned_cell` are its callers or are gone
- [ ] 1.2 A copy: an inside read goes to the copy, in the column it landed in (D223); an outside name is kept and an outside offset is as written (D219, D220)
- [ ] 1.3 A cut: every read keeps reading its cell, the sheet named when it is another (D221); readers elsewhere follow, as `follow_cut` has it
- [ ] 1.4 A transposed paste: an inside read goes to the turned copy; an outside read is fixed to the cell, with the sheet named when it is another (D222); a fixed read, a range and a column are left as they are

## 2. The paste (`app`)
- [ ] 2.1 `Clip::paste` and `Clip::paste_transposed` pass the operation and the origin of the block, from the cut or the copy, for the same sheet and for another
- [ ] 2.2 A formula pasted into a formula column is still the column's formula (`paste-as-reference` D214, as for a copy)
- [ ] 2.3 No status-bar notice for a mixed paste (D225)

## 3. Tests
- [ ] 3.1 Each scenario of `specs/app/spec.md`, as a clip test
- [ ] 3.2 Copy and cut, the same sheet and another, side by side on one block
- [ ] 3.3 The tests that hold D214 for a copy are changed to the new result and say why
- [ ] 3.4 The existing `a_block_is_pasted_turned_about`, cut and undo tests still pass

## 4. Docs and checks
- [ ] 4.1 `docs/app.md`: the table of the rules, with a copy and a cut side by side on another sheet
- [ ] 4.2 `paste-as-reference` D214 and its requirement noted as superseded for a copy
- [ ] 4.3 The workspace passes
