# sheet-format

Units as column types.

## MODIFIED Requirements

### Requirement: Column schema
A table SHALL be able to declare the type of each column with lines of the form
`<Column> : <Type>` placed between the `table` line and the header row. A type
SHALL be a base type (`Int`, `Rational`, `Num`, `Complex`, `Text`, `Date`, `Time`, `DateTime`, `Bool`) or
a unit (which implies `Rational` with that unit). A column with no declaration SHALL
have its type inferred from its cells.

#### Scenario: Typed columns
- **GIVEN** the source
  ```omx
  table Items

  Qty   : Int
  Price : Rational

  Qty | Price
  2   | 19.99
  5   | 0.50
  ```
- **WHEN** the file is parsed
- **THEN** `Items.Qty` has type `Int`, `Items.Price` has type `Rational`, and the first row's `Price` is exactly `1999/100`

#### Scenario: Unit-typed columns
- **GIVEN** the source
  ```omx
  table Runs

  Distance : m
  Time     : s

  Distance | Time
  100      | 10
  200      | 20
  ```
- **WHEN** the file is parsed
- **THEN** `Runs.Distance` has type `Rational<m>` and the first row's `Distance` is `100m`

#### Scenario: Schema names a column not in the header
- **WHEN** a schema line declares a column that does not appear in the header and is not a computed column
- **THEN** parsing reports an error
