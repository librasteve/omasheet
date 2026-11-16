# Tasks: a quantity column's unit is in its head

## 1. omx
- [x] 1.1 `v` in a quantity number format; `_` needs it where there is a unit; `v` needs a unit or a kind
- [x] 1.2 `Column::shown_format`, `head_unit` and `head`: the format a column shows by, with a kind's unit filled in

## 2. engine
- [x] 2.1 A lifted column shows numbers; a value that does not convert keeps its unit
- [x] 2.2 Every display path uses the shown format; a value in a sentence keeps its unit
- [x] 2.3 `ColumnSnap.lifted` and the JSON field; printed and Markdown heads name the unit

## 3. app
- [x] 3.1 The head's type line is `Mass[kg]`, and the column is as wide as it needs

## 4. checks
- [x] 4.1 Tests for the flags, the errors, the heads and the rows; `examples/*.expected` regenerated
- [x] 4.2 `docs/format.md`, `docs/app.md`, the skill, the format help and `examples/units.omx`
- [ ] 4.3 Look at it in the running app
