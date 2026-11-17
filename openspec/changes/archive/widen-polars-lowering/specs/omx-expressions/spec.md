# omx-expressions

## ADDED Requirements

### Requirement: Lowering needs static types

A calculated column SHALL be calculated in Polars only if the type of every operand is known before it is run, from a declared column type or the type of a literal, a constant or a function result. A column with no declared type, a `Fraction`, and an operand typed `Any` SHALL be calculated by the engine. The answer SHALL be the same either way.

#### Scenario: An undeclared column

- **GIVEN** a column with no declared type, calculated from values whose dress is inferred
- **WHEN** it is calculated
- **THEN** the engine calculates it, and the values and their dress are as they were

### Requirement: Percentages in lowered arithmetic

A percentage literal or constant in an expression for a declared column SHALL give the same answer whether the column is calculated in Polars or by the engine: two percentages give a percentage, and any other mix gives the plain number.

#### Scenario: A rate applied to a column

- **GIVEN** `const TaxRate = 20%` and `Tax := Profit * TaxRate` in a column declared `Decimal`
- **WHEN** it is calculated in Polars
- **THEN** each value is `Profit * 0.2`, the same as the engine's

### Requirement: Quantities and uncertain values in lowered arithmetic

`Quantity op Quantity` and `Scientific op` a number or a `Scientific`, with both types known, SHALL give the same value, unit and uncertainty whether calculated in Polars or by the engine. An offset unit and an operand typed `Any` SHALL be calculated by the engine.

#### Scenario: Mixed units of one dimension

- **GIVEN** a `Mass` column in `kg` and `Total := Weight + 300g`
- **WHEN** it is calculated
- **THEN** each row is the weight plus `0.3kg`, in `kg`, the same as the engine's
