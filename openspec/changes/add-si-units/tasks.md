# Tasks: SI units

> Stage 1 of Phase 4, built on `dev` against a path dependency on `siquant` (see D228). Every increment ends with a demo.

## 0. siquant
- [x] 0.1 Type ids, the list of quantity types and the scale lookup that a host needs (siquant 0.1.1)

## 1. Literals and arithmetic (`units`)
- [x] 1.1 The unit suffix and the scale literal in the lexer; the `.` rule (D229, D231, D237)
- [x] 1.2 `Lit::Quantity`, `S::Quantity(QId)` and `Value::Quantity`
- [x] 1.3 `+ - * /`, `**`, comparison and `sqrt` and `abs` by siquant; uncertainty propagates
- [x] 1.4 `to` (D234); a dimensionless result is a number (D235)
- [x] 1.5 Display with the unit after the number
- Demo: `omasheet eval '5kg + 300g'` is `5.3kg` (done)

## 2. Named column types (`units`, `file-format`)
- [x] 2.1 `Weight : Mass` and the other named types; a mismatch is an error
- [x] 2.2 A bare number takes the column unit; `sum`, `mean`, `min`, `max`, `median` (D233, D238)
- [x] 2.3 `examples/units.omx` and its expected output
- Demo: `omasheet print examples/units.omx` (done)

## 3. Display unit and ± (`units`, `file-format`)
- [x] 3.1 `Mass[g]` as the column display unit
- [x] 3.2 `±` on quantities
- Demo: a `Mass[g]` column shows `5000g` (done)

## 4. Interop (`xlsx-interop`)
- [x] 4.1 CSV and JSON text, import never reads a quantity (D236)
- [ ] 4.2 Parquet struct of value, uncertainty and unit (not done: a column of quantities is written as text, with a note)
- [x] 4.3 XLSX writes text, with a note
- Demo: `omasheet export examples/units.omx --csv` writes `5kg` (done)

## 5. App and docs
- [x] 5.1 Type menu and unit picker
- [x] 5.2 `docs/`, `skill/omasheet/SKILL.md`
