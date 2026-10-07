# uncertainty

Measurement uncertainty as a first-class part of a value.

## ADDED Requirements

### Requirement: Uncertain values
A number or quantity SHALL be able to carry a measurement uncertainty, written
`<value> ± <uncertainty>` with an optional trailing unit applying to both. `+-`
SHALL be accepted as an ASCII spelling of `±`. The uncertainty SHALL be
non-negative and, if it has a unit, of the same dimension as the value.

#### Scenario: Literal with shared unit
- **WHEN** `10 ± 0.1 m` is evaluated
- **THEN** the result has value `10m` and uncertainty `0.1m`

#### Scenario: ASCII spelling
- **WHEN** `10 +- 0.1 m` is evaluated
- **THEN** the result is identical to `10 ± 0.1 m`

#### Scenario: Different units of the same dimension
- **WHEN** `10m ± 10cm` is evaluated
- **THEN** the result has value `10m` and uncertainty `0.1m`

#### Scenario: Mismatched dimensions
- **WHEN** `10m ± 1s` is checked
- **THEN** a type error is reported

### Requirement: Automatic propagation
Arithmetic on uncertain values SHALL propagate uncertainty automatically, treating
operands as independent. For addition and subtraction the absolute uncertainties
SHALL combine in quadrature. For multiplication and division the relative
uncertainties SHALL combine in quadrature. An operand without uncertainty SHALL
be treated as having uncertainty zero.

#### Scenario: Addition
- **WHEN** `(10 ± 1 m) + (20 ± 2 m)` is evaluated
- **THEN** the result is `30m` with uncertainty `√5 m` (≈ 2.24 m), not `3m`

#### Scenario: Multiplication
- **WHEN** `(100 ± 2 GBP) * (20% ± 1%)` is evaluated
- **THEN** the value is `20GBP` and the relative uncertainty is `√(0.02² + 0.05²)`, giving ≈ `1.08GBP`

#### Scenario: Exact operand
- **WHEN** `(10 ± 0.1 m) * 2` is evaluated
- **THEN** the result is `20 ± 0.2 m`

### Requirement: Aggregation propagates uncertainty
Aggregation methods over a vector of uncertain values SHALL return an uncertain
value propagated by the same rules.

#### Scenario: Mean of measurements
- **GIVEN** a column with cells `10 ± 0.1`, `11 ± 0.2`, `12 ± 0.1` and unit `m`
- **WHEN** `.avg()` is evaluated
- **THEN** the result is `11m` with uncertainty `√0.06 / 3 m` (≈ 0.08 m)

### Requirement: Uncertain column type
A column SHALL be able to be declared `Uncertain<Unit>`, so that cells need
neither repeat the unit nor, when exact, the uncertainty.

#### Scenario: Unit omitted in cells
- **GIVEN** `Length : Uncertain<m>` and a cell `10 ± 0.1`
- **THEN** the cell's value is `10 ± 0.1 m`

#### Scenario: Cell without uncertainty
- **GIVEN** `Length : Uncertain<m>` and a cell `10`
- **THEN** the cell's value is `10m` with uncertainty zero

### Requirement: Uncertainty is not an interval
A value with `±` SHALL denote a best estimate with an error model, and SHALL NOT
be interchangeable with a range (`a..b`).

#### Scenario: Range is not an uncertain value
- **WHEN** `(9.9..10.1) + 1` is checked where an uncertain scalar is expected
- **THEN** it is not treated as `10 ± 0.1`

### Requirement: Display of uncertain values
An uncertain value SHALL display by default as `<value> ± <uncertainty> <unit>`,
with the value rounded to the precision of the uncertainty. Display SHALL NOT
alter the stored value or uncertainty.

#### Scenario: Default display
- **GIVEN** a stored value of `11m` with uncertainty `0.0816…m`
- **WHEN** it is displayed with default formatting
- **THEN** it shows as `11.00 ± 0.08 m`
