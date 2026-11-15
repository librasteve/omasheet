# units

SI quantities: a number with a unit.

## ADDED Requirements

### Requirement: Quantity literals
A `Scientific` literal immediately followed by an SI unit SHALL make a quantity. The unit SHALL be written as one or more terms joined by `.`, each a unit symbol with an optional SI prefix and an optional `^` and signed integer exponent. A unit SHALL NOT contain a space, `/`, a bracket, a superscript or a name. The uncertainty of the literal, if any, SHALL be the uncertainty of the quantity.

#### Scenario: A unit after a number
- **WHEN** `5kg` is evaluated
- **THEN** the result is a quantity of `5` with the unit `kg`

#### Scenario: A compound unit
- **WHEN** `9.81m.s^-2` is evaluated
- **THEN** the result is `9.81` with the unit `m.s^-2`

#### Scenario: With an uncertainty
- **WHEN** `10.5(3)e-3kg` is evaluated
- **THEN** the result has value `0.0105`, uncertainty `0.0003` and the unit `kg`

#### Scenario: Not a unit
- **WHEN** `5 kg`, `5km/s`, `5m2` or `5ft` is checked
- **THEN** an error is reported

#### Scenario: A method call is not a unit
- **WHEN** `5kg.abs()` is evaluated
- **THEN** the result is `5kg`, the `abs` of the quantity

### Requirement: Scales
A prefix and a trailing `.` SHALL be read as the number it stands for, with no unit: `1k.` is 1000. A scale on an exact number SHALL be exact.

#### Scenario: Kilo
- **WHEN** `1k. == 1000` is evaluated
- **THEN** the result is true

#### Scenario: Exact
- **WHEN** `1.5k.` is evaluated
- **THEN** the result is the exact decimal `1500`

#### Scenario: Milli is not the metre
- **WHEN** `5m.` is evaluated
- **THEN** the result is `0.005`, and `5m` is five metres

### Requirement: Arithmetic
Addition, subtraction and comparison of two quantities SHALL need the same dimension, and SHALL convert the right side to the unit of the left. Multiplication and division SHALL combine the units. A power by a whole number SHALL apply to the unit. The uncertainty SHALL propagate as for `Scientific`. A result with no dimension SHALL be a plain number.

#### Scenario: Conversion
- **WHEN** `5kg + 300g` is evaluated
- **THEN** the result is `5.3kg`

#### Scenario: Dimension mismatch
- **WHEN** `5kg + 3m` is evaluated
- **THEN** an error is reported, naming the two dimensions

#### Scenario: Product
- **WHEN** `2m * 3m` is evaluated
- **THEN** the result is `6m^2`

#### Scenario: No dimension
- **WHEN** `6m / 3m` is evaluated
- **THEN** the result is the number `2`

#### Scenario: Temperature
- **WHEN** `20°C - 10°C` is evaluated
- **THEN** the result is `10°C`

### Requirement: Conversion
`to(quantity, 'unit')` SHALL give the quantity in another unit of its dimension.

#### Scenario: To grams
- **WHEN** `5kg.to('g')` is evaluated
- **THEN** the result is `5000g`

#### Scenario: Wrong dimension
- **WHEN** `5kg.to('m')` is evaluated
- **THEN** an error is reported

### Requirement: Quantity types
A column type SHALL be `Quantity`, or the name of a dimension (`Length`, `Mass`, `Force`, ...). A quantity SHALL be admitted by a named type if its unit has that dimension. A bare number in such a column SHALL be read in the column's display unit.

#### Scenario: Declared type
- **GIVEN** a column `Weight : Mass`
- **WHEN** a cell `300g` is read
- **THEN** it is a quantity, shown in the column unit `kg` as `0.3kg`

#### Scenario: Wrong dimension
- **GIVEN** a column `Weight : Mass`
- **WHEN** a cell `3m` is read
- **THEN** an error is reported

#### Scenario: Bare number
- **GIVEN** a column `Weight : Mass`
- **WHEN** a cell `5` is read
- **THEN** it is `5kg`

### Requirement: Aggregation
`sum`, `mean`, `min`, `max` and `median` of a column of quantities SHALL give a quantity in the unit of the first. `std` and `var` SHALL be refused.

#### Scenario: Sum
- **GIVEN** a `Mass` column of `5kg`, `300g`
- **WHEN** `sum` is evaluated
- **THEN** the result is `5.3kg`
