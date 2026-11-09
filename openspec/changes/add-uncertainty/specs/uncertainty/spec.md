# uncertainty

Measurement uncertainty as a first-class part of a value.

## ADDED Requirements

### Requirement: Uncertain values
A real number SHALL be able to carry a measurement uncertainty: a non-negative standard deviation, held as a floating-point number beside its value. An uncertain value SHALL be more infectious than a `Float`: an operation with an uncertain operand gives an uncertain result, and an operand with no uncertainty counts as having uncertainty zero.

#### Scenario: Exact operand
- **WHEN** `(10 ± 0.1) * 2` is evaluated
- **THEN** the result is `20` with uncertainty `0.2`

### Requirement: The ± literal
`<value> ± <uncertainty>` SHALL make an uncertain value, and `+-` SHALL be accepted as an ASCII spelling of `±`. Both sides SHALL be real numbers with no uncertainty of their own, and the uncertainty SHALL be non-negative. `±` SHALL bind more loosely than `+` and `-` and more tightly than a comparison, so a result is combined further by putting it in brackets.

#### Scenario: Literal
- **WHEN** `10 ± 0.1` is evaluated
- **THEN** the result has value `10` and uncertainty `0.1`

#### Scenario: ASCII spelling
- **WHEN** `10 +- 0.1` is evaluated
- **THEN** the result is identical to `10 ± 0.1`

#### Scenario: Uncertainty is an expression
- **WHEN** `10 ± 0.1 * 2` is evaluated
- **THEN** the result has value `10` and uncertainty `0.2`

#### Scenario: Negative uncertainty
- **WHEN** `10 ± -1` is checked
- **THEN** an error is reported

#### Scenario: Uncertain operand of ±
- **WHEN** `(10 ± 1) ± 1` is checked
- **THEN** an error is reported

### Requirement: The concise literal
A number literal followed at once, with no space, by digits in brackets SHALL make an uncertain value whose uncertainty is those digits taken at the place of the number's last digit. An exponent after the brackets SHALL scale both the value and the uncertainty. Only digits SHALL be allowed in the brackets.

#### Scenario: One decimal place
- **WHEN** `10.5(3)` is evaluated
- **THEN** the result is `10.5 ± 0.3`

#### Scenario: Whole number
- **WHEN** `10(3)` is evaluated
- **THEN** the result is `10 ± 3`

#### Scenario: Digits longer than the decimals
- **WHEN** `1.5(11)` is evaluated
- **THEN** the result is `1.5 ± 1.1`

#### Scenario: A physical constant
- **WHEN** `7.297_352_5643(11)e-3` is evaluated
- **THEN** the result has value `7.2973525643e-3` and uncertainty `1.1e-12`

#### Scenario: Exact in brackets
- **WHEN** `10.5(0)` is evaluated
- **THEN** the result has value `10.5` and uncertainty zero

#### Scenario: Not digits
- **WHEN** `10.5(3.2)` or `10.5(x)` is checked
- **THEN** an error is reported

### Requirement: Arithmetic propagation
Arithmetic on uncertain values SHALL propagate uncertainty automatically, treating the operands as independent. For addition and subtraction the absolute uncertainties SHALL combine in quadrature. For multiplication and division the relative uncertainties SHALL combine in quadrature.

#### Scenario: Addition
- **WHEN** `(10 ± 1) + (20 ± 2)` is evaluated
- **THEN** the result is `30` with uncertainty `√5` (≈ 2.236), not `3`

#### Scenario: Multiplication
- **WHEN** `(100 ± 2) * (0.2 ± 0.01)` is evaluated
- **THEN** the value is `20` and the relative uncertainty is `√(0.02² + 0.05²)`, giving an uncertainty of ≈ `1.077`

#### Scenario: Division
- **WHEN** `(100 ± 2) / (50 ± 1)` is evaluated
- **THEN** the value is `2` and the relative uncertainty is `√(0.02² + 0.02²)`, giving an uncertainty of ≈ `0.0566`

#### Scenario: Operands are independent
- **GIVEN** a cell `A1` of `10 ± 1`
- **WHEN** `A1 - A1` is evaluated
- **THEN** the result is `0` with uncertainty `√2`, not `0`

### Requirement: Functions propagate to first order
A function of an uncertain value SHALL return an uncertain value with `f(x)` as its value and `|f′(x)|·σ` as its uncertainty, for every math function the language has: the powers and roots, the exponential and logarithmic functions, and the trigonometric, inverse and hyperbolic functions. A power with an uncertain base or exponent, and any function of two uncertain values, SHALL combine the contributions of the partial derivatives in quadrature. The result is a linear approximation and is poor where σ is large beside the distance to the edge of the function's domain.

#### Scenario: Sine
- **WHEN** `sin(0.5 ± 0.01)` is evaluated
- **THEN** the value is `sin(0.5)` ≈ `0.4794` and the uncertainty is `cos(0.5) × 0.01` ≈ `0.00878`

#### Scenario: Logarithm
- **WHEN** `ln(100 ± 1)` is evaluated
- **THEN** the value is ≈ `4.60517` and the uncertainty is `1/100 × 1` = `0.01`

#### Scenario: Root
- **WHEN** `sqrt(4 ± 0.4)` is evaluated
- **THEN** the value is `2` and the uncertainty is `0.4 / (2 × 2)` = `0.1`

#### Scenario: Power
- **WHEN** `(3 ± 0.1) ** 2` is evaluated
- **THEN** the value is `9` and the uncertainty is `2 × 3 × 0.1` = `0.6`

#### Scenario: Exact argument
- **WHEN** `sin(0.5)` is evaluated
- **THEN** the result has no uncertainty, as before

### Requirement: Aggregation
`sum`, `mean`, `product` and the `median` of an even count over a vector of uncertain values SHALL return an uncertain value propagated by the arithmetic rules. `min`, `max`, `first`, `last` and `quantile` SHALL return the chosen element, with its uncertainty. `std` and `var` SHALL work on the values and return a `Float` with no uncertainty. Comparison and sorting SHALL use the values alone.

#### Scenario: Mean of measurements
- **GIVEN** a column with cells `10 ± 0.1`, `11 ± 0.2`, `12 ± 0.1`
- **WHEN** `.mean()` is evaluated
- **THEN** the result is `11` with uncertainty `√0.06 / 3` (≈ 0.0816)

#### Scenario: Maximum keeps its uncertainty
- **GIVEN** a column with cells `10 ± 0.1`, `12 ± 0.3`
- **WHEN** `.max()` is evaluated
- **THEN** the result is `12 ± 0.3`

#### Scenario: A spread has none
- **GIVEN** a column with cells `10 ± 0.1`, `12 ± 0.3`
- **WHEN** `.std()` is evaluated
- **THEN** the result is a `Float`, with no uncertainty

### Requirement: Uncertain complex is not supported
An operation that gives a `Complex` from an uncertain operand SHALL not carry the uncertainty: the uncertain value SHALL degrade to a `Float`, the operation SHALL proceed with the `Float`, and the message `uncertain complex not supported` SHALL be shown as a warning in the bottom bar while the cell is selected, in an amber warning colour distinct from the red of an error. The cell SHALL show its value, not an error. This applies when an operand of an arithmetic operator is a `Complex`, and where `Complex(re, im)` or `x.Complex` is given an uncertain value. A real function of a value outside its domain is `NaN`, as it is for a `Float`, and is not a `Complex`.

#### Scenario: Complex operand
- **WHEN** `(10 ± 1) + 3i` is evaluated
- **THEN** the result is `10+3i` with no uncertainty, and the bottom bar shows `uncertain complex not supported` in amber

#### Scenario: Complex made of an uncertain value
- **WHEN** `Complex(1 ± 0.1, 2)` or `(1 ± 0.1).Complex` is evaluated
- **THEN** the result is `1+2i` or `1+0i`, with no uncertainty, and the same warning is shown

#### Scenario: A real result
- **WHEN** `(10 ± 1) * 2` is evaluated
- **THEN** the result is `20 ± 2` and no message is shown

#### Scenario: Outside the domain
- **WHEN** `sqrt(-4 ± 0.4)` is evaluated
- **THEN** the result is `NaN ± NaN`, as `sqrt(-4.0)` is `NaN`, and no message is shown

### Requirement: Uncertainty is not an interval
A value with an uncertainty SHALL denote a best estimate with an error model, and SHALL NOT be interchangeable with a range (`a..b`).

#### Scenario: Range is not an uncertain value
- **WHEN** `(9.9..10.1) + 1` is checked where an uncertain scalar is expected
- **THEN** it is not treated as `10 ± 0.1`

### Requirement: Display of uncertain values
An uncertain value SHALL display by default in the concise form: the uncertainty rounded to two significant digits, written as those digits in brackets after the value rounded to the same decimal place, with any exponent written once after the brackets. An uncertain value whose uncertainty is not zero SHALL always display this way, however large the uncertainty is beside the value. A value whose uncertainty is zero SHALL display as the plain number. A column format of `±` SHALL display the value as `<value> ± <uncertainty>`, rounded in the same way. Display SHALL NOT alter the stored value or uncertainty.

#### Scenario: Default display
- **GIVEN** a stored value of `11` with uncertainty `0.0816…`
- **WHEN** it is displayed with default formatting
- **THEN** it shows as `11.000(82)`

#### Scenario: A physical constant
- **GIVEN** a stored value of `7.2973525643e-3` with uncertainty `1.1e-12`
- **WHEN** it is displayed with default formatting
- **THEN** it shows as `7.2973525643(11)e-3`

#### Scenario: Uncertainty larger than the value
- **GIVEN** a stored value of `0.5` with uncertainty `0.8`
- **WHEN** it is displayed with default formatting
- **THEN** it shows as `0.50(80)`

#### Scenario: Uncertainty coarser than the units
- **GIVEN** a stored value of `1000` with uncertainty `200`
- **WHEN** it is displayed with default formatting
- **THEN** it shows as `1.00(20)e3`

#### Scenario: No uncertainty
- **GIVEN** a stored value of `10` with uncertainty zero
- **WHEN** it is displayed
- **THEN** it shows as `10`

#### Scenario: Plus-minus format
- **GIVEN** a column declared `Uncertain[±]` and a stored value of `11` with uncertainty `0.0816…`
- **WHEN** it is displayed
- **THEN** it shows as `11.000 ± 0.082`
