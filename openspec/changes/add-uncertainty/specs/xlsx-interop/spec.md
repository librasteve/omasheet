# xlsx-interop

How uncertain values are exported to XLSX.

## ADDED Requirements

### Requirement: Lossy mapping of uncertainty
A column of uncertain values SHALL be exported as two columns: the values, in place, with no formulas, and the uncertainties in a column of its own after the last column of the sheet, headed with the column's name and ` ±`, so that no formula's reference moves. Import SHALL NOT read the pair back as one uncertain column.

#### Scenario: Uncertain column
- **WHEN** a column `Length : Uncertain` with a cell `10 ± 0.1` is exported
- **THEN** the worksheet has a column `Length` containing `10` and, after its last column, `Length ±` containing `0.1`
