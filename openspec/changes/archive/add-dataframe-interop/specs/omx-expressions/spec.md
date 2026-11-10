# omx-expressions

`.head(n)`: the first rows of a sheet.

## ADDED Requirements

### Requirement: `head`

`x.head(n)` SHALL give the first `n` rows of a sheet or a column. It SHALL be
the selection `x[; ^n]`, which the language already has, so it SHALL add no
evaluator behaviour of its own (D32).

`n` SHALL be a whole number. Asking for more rows than there are SHALL give
every row, not an error.

#### Scenario: The first rows of a table

- **GIVEN** a sheet `Sales` of many rows
- **WHEN** `Sales.head(5)` is evaluated
- **THEN** the first five rows are given
- **AND** the result is what `Sales[; ^5]` gives

#### Scenario: More rows than there are

- **GIVEN** a sheet of three rows
- **WHEN** `Sales.head(10)` is evaluated
- **THEN** the three rows are given
- **AND** no diagnostic is reported

#### Scenario: The first rows of one column

- **GIVEN** a column `Revenue`
- **WHEN** `Revenue.head(2)` is evaluated
- **THEN** the first two values are given
