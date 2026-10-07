# xlsx-interop

How units are exported to XLSX.

## ADDED Requirements

### Requirement: Lossy mapping of units
A quantity SHALL be exported as its magnitude in the column's unit, with the unit
carried in the cell's number format where XLSX can express it (currencies,
percent) and otherwise in the column header.

#### Scenario: Currency column
- **WHEN** a `GBP` column is exported
- **THEN** the cells are numbers with a GBP currency number format

#### Scenario: Physical unit column
- **WHEN** a column `Distance : m` is exported
- **THEN** the cells are plain numbers and the header reads `Distance (m)`
