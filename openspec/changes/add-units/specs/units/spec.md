# units

Units of measure as part of the type system.

## ADDED Requirements

### Requirement: Quantities
A numeric value SHALL be able to carry a unit, forming a quantity. A unit literal
SHALL be written as a number immediately followed by a unit symbol. The magnitude
of a quantity SHALL obey the `numerics` rules, so quantities are exact by default.

#### Scenario: Unit literals
- **WHEN** `10m`, `5kg`, `100ms` and `12.5USD` are evaluated
- **THEN** each is a quantity with an exact `Ratio` or `Int` magnitude and the named unit

### Requirement: Addition and comparison need compatible dimensions
Adding, subtracting or comparing two quantities SHALL require that they have the
same dimension. Quantities of the same dimension in different units SHALL be
converted automatically and exactly. Quantities of different dimensions SHALL be
a type error reported before execution.

#### Scenario: Same unit
- **WHEN** `5m + 3m` is evaluated
- **THEN** the result is `8m`

#### Scenario: Same dimension, different unit
- **WHEN** `5m + 300cm` is evaluated
- **THEN** the result is `8m`

#### Scenario: Different dimensions
- **WHEN** `5m + 3s` is checked
- **THEN** a type error reports that `m` and `s` are incompatible

### Requirement: Multiplication and division combine units
Multiplying or dividing quantities SHALL combine their units algebraically, and
units that cancel SHALL leave a dimensionless number.

#### Scenario: Derived unit
- **WHEN** `10m / 2s` is evaluated
- **THEN** the result is `5 m/s`

#### Scenario: Cancellation
- **WHEN** `6m / 2m` is evaluated
- **THEN** the result is the dimensionless `3`

#### Scenario: Scaling by a percentage
- **WHEN** `100GBP * 20%` is evaluated
- **THEN** the result is `20GBP`

### Requirement: Currencies are units without implicit conversion
Each currency SHALL be a unit with its own dimension. Arithmetic that would
require converting between currencies SHALL be a type error unless the conversion
is written explicitly with `convert(<quantity>, <unit>)` and a rate is available.

#### Scenario: Mixed currencies
- **WHEN** `10GBP + 15USD` is checked
- **THEN** a type error is reported

#### Scenario: Explicit conversion
- **GIVEN** a declared GBP/USD rate
- **WHEN** `10GBP + convert(15USD, GBP)` is evaluated
- **THEN** the result is a `GBP` quantity

### Requirement: Column-level units
A column SHALL be able to declare its unit in the table schema. Cells in that
column SHALL NOT need to repeat the unit: a bare number takes the column's unit.
A cell written with a different unit of the same dimension SHALL be accepted and
converted. A cell written with a unit of a different dimension SHALL be a type
error reported before execution.

#### Scenario: Bare numbers take the column unit
- **GIVEN** `Distance : m` and a cell `100`
- **THEN** the cell's value is `100m`

#### Scenario: Mixed units in one column
- **GIVEN** `Distance : m` and cells `10m`, `300cm`, `2km`
- **THEN** the stored values are `10m`, `3m`, `2000m`

#### Scenario: Wrong dimension in a cell
- **GIVEN** `Distance : m` and a cell `5kg`
- **WHEN** the sheet is checked
- **THEN** an error reports `expected m, found kg` with the cell's location

### Requirement: Unit inference for computed columns
The unit of a computed column SHALL be inferred from its expression. If the
column also declares a unit, the inferred unit SHALL be checked against the
declaration.

#### Scenario: Same-unit subtraction
- **GIVEN** `Revenue : GBP`, `Cost : GBP`, `Profit := Revenue - Cost`
- **THEN** `Profit` has unit `GBP`

#### Scenario: Quotient
- **GIVEN** `Distance : m`, `Time : s`, `Speed := Distance / Time`
- **THEN** `Speed` has unit `m/s`

#### Scenario: Incompatible columns
- **GIVEN** `Revenue : GBP`, `Weight : kg`
- **WHEN** a column is defined as `Revenue + Weight`
- **THEN** a type error is reported and nothing is executed

#### Scenario: Declared unit disagrees with inferred
- **GIVEN** `Speed : kg` and `Speed := Distance / Time` with `Distance : m`, `Time : s`
- **THEN** a type error reports `declared kg, inferred m/s`

### Requirement: User-defined units
A sheet SHALL be able to declare a new base unit with `unit <Name>` and a derived
unit with `unit <Name> = <quantity expression>`.

#### Scenario: Derived unit
- **GIVEN** `unit Day = 24h`
- **WHEN** `2Day + 12h` is evaluated
- **THEN** the result equals `60h`

#### Scenario: New base unit
- **GIVEN** `unit Widget`
- **WHEN** `3Widget + 2m` is checked
- **THEN** a type error is reported

### Requirement: Display unit is separate from stored value
The unit in which a quantity is displayed SHALL be a formatting choice that does
not change the stored quantity.

#### Scenario: Display in a different unit
- **GIVEN** a stored value `1500m`
- **WHEN** it is displayed in `km`
- **THEN** it shows as `1.5 km` and still compares equal to `1500m`
