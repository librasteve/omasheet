# numerics

## MODIFIED Requirements

### Requirement: Powers

A negative real base to a non-whole finite real exponent SHALL give a `Complex`, `exp(y ln x)`.

#### Scenario: A fractional power of a negative number

- **GIVEN** `(-2) ** 0.5`
- **WHEN** it is evaluated
- **THEN** the result is a `Complex` with real part about `8.7e-17` and imaginary part `1.4142135623730951`
