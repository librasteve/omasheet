# csv-interop

CSV is a compatibility format for one table of values.

## ADDED Requirements

### Requirement: CSV is not the native model
CSV import and export SHALL be conversions at the boundary, like XLSX. A CSV file
holds the values of one table: it SHALL NOT carry formulas, computed-column
definitions, constants, functions, column types or the sheet's time zone.

#### Scenario: Export does not replace the source
- **WHEN** a sheet is exported to CSV
- **THEN** the `.omx` file is unchanged and remains the source of truth

### Requirement: CSV dialect
CSV SHALL be read and written as UTF-8 with comma separators and RFC 4180
quoting: a field containing a comma, a double quote or a line break is enclosed
in double quotes, and a double quote inside it is doubled. A leading byte-order
mark SHALL be ignored on import and SHALL NOT be written on export.

#### Scenario: Quoted field
- **GIVEN** a `Text` cell holding `Smith, J "Jay"`
- **WHEN** it is exported
- **THEN** the field reads `"Smith, J ""Jay"""`

### Requirement: Export to CSV
Exporting SHALL write one CSV file per table: a header row of column names, then
one row per table row, with computed columns and formula cells written as their
values. A sheet with one table SHALL be written to `<name>.csv`; a sheet with
several SHALL be written to `<name>.<Table>.csv` for each, unless one table is
chosen. An empty cell SHALL be written as an empty field.

#### Scenario: One table
- **GIVEN** `budget.omx` with the single table `Sales`
- **WHEN** it is exported to CSV
- **THEN** `budget.csv` is written with the header `Month,Revenue,Cost,Profit`

#### Scenario: Several tables
- **GIVEN** `budget.omx` with tables `Sales` and `Summary`
- **WHEN** it is exported to CSV
- **THEN** `budget.Sales.csv` and `budget.Summary.csv` are written

### Requirement: Values in CSV
Values SHALL be written as the `.omx` file writes them, so that other tools read
them without knowing Omasheet: an `Int` in full, a terminating `Ratio` as its
exact decimal however many digits it needs, dates and times in their ISO forms,
`true` / `false`. A non-terminating `Ratio` SHALL be written as the shortest
decimal of its nearest double, and the export SHALL report that it did so. A cell
that could not be calculated SHALL be written as an empty field and reported.

#### Scenario: Exact decimal
- **WHEN** a cell holding exactly `1999/100` is exported
- **THEN** the field reads `19.99`

#### Scenario: Large integer
- **WHEN** a cell holding `2 ** 100` is exported
- **THEN** the field reads `1267650600228229401496703205376`

#### Scenario: One third
- **WHEN** a cell holding exactly `1/3` is exported
- **THEN** the field reads `0.3333333333333333` and a notice states that exact rationals were written as decimals

### Requirement: Import from CSV
Importing SHALL produce `.omx` source with one table, named after the file,
taking the first row as the header. A field SHALL be read from its text, with
no floating-point step: a number becomes an exact `Int` or `Ratio`, a field in
an ISO date or time form becomes a `Date`, `Time` or `DateTime`, `true` / `false`
become `Bool`, an empty field becomes an empty cell, and anything else becomes
`Text`. A field is read as a cell of a sheet reads it, so `20%`, `1/8` and
`1.5e3` are the numbers a sheet takes them for. A column whose fields agree on a
type other than an exact number or text SHALL be declared with it. A column
whose fields do not agree on a type SHALL be left undeclared, each cell keeping
the type it reads as, and the import SHALL report it.

#### Scenario: Simple file
- **GIVEN** `sales.csv` whose first row is `Month,Revenue,Cost`
- **WHEN** `omasheet import sales.csv` is run
- **THEN** the output contains `table sales` with those columns and one data row per CSV row

#### Scenario: Digits beyond a double
- **WHEN** the field `0.12345678901234567890123` is imported
- **THEN** the cell evaluates to exactly `12345678901234567890123/100000000000000000000000`

#### Scenario: Numbers and text in one column
- **GIVEN** a column `Value` with the fields `35200` and `n/a`
- **WHEN** it is imported
- **THEN** the first cell is the number `35200`, the second the text `n/a`, and a notice names the column

#### Scenario: Text that looks like a formula
- **WHEN** the field `=SUM(A1:A3)` is imported
- **THEN** the cell is the text `=SUM(A1:A3)`, not a formula

#### Scenario: Round trip
- **GIVEN** a table whose cells are all literals and whose `Ratio` values all terminate
- **WHEN** it is exported to CSV and the CSV is imported
- **THEN** the imported table has the same values, exactly

### Requirement: Names on import
A header or file name that is not an OMX identifier SHALL be changed into one,
and a repeated or empty header SHALL be made distinct. The import SHALL report
each name it changed. The same rule SHALL apply to XLSX worksheet and header
names.

#### Scenario: Header with a space
- **WHEN** a file with the header `Unit Price` is imported
- **THEN** the column is `UnitPrice` and a notice says `Unit Price` was renamed
