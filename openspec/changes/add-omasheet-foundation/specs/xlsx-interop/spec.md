# xlsx-interop

XLSX is a compatibility format, not the native format.

## ADDED Requirements

### Requirement: XLSX is not the native model
The native model SHALL be the `.sheet` source. XLSX import and export SHALL be
conversions at the boundary, and the system SHALL NOT use XLSX as its internal
representation or as a lossless save format.

#### Scenario: Export does not replace the source
- **WHEN** a sheet is exported to XLSX
- **THEN** the `.sheet` file is unchanged and remains the source of truth

### Requirement: Export to XLSX
Exporting SHALL write one worksheet per table, with a header row of column names
and one row per table row. Data cells SHALL be written as values. Constructs with
no XLSX equivalent SHALL be mapped by the documented lossy rules below, and the
export SHALL report which lossy rules were applied.

#### Scenario: One worksheet per table
- **GIVEN** a sheet with tables `Sales` and `Customers`
- **WHEN** it is exported
- **THEN** the workbook has worksheets `Sales` and `Customers`

#### Scenario: Lossy export is reported
- **GIVEN** a sheet containing a non-terminating `Rat`
- **WHEN** it is exported
- **THEN** a notice states that exact rationals were written as floating point

### Requirement: Lossy mapping of exact numbers
An `Int` or `Rat` that cannot be represented exactly as an XLSX number SHALL be
exported as the nearest double.

#### Scenario: One third
- **WHEN** a cell holding exactly `1/3` is exported
- **THEN** the XLSX cell contains `0.3333333333333333`

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

### Requirement: Lossy mapping of uncertainty
A column of uncertain values SHALL be exported as two adjacent columns: the value
and its uncertainty.

#### Scenario: Uncertain column
- **WHEN** a column `Length : Uncertain<m>` with a cell `10 ± 0.1` is exported
- **THEN** the workbook has columns `Length (m)` containing `10` and `Length ± (m)` containing `0.1`

### Requirement: Import from XLSX
Importing SHALL produce `.sheet` source with one table per worksheet, taking the
first row as the header. Numeric cells SHALL be imported as exact `Rat` or `Int`
values from their shortest decimal representation. A1-style cell references SHALL
NOT appear in the generated source.

#### Scenario: Simple worksheet
- **GIVEN** a workbook with a worksheet `Sales` whose first row is `Month, Revenue, Cost`
- **WHEN** `omasheet import` is run
- **THEN** the output contains `table Sales` with those columns and one data row per worksheet row

#### Scenario: Decimal cell
- **WHEN** an XLSX cell displaying `19.99` is imported
- **THEN** the `.sheet` cell reads `19.99` and evaluates to exactly `1999/100`

#### Scenario: Formula cell
- **WHEN** a worksheet cell contains an Excel formula
- **THEN** its cached value is imported and a notice lists the cells whose formulas were not translated
