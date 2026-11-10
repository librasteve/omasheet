# xlsx-interop

How uncertain values are exported to XLSX.

## ADDED Requirements

### Requirement: Uncertainty is dropped
An uncertain value SHALL be exported as its value, a plain number, and its uncertainty SHALL be dropped, with a note naming the column. Import has no uncertain values to read.

#### Scenario: Scientific column
- **WHEN** a column `Length : Scientific` with a cell `10 ± 0.1` is exported
- **THEN** the worksheet has the one column `Length` containing `10`, and the notes say that uncertainties were dropped
