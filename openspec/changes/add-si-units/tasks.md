# Tasks: SI units

> Stage 1 of Phase 4, built on `dev` against a path dependency on `siquant` (see D228). Every increment ends with a demo.

## 0. siquant
- [x] 0.1 Type ids, the list of quantity types and the scale lookup that a host needs (siquant 0.1.1)

## 1. Literals and arithmetic (`units`)
- [ ] 1.1 The unit suffix and the scale literal in the lexer; the `.` rule (D229, D231, D237)
- [ ] 1.2 `Lit::Quantity`, `S::Quantity(QId)` and `Value::Quantity`
- [ ] 1.3 `+ - * /`, `**`, comparison and `sqrt` and `abs` by siquant; uncertainty propagates
- [ ] 1.4 `to` (D234); a dimensionless result is a number (D235)
- [ ] 1.5 Display with the unit after the number
- Demo: `omasheet eval '5kg + 300g'` is `5.3kg`

## 2. Named column types (`units`, `file-format`)
- [ ] 2.1 `Weight : Mass` and the other named types; a mismatch is an error
- [ ] 2.2 A bare number takes the column unit; `sum`, `mean`, `min`, `max`, `median` (D233, D238)
- [ ] 2.3 `examples/units.omx` and its expected output
- Demo: `omasheet print examples/units.omx`

## 3. Display unit and ± (`units`, `file-format`)
- [ ] 3.1 `Mass[g]` as the column display unit
- [ ] 3.2 `±` on quantities
- Demo: a `Mass[g]` column shows `5000g`

## 4. Interop (`xlsx-interop`)
- [ ] 4.1 CSV and JSON text, import never reads a quantity (D236)
- [ ] 4.2 Parquet struct of value, uncertainty and unit
- [ ] 4.3 XLSX writes text, with a note
- Demo: export and import a units file

## 5. App and docs
- [ ] 5.1 Type menu and unit picker
- [ ] 5.2 `docs/`, `skill/omasheet/SKILL.md`
