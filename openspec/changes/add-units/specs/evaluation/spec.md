# evaluation

Units in static checking and in table operations.

## MODIFIED Requirements

### Requirement: Static checking before execution
Types, units and shapes SHALL be checked for the whole sheet before any
calculation runs. If checking reports any error, the system SHALL NOT execute any
part of the sheet.

#### Scenario: One type error blocks execution
- **GIVEN** a sheet with ten valid computed columns and one defined as `Revenue + Month` where `Revenue` is `Ratio` and `Month` is `Text`
- **WHEN** the sheet is evaluated
- **THEN** the type error is reported and no column is calculated

#### Scenario: One unit error blocks execution
- **GIVEN** a sheet with ten valid computed columns and one defined as `Revenue + Weight` where units are `GBP` and `kg`
- **WHEN** the sheet is evaluated
- **THEN** the unit error is reported and no column is calculated

#### Scenario: All errors reported together
- **WHEN** a sheet contains three independent type errors
- **THEN** all three are reported in one run

### Requirement: Table operations preserve exactness
Selection, sorting, grouping, joining and aggregation SHALL preserve the exact
numeric values and units of the data they operate on.

#### Scenario: Exact group sum
- **GIVEN** a `Ratio` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10`

#### Scenario: Group sum keeps its unit
- **GIVEN** a `Ratio<GBP>` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10 GBP`
