# omx-expressions

## ADDED Requirements

### Requirement: The maths functions cast to Float

`sin`, `cos`, `tan`, their inverse and hyperbolic forms, `exp`, `ln`, `log10` and `log2` SHALL cast an argument that is not a `Complex` to `Float` before they are applied, accepting exactly what `x.Float` accepts: any number type, a `Boolean`, text that reads as a number as a cell does, and a `Complex` whose imaginary part is zero to within `1e-15`. They SHALL give a `Float`. A value that `x.Float` refuses SHALL be an error naming the function. An empty cell SHALL give an empty result. `exp` and `ln` of a `Complex` SHALL stay `Complex`.

#### Scenario: A Boolean

- **WHEN** `sin(true)` is evaluated
- **THEN** it is `0.8414709848078965`, as `sin(1)` is

#### Scenario: Text that reads as a number

- **WHEN** `ln('20')` is evaluated
- **THEN** it is `2.995732273553991`
- **AND** `ln('abc')` is an error

#### Scenario: A real Complex

- **WHEN** `sin(3+0i)` is evaluated
- **THEN** it is `0.1411200080598672`
- **AND** `sin(3+4i)` is an error naming `sin`

#### Scenario: A Complex stays Complex

- **WHEN** `exp(3+0i)` is evaluated
- **THEN** it is `20.085536923187668+0i`

#### Scenario: A date

- **WHEN** `sin(2025-01-01)` is evaluated
- **THEN** it is an error that says `sin` cannot be applied to a Date

#### Scenario: A column of text

- **GIVEN** a column holding the text `1`, `2` and an empty cell
- **WHEN** `sin` is applied to it
- **THEN** it gives two `Float`s and an empty cell
