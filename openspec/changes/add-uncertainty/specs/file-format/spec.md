# file-format

`Uncertain` as a column type.

## MODIFIED Requirements

### Requirement: Column schema
A sheet SHALL be able to declare the type of each column with lines of the form `<Column> : <Type>` placed between the `sheet` line and the header row. A type SHALL be a base type (`Int`, `Fraction`, `Num`, `Complex`, `Text`, `Date`, `Time`, `Datetime`, `Bool`) or `Uncertain`. A column with no declaration SHALL have its type inferred from its cells. An `Uncertain` column MAY be declared with a display format after its type, in brackets: `<Column> : Uncertain[±]`. `+-` SHALL be accepted as an ASCII spelling of `±`. With no format, the column displays in the concise form.

#### Scenario: Uncertain column
- **GIVEN** a schema line `Length : Uncertain` and the cells `10`, `10 ± 0.1` and `10.5(3)`
- **WHEN** the file is parsed
- **THEN** `Length` has type `Uncertain`, and its cells are `10` with uncertainty zero, `10 ± 0.1` and `10.5 ± 0.3`

#### Scenario: Plus-minus format
- **GIVEN** a schema line `Length : Uncertain[±]`
- **WHEN** the file is parsed and written out
- **THEN** the column has the `±` display format, and the file is written out with the format as it was declared

#### Scenario: Unknown format
- **GIVEN** a schema line `Length : Uncertain[x]`
- **WHEN** the file is parsed
- **THEN** an error is reported, naming the formats an `Uncertain` column takes

#### Scenario: Schema names a column not in the header
- **WHEN** a schema line declares a column that does not appear in the header and is not a computed column
- **THEN** parsing reports an error
