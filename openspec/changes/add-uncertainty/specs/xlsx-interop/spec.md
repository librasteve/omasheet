# xlsx-interop

How uncertain values are exported to XLSX.

## ADDED Requirements

### Requirement: Lossy mapping of uncertainty
A column of uncertain values SHALL be exported as two adjacent columns: the value
and its uncertainty.

#### Scenario: Uncertain column
- **WHEN** a column `Length : Uncertain<m>` with a cell `10 ± 0.1` is exported
- **THEN** the workbook has columns `Length (m)` containing `10` and `Length ± (m)` containing `0.1`
