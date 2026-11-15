# numerics

What a number is, and what arithmetic on one gives.

## MODIFIED Requirements

### Requirement: An omasheet type names a Polars dtype

`Integer` SHALL be `Int64`, `Decimal(s)` SHALL be `Decimal(38, s)` with `s` from 0 to 38, bare `Decimal` SHALL be `Decimal(10)`, and `Percent` SHALL be a `Decimal`. Every value of a `Decimal(s)` column SHALL have exactly the scale `s`; the point SHALL NOT move.

#### Scenario: Two values of a column have one scale

- **GIVEN** a column declared `Decimal(2)`
- **WHEN** it holds `19.99` and `5`
- **THEN** both are held at two places, `19.99` and `5.00`

#### Scenario: A bare Decimal is Decimal(10)

- **GIVEN** a column declared `Decimal`
- **WHEN** it holds `0.1`
- **THEN** it is held with ten places

#### Scenario: A million trillion is held

- **GIVEN** a column declared `Decimal`
- **WHEN** it holds `1000000000000000000`
- **THEN** it is held exactly, and `* 2` is exact

### Requirement: A result is rounded to its scale or is an error

An arithmetic result of `Decimal` SHALL be rounded to the scale that the type of the operation gives it, half away from zero. A result that needs more than 38 digits at that scale SHALL be an error cell saying it does not fit and naming `Fraction`.

#### Scenario: A quotient is rounded

- **GIVEN** two `Decimal(2)` values `1.00` and `3.00`
- **WHEN** `1.00 / 3.00` is calculated
- **THEN** the result is `0.33`

#### Scenario: A tie goes away from zero

- **GIVEN** `Decimal(2)` values
- **WHEN** `0.05 / 2` and `-0.05 / 2` are calculated at two places
- **THEN** the results are `0.03` and `-0.03`

#### Scenario: A result too large is an error

- **GIVEN** a `Decimal(10)` column holding `100000000000000000000`
- **WHEN** `Big * Big` is calculated
- **THEN** the cell is an error saying the result does not fit `Decimal(10)` and naming `Fraction`

### Requirement: Mixed scales

`Decimal(a)` beside `Decimal(b)` SHALL give `Decimal(max(a, b))`; `Integer` SHALL count as `Decimal(0)`; `/` and `mean` SHALL take the scale of the left operand; `Decimal` beside `Fraction` SHALL give `Fraction`; `Fraction` beside `Float` SHALL give `Float`.

#### Scenario: Two scales add exactly

- **GIVEN** a `Decimal(2)` value `1.25` and a `Decimal(4)` value `0.0001`
- **WHEN** they are added
- **THEN** the result is the `Decimal(4)` value `1.2501`

### Requirement: A declared column keeps what it declares

A cell with more places than the column's scale SHALL be refused, naming a `Decimal(n)` or `Fraction` that would hold it, and SHALL NOT be rounded.

#### Scenario: Too many places

- **GIVEN** a column declared `Decimal(2)`
- **WHEN** a cell holds `0.125`
- **THEN** the cell is refused, saying `Decimal(3)` or `Fraction` would hold it

## ADDED Requirements

### Requirement: Fraction

`Fraction` SHALL be an exact rational of arbitrary-precision integers, always reduced and with a positive denominator, so that two equal values compare equal. `+ - * /` SHALL be exact. A function with no rational result (`sqrt`, `ln`, `exp`, the trigonometric functions, a fractional power) SHALL give a `Float`.

#### Scenario: A third is exact

- **GIVEN** `x` a `Fraction` equal to `1/3`
- **WHEN** `x * 3 == 1` is calculated
- **THEN** it is true

#### Scenario: A googol and its reciprocal

- **GIVEN** a `Fraction` column
- **WHEN** it holds `10 ** 100` and `1 / 10 ** 80`
- **THEN** both are held exactly and `(10 ** 100) * (1 / 10 ** 80)` is `10 ** 20`

#### Scenario: An irrational function

- **GIVEN** a `Fraction` value `1/2`
- **WHEN** `sqrt(1/2)` is calculated
- **THEN** the result is a `Float`

### Requirement: A Fraction beside a Decimal

A `Decimal` SHALL convert to a `Fraction` exactly, and a `Fraction` SHALL convert to a `Decimal(s)` only by an explicit conversion that rounds to `s`.

#### Scenario: An explicit narrowing

- **GIVEN** a `Fraction` value `1/3`
- **WHEN** `x.Decimal(4)` is calculated
- **THEN** the result is `0.3333`
