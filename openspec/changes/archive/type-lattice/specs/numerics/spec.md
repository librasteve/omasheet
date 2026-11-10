# numerics

The lattice of the number types.

## MODIFIED Requirements

### Requirement: Joining number types
The join of two number types SHALL be: `Integer` with `Decimal` a `Decimal`;
either with `Float` a `Float`; two of one type that type; two different
members of `Hex`, `Octal`, `Binary` and `Integer` an `Integer`; and two
different members of `Fraction`, `Percent` and `Decimal`, or one of them with
an `Integer`, a `Decimal`. A number joined with text SHALL be of any type.

#### Scenario: Fractions and decimals
- **WHEN** a column holds `1/1` and `1.0`
- **THEN** it is a `Decimal` column

#### Scenario: Fractions alone
- **WHEN** a column holds `1/2` and `2/3`
- **THEN** it is a `Fraction` column

### Requirement: Arithmetic types
The type of `a + b`, `a - b`, `a * b` and `a / b` SHALL be the join of the types
of `a` and `b`, but that a quotient of whole numbers SHALL be a `Decimal`.

#### Scenario: Fractions
- **WHEN** `Yield + Yield` and `Yield * 2` are evaluated over a `Fraction` column
- **THEN** the first is a `Fraction` and the second a `Decimal`

### Requirement: Powers
A power SHALL be exact when its exponent is a whole number and a `Float` when
it is any other.

#### Scenario: A root
- **WHEN** `2 ** 0.5` is evaluated
- **THEN** it is the `Float` 1.4142135623730951

### Requirement: Fraction literals in a formula
A fraction SHALL be written in a formula in angle brackets, `<n/d>`, `<w n/d>`
or `<n>`, optionally negative, and SHALL be a `Fraction`. A zero denominator
SHALL be an error. A bare `n/d` in a formula SHALL remain a quotient.

#### Scenario: Fractions
- **WHEN** `<1/10> + <1/10>` and `<2 3/4>` are evaluated
- **THEN** they are the `Fraction`s `1/5` and `2 3/4`
- **AND** `1/10 + 1/10` is the `Decimal` 0.2
