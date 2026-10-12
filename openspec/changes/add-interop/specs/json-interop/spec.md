# json-interop

JSON is a compatibility format for the values of a sheet's tables.

## ADDED Requirements

### Requirement: JSON is not the native model
JSON import and export SHALL be conversions at the boundary, like XLSX and CSV.
A JSON file holds the values of tables: it SHALL NOT carry formulas,
computed-column definitions, constants, functions, column types or the sheet's
time zone.

#### Scenario: Export does not replace the source
- **WHEN** a sheet is exported to JSON
- **THEN** the `.omx` file is unchanged and remains the source of truth

### Requirement: JSON export
An export SHALL write one UTF-8 file: an object holding each table by its name,
in the order of the sheet, as a list of rows, each row an object of the row's
values by column name, in the order of the columns. A table chosen by name SHALL
be written as its list of rows alone. Each row SHALL be written on a line of
its own.

An `Int`, a `Decimal` and a `Fraction` SHALL be a JSON number in full, and one
that does not terminate the nearest floating-point number, which SHALL be
reported with its column. A `Percent` SHALL be the number it stands for. A
finite `Num` SHALL be a number, and `Inf`, `-Inf` and `NaN` the strings of those
names. A `Bool` SHALL be `true` or `false`, and `Text` a string. A `Date`, a
`Time`, a `DateTime` and a `Complex` SHALL be a string as a sheet writes it. An
empty cell SHALL be `null`; so SHALL a cell that could not be calculated, which
SHALL be reported with its column.

#### Scenario: Every table
- **WHEN** a sheet with tables `Sales` and `Summary` is exported to JSON
- **THEN** one file is written, an object with the keys `Sales` and `Summary`
- **AND** each holds a list with an object for each row

#### Scenario: One table
- **WHEN** the export names the table `Sales`
- **THEN** the file is the list of the rows of `Sales`

#### Scenario: A value JSON has no number for
- **WHEN** a cell holding `1/0` is exported
- **THEN** it is written as the string `"Inf"`

### Requirement: JSON import
An import SHALL read a list of rows as one table, named after the file, and an
object of such lists as a table for each, named by its key. A row SHALL be an
object of values by column name: the columns SHALL be in the order they are
first met, and a cell SHALL be empty where a row has no such key. Rows that are
all lists SHALL be read with the first as the column names. Any other shape
SHALL be refused, saying what is expected. Names SHALL be made identifiers as
for CSV.

A number SHALL be read as a cell reads its text, every digit of it; `true` and
`false` SHALL be `Bool`, and `null` an empty cell. A string that a cell reads as
a `Date`, a `Time`, a `DateTime`, a `Complex`, `Inf`, `-Inf` or `NaN` SHALL be
that value; any other string SHALL be `Text`, a number in quotes too. A list or
an object inside a row SHALL be kept as the text of its JSON and reported.

#### Scenario: What was exported comes back
- **WHEN** a sheet is exported to JSON and the file is imported
- **THEN** each table has the values it was written from, of the same types,
  but for a `Percent`, which is the number it stands for

#### Scenario: A number in quotes
- **WHEN** a row holds `"42"`
- **THEN** the cell is the text `42`, not a number

#### Scenario: Rows with different keys
- **WHEN** the second row has a key the first has not
- **THEN** it is a further column, empty in the first row
