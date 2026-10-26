# file-format

The `.omx` file: its declarations, its sheets and its column schemas.

## MODIFIED Requirements

### Requirement: The declarable type names

A `Name : Type` line SHALL name one of `Int`, `Decimal`, `Fraction`,
`Percent`, `Float`, `String`, `Binary`, `Boolean`, `Date`, `Time`, `Datetime`
or `Duration`.

`Datetime` MAY name a time zone, as `Datetime("Europe/London")`, which is the
zone that column's clocks are in. No other type takes an argument. A zone the
engine cannot find SHALL be a diagnostic, as it is on a `zone` declaration.

`Bool`, `Text`, `Num` and `Complex` SHALL NOT be accepted, and a
file using one SHALL be told the list of names that are.

#### Scenario: An old type name is rejected with the list

- **GIVEN** a column declared `Paid : Bool`
- **WHEN** the file is checked
- **THEN** a diagnostic says `Bool` is not a known type
- **AND** the help lists `Boolean` among the names that are

#### Scenario: A column declares its zone

- **GIVEN** a file in `America/New_York` with a column declared
  `Logged : Datetime("Europe/London")` holding `2025-07-15T21:30`
- **WHEN** the sheet is shown
- **THEN** the cell is shown as `2025-07-15T21:30+01:00`
- **AND** the file still holds it as `2025-07-15T21:30`
- **AND** it is equal to a `Datetime` column holding `2025-07-15T16:30`, the
  same instant on the file's own clocks

#### Scenario: The conversion functions carry the same names

- **GIVEN** the expression `0.5.Boolean`
- **WHEN** it is evaluated
- **THEN** the result is `true`
- **AND** `0.5.Bool` is not a conversion

### Requirement: A blank cell is Null

A cell with nothing in it SHALL be `Null`, which is a value of the `Null`
dtype. A file SHALL continue to write it as nothing at all.

#### Scenario: A blank cell round-trips as nothing

- **GIVEN** a sheet with a blank cell
- **WHEN** the file is printed
- **THEN** the cell is written as nothing
- **AND** reading it back gives `Null`
