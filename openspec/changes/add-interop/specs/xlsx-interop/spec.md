# xlsx-interop

XLSX is a compatibility format, not the native format.

## ADDED Requirements

### Requirement: XLSX is not the native model
The native model SHALL be the `.omx` source. XLSX import and export SHALL be
conversions at the boundary, and the system SHALL NOT use XLSX as its internal
representation or as a lossless save format.

#### Scenario: Export does not replace the source
- **WHEN** a sheet is exported to XLSX
- **THEN** the `.omx` file is unchanged and remains the source of truth

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
- **GIVEN** a sheet containing a non-terminating `Fraction`
- **WHEN** it is exported
- **THEN** a notice states that exact rationals were written as floating point

### Requirement: Lossy mapping of exact numbers
An `Int` or `Fraction` that cannot be represented exactly as an XLSX number SHALL be
exported as the nearest double.

#### Scenario: One third
- **WHEN** a cell holding exactly `1/3` is exported
- **THEN** the XLSX cell contains `0.3333333333333333`

### Requirement: Import from XLSX
Importing SHALL produce `.omx` source with one table per worksheet, taking the
first row as the header. Numeric cells SHALL be imported as exact `Fraction` or `Int`
values from their shortest decimal representation. A1-style cell references SHALL
NOT appear in the generated source.

#### Scenario: Simple worksheet
- **GIVEN** a workbook with a worksheet `Sales` whose first row is `Month, Revenue, Cost`
- **WHEN** `omasheet import` is run
- **THEN** the output contains `table Sales` with those columns and one data row per worksheet row

#### Scenario: Decimal cell
- **WHEN** an XLSX cell displaying `19.99` is imported
- **THEN** the `.omx` cell reads `19.99` and evaluates to exactly `1999/100`

### Requirement: Formulas are translated on import
A formula of a worksheet SHALL be imported as an OMX formula where OMX can
express it. A cell reference SHALL become the column it is in and a row of the
table: its own row as the bare column name, another row as an offset from its
own (`[Revenue; *-1]`), and a row held with `$` as that row's index. A reference
to another worksheet SHALL name that worksheet's table. A range within one
column SHALL become a vector, and a range that takes in every row of a column
SHALL become the whole column (`Sales.Revenue`). A1-style references SHALL NOT
appear in the generated source.

A column whose every row holds the same formula, written relative to its own
row, SHALL be imported as a computed column.

A formula SHALL NOT be translated, and its cached value SHALL be imported, when
it uses a function or operator with no OMX equivalent, a defined name, an array
constant, or a cell in the header row or outside the table. A notice SHALL list
those cells by worksheet, with the reason.

Every translated formula SHALL be checked: the generated sheet SHALL be
compiled and calculated, and a formula that the sheet does not accept, or whose
result differs from the workbook's cached value, SHALL be replaced by that
value and listed in a notice. A formula whose result depends on the clock SHALL
be exempt from the comparison. The generated source SHALL always compile.

#### Scenario: A formula on its own row
- **GIVEN** a worksheet `Sales` with headers `Month, Revenue, Cost, Total` and `=B2+C2` in `D2`
- **WHEN** it is imported
- **THEN** the cell reads `= Revenue + Cost`

#### Scenario: A column filled down
- **GIVEN** `=B2-C2`, `=B3-C3` and `=B4-C4` in `D2:D4` of a worksheet with three rows
- **WHEN** it is imported
- **THEN** the column is a computed column with `Profit := Revenue - Cost`

#### Scenario: A running total
- **GIVEN** `=SUM(D$2:D2)` filled down a column
- **WHEN** it is imported
- **THEN** the column's formula is `[Profit; 0..*].sum()`

#### Scenario: Another worksheet
- **GIVEN** `=SUM(Sales!B2:B4)` where `Sales` has three rows
- **WHEN** it is imported
- **THEN** the cell reads `= Sales.Revenue.sum()`

#### Scenario: No equivalent
- **WHEN** a cell holds `=VLOOKUP("Feb",Sales!A2:D4,4,FALSE)`
- **THEN** its cached value is imported
- **AND** a notice names the cell and says that `VLOOKUP` has no equivalent

#### Scenario: A translation that does not agree
- **GIVEN** `=B3+C3` where `C3` is empty and the workbook holds `0.1` for the cell
- **WHEN** it is imported
- **THEN** the cell reads `0.1`, and a notice names it as giving another value than the workbook holds
