# evaluation

Uncertainty in table operations.

## MODIFIED Requirements

### Requirement: Table operations preserve exactness
Selection, sorting, grouping, joining and aggregation SHALL preserve the exact
numeric values, units and uncertainties of the data they operate on.

#### Scenario: Exact group sum
- **GIVEN** a `Ratio` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10`

#### Scenario: Group sum keeps its unit
- **GIVEN** a `Ratio<GBP>` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10 GBP`

#### Scenario: Group sum propagates uncertainty
- **GIVEN** an `Uncertain<m>` column with values `10 ± 0.3`, `20 ± 0.4` in one group
- **WHEN** the group is summed
- **THEN** the result is `30 ± 0.5 m`
