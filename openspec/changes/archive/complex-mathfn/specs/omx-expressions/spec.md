# omx-expressions

## MODIFIED Requirements

### Requirement: Complex functions

`abs sqrt exp ln log10 log2 sin cos tan arcsin arccos arctan sinh cosh tanh arcsinh arccosh arctanh sign` SHALL accept a `Complex` and give the principal value. A real argument outside the function's real domain SHALL give the principal `Complex` and not `NaN`. `round floor ceil radians degrees` SHALL refuse a `Complex`, naming the function.

#### Scenario: A root of a negative number

- **GIVEN** `sqrt(-4)`
- **WHEN** it is evaluated
- **THEN** the result is `0+2i`

#### Scenario: Beyond the domain of an inverse

- **GIVEN** `arccos(2)` and `arctanh(2)`
- **WHEN** each is evaluated
- **THEN** the results are `0-1.3169578969248166i` and `0.5493061443340549+1.5707963267948966i`

#### Scenario: A boundary stays real

- **GIVEN** `ln(0)`, `arctanh(1)` and `sqrt(0)`
- **WHEN** each is evaluated
- **THEN** the results are `-Inf`, `Inf` and `0`

#### Scenario: A declared real column

- **GIVEN** a column declared `Float` holding `= sqrt(-4)`
- **WHEN** it is evaluated
- **THEN** the cell is an error saying `0+2i` cannot be converted to `Float`

#### Scenario: An undeclared column

- **GIVEN** an undeclared column with the cells `= sqrt(4)` and `= sqrt(-4)`
- **WHEN** its type is worked out
- **THEN** it is `Complex`

## ADDED Requirements

### Requirement: Inverse hyperbolic functions

`arcsinh`, `arccosh` and `arctanh` SHALL give the inverse hyperbolic sine, cosine and tangent of a real number or a `Complex`.
