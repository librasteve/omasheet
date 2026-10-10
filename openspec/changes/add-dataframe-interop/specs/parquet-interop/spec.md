# parquet-interop

Parquet is the compatibility format for dataframe tools: one table of typed
columns.

## ADDED Requirements

### Requirement: Parquet is not the native model
Parquet import and export SHALL be conversions at the boundary, like XLSX and
CSV. A Parquet file holds the values and column types of one table: it SHALL NOT
carry formulas, computed-column definitions, constants or functions.

#### Scenario: Export does not replace the source
- **WHEN** a sheet is exported to Parquet
- **THEN** the `.omx` file is unchanged and remains the source of truth

### Requirement: Export to Parquet
Exporting SHALL write one Parquet file per table, with one column per table
column under the same name, and computed columns and formula cells written as
their values. A sheet with one table SHALL be written to `<name>.parquet`; a
sheet with several SHALL be written to `<name>.<Table>.parquet` for each, unless
one table is chosen. An empty cell SHALL be written as null.

#### Scenario: One table
- **GIVEN** `budget.omx` with the single table `Sales`
- **WHEN** it is exported to Parquet
- **THEN** `budget.parquet` is written with the columns `Month`, `Revenue`, `Cost`, `Profit`

#### Scenario: Several tables
- **GIVEN** `budget.omx` with tables `Sales` and `Summary`
- **WHEN** it is exported to Parquet
- **THEN** `budget.Sales.parquet` and `budget.Summary.parquet` are written

### Requirement: Column types in Parquet
Each column SHALL be written with the Parquet type that holds its values, chosen
from the values of the whole column:

| Omasheet | Parquet | When it does not fit |
|---|---|---|
| `Int` | 64-bit integer | decimal of scale 0 up to 38 digits; beyond that, string |
| `Ratio` | decimal, with the scale the column needs | double, when a value does not terminate or needs more than 38 digits |
| `Num` | double | |
| `Complex` | struct of doubles `re`, `im` | |
| `Text` | string | |
| `Bool` | boolean | |
| `Date` | date | |
| `Time` | time of day | |
| `DateTime` | timestamp, in the sheet's time zone when it names one, otherwise with no zone | |

A column whose values are not all of one type SHALL be written as strings. A
cell that could not be calculated SHALL be written as null. The export SHALL
report each column written as something less exact than its values, and each
cell written as null because it failed.

#### Scenario: Exact decimals
- **GIVEN** a `Ratio` column holding `19.99` and `5.5`
- **WHEN** it is exported
- **THEN** the column is a decimal of scale 2 holding exactly `19.99` and `5.50`, and no notice is given

#### Scenario: One third
- **GIVEN** a `Ratio` column holding `1/3`
- **WHEN** it is exported
- **THEN** the column is a double and a notice names the column as written in floating point

#### Scenario: Large integer
- **GIVEN** an `Int` column holding `2 ** 100`
- **WHEN** it is exported
- **THEN** the column is a decimal of scale 0 holding `1267650600228229401496703205376`

#### Scenario: Time zone
- **GIVEN** a sheet with `zone Europe/London` and a `DateTime` column
- **WHEN** it is exported
- **THEN** the column is a timestamp in `Europe/London`

### Requirement: Import from Parquet
Importing SHALL produce `.omx` source with one table, named after the file, with
a `Name : Type` line for every column, taken from the file's schema and not
inferred from the values. Integers of any width SHALL become `Int`; decimals
SHALL become `Ratio`, exactly; floating-point columns SHALL become `Num`; a
struct of two doubles `re`, `im` SHALL become `Complex`; null SHALL become an
empty cell. When timestamp columns carry a time zone, the source SHALL begin
with a `zone` line naming it.

A column of a type with no Omasheet equivalent (a list, a map, any other
struct, binary) SHALL be left out, and the import SHALL report it.

#### Scenario: Typed columns
- **GIVEN** `sales.parquet` with a string column `Month` and a decimal column `Revenue`
- **WHEN** `omasheet import sales.parquet` is run
- **THEN** the output contains `table sales` with the lines `Month : Text` and `Revenue : Ratio`

#### Scenario: Floating point stays floating point
- **GIVEN** a double column `Reading`
- **WHEN** it is imported
- **THEN** the column is declared `Reading : Num`

#### Scenario: Nested column
- **GIVEN** a file with a list column `Tags`
- **WHEN** it is imported
- **THEN** the table has no `Tags` column and a notice says it was left out

#### Scenario: Round trip
- **GIVEN** a table whose `Ratio` values all terminate
- **WHEN** it is exported to Parquet and the file is imported
- **THEN** the imported table has the same column types and the same values, exactly

### Requirement: Names on import
Column and file names SHALL be made into OMX identifiers by the rule in
`csv-interop`.

#### Scenario: Column with a space
- **WHEN** a file with the column `Unit Price` is imported
- **THEN** the column is `UnitPrice` and a notice says `Unit Price` was renamed
