# Tasks: `apply` on groups

Forthcoming; follows `add-units`.

## 0. Settle
- [x] 0.1 D-A1 against the registry's naming rules; record the exception there
- [x] 0.2 The open items in `design.md`

## 1. The stage
- [x] 1.1 Parse `apply(Fn)` after `group_by`; refuse it elsewhere with an error saying where it goes
- [x] 1.2 (a value; a column or a sheet is task 2.1) The checker: `Fn` is a function of one sheet parameter; its result type gives the shape (value, vector, sheet)
- [x] 1.3 The engine: split into groups in key order, call per group, assemble rows with keys

## 2. The result
- [x] 2.1 One row per group for a value or a vector; prepended keys for a sheet
- [x] 2.2 Shape and type checks across groups (D-A4, D-A5), errors naming the group
- [x] 2.3 A failing group is an error in its rows only (D-A8)
- [x] 2.4 The result is an ordinary calculated sheet: later stages, read-only, exports
- [x] 2.5 A function that gives rows of another sheet: paired as a join's rows are (D-A10)

## 3. Checks
- [x] 3.1 Tests: a value, a vector, a sheet, an empty group, an empty key, a failing group, the order
- [x] 3.2 (the engine runs every body; the lowered path is a follow-on) `apply` and `agg` give the same groups in the same order, and a body that lowers gives the same answer, shape, type and error lowered as in the engine (the lowered path follows `widen-polars-lowering`; until then the engine runs every body)
- [x] 3.3 `docs/expressions.md`, `docs/format.md`, the skill, `examples/groups.omx` and its `.expected`, with a good set of `group_by |> apply` examples (a value, a column, a sheet, the top rows of each group, a rank within a group)
- [x] 3.4 fmt, clippy -D warnings, workspace tests

## 4. The lowered path
- [x] 4.1 A function that gives a value lowers with its body: the call is read as the body over the group's columns, and an aggregate of an expression of them is a column of its own for Polars to aggregate in the one `group_by` `agg` already uses; a body or a column it cannot lower, a cell empty in a column it reads, or an operation that might not fit, is the engine's
- [x] 4.2 A function that gives rows of the grouped sheet lowers when it is `sort` (once), `head`, `tail`, a row condition and `select` over the group's rows: the conditions are asked of the whole sheet first, and the rest is one `group_by_stable` with `sort_by` and `head`/`tail` for each group; any other step, or a second `sort`, or a condition after a `head`, is the engine's
- [x] 4.3 The other-sheet pairing lowers when the function picks rows of the other sheet by equality with the group's first row (`Other[; Col == rows[Key; 0]]`, joined with `and`, then `select`): a join of the groups' first rows to the sheet, by the join's own rules, taken by Polars. A rank within a group is a `rank(... by Key)` column, which is already a window function and lowers as one; no `apply` is needed for it
