# file-format

## ADDED Requirements

### Requirement: Decimal and Fraction as column types

A column MAY be declared `Decimal(n)` for `n` from 0 to 38, `Decimal` (meaning `Decimal(10)`) or `Fraction`. The scale of a `Decimal` is the column's.

#### Scenario: A scale is declared

- **GIVEN** `Price : Decimal(2)`
- **WHEN** the file is read and written back
- **THEN** the declaration is kept as written

### Requirement: An undeclared column's type is inferred

An undeclared column of decimal cells SHALL be `Decimal(n)` for the smallest `n` (at most 18) that holds every cell exactly, and SHALL be `Fraction` where none does.

#### Scenario: The smallest scale that holds

- **GIVEN** an undeclared column holding `19.99` and `5`
- **WHEN** it is read
- **THEN** it is `Decimal(2)`

#### Scenario: Too many places for a decimal

- **GIVEN** an undeclared column holding `0.1234567890123456789`
- **WHEN** it is read
- **THEN** it is `Fraction`, and the value is exact

## MODIFIED Requirements

### Requirement: A declared bare Decimal

A column declared bare `Decimal` that holds a cell with more than ten places SHALL give a diagnostic for that cell naming the `Decimal(n)` that would hold it.

#### Scenario: A cell longer than the default holds

- **GIVEN** `Rate : Decimal` and a cell `0.12345678901`
- **WHEN** the file is read
- **THEN** the cell is refused, saying `Decimal(11)` or `Fraction` would hold it
