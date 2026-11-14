# evaluation

## MODIFIED Requirements

### Requirement: Which columns are calculated in one step

Every operation over `Integer`, `Decimal(s)`, `Percent`, `Float`, `String`, `Boolean` and the temporal types SHALL be lowered to Polars where the column is a table's or the formula is a column of cells, including `/` and the mean of a `Decimal`. A `Fraction`, and a power of a `Decimal` other than a whole literal power that fits (`x ** 2`), SHALL be calculated by the engine. The two paths SHALL give the same value for every operation, scale pair and edge value.

#### Scenario: A mixed-scale sum is lowered

- **GIVEN** a `Decimal(2)` column and a `Decimal(4)` column of a table of a million rows
- **WHEN** their sum is calculated
- **THEN** the column is lowered
- **AND** each value equals the engine's

#### Scenario: A quotient is lowered and rounded

- **GIVEN** two `Decimal(2)` columns
- **WHEN** their quotient is calculated
- **THEN** the column is lowered, and each value is rounded to two places, half away from zero, as the engine rounds it

#### Scenario: The guard hands a row to the exact path

- **GIVEN** a `Decimal(10)` column holding a value whose product with itself passes 38 digits at twice the scale
- **WHEN** the product is calculated
- **THEN** the column is calculated by the engine
- **AND** the cell is the error of the numerics spec, as it is on the lowered path

#### Scenario: A Fraction is never lowered

- **GIVEN** a `Fraction` column
- **WHEN** a formula reads it
- **THEN** the formula is calculated by the engine
