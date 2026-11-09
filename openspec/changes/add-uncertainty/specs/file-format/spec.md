# file-format

`Scientific` as a column type.

## MODIFIED Requirements

### Requirement: Column schema
A sheet SHALL be able to declare the type of each column with lines of the form `<Column> : <Type>` placed between the `sheet` line and the header row. A type SHALL be a base type (`Int`, `Fraction`, `Num`, `Complex`, `Text`, `Date`, `Time`, `Datetime`, `Bool`) or `Scientific`. A column with no declaration SHALL have its type inferred from its cells. A `Scientific` column MAY be declared with a display format after its type, in brackets: `<Column> : Scientific[±]`, `Scientific[.4]`, `Scientific[e-3]` or those together in that order, `Scientific[±.4e-3]`. With no format, the column displays in the concise form.

#### Scenario: Scientific column
- **GIVEN** a schema line `Length : Scientific` and the cells `10`, `10 ± 0.1` and `10.5(3)`
- **WHEN** the file is parsed
- **THEN** `Length` has type `Scientific`, and its cells are `10` with uncertainty zero, `10 ± 0.1` and `10.5 ± 0.3`

#### Scenario: Plus-minus format
- **GIVEN** a schema line `Length : Scientific[±]`
- **WHEN** the file is parsed and written out
- **THEN** the column has the `±` display format, and the file is written out with the format as it was declared

#### Scenario: Unknown format
- **GIVEN** a schema line `Length : Scientific[x]`
- **WHEN** the file is parsed
- **THEN** an error is reported, naming the formats a `Scientific` column takes

#### Scenario: Schema names a column not in the header
- **WHEN** a schema line declares a column that does not appear in the header and is not a computed column
- **THEN** parsing reports an error
