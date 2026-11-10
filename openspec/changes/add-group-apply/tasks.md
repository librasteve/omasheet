# Tasks: `apply` on groups

Forthcoming; follows `add-units`.

## 0. Settle
- [ ] 0.1 D-A1 against the registry's naming rules; record the exception there
- [ ] 0.2 The open items in `design.md`

## 1. The stage
- [ ] 1.1 Parse `apply(Fn)` after `group_by`; refuse it elsewhere with an error saying where it goes
- [ ] 1.2 The checker: `Fn` is a function of one sheet parameter; its result type gives the shape (value, vector, sheet)
- [ ] 1.3 The engine: split into groups in key order, call per group, assemble rows with keys

## 2. The result
- [ ] 2.1 One row per group for a value or a vector; prepended keys for a sheet
- [ ] 2.2 Shape and type checks across groups (D-A4, D-A5), errors naming the group
- [ ] 2.3 A failing group is an error in its rows only (D-A8)
- [ ] 2.4 The result is an ordinary calculated sheet: later stages, read-only, exports

## 3. Checks
- [ ] 3.1 Tests: a value, a vector, a sheet, an empty group, an empty key, a failing group, the order
- [ ] 3.2 `apply` and `agg` give the same groups in the same order
- [ ] 3.3 `docs/expressions.md`, `docs/format.md`, the skill, `examples/groups.omx` and its `.expected`
- [ ] 3.4 fmt, clippy -D warnings, workspace tests
