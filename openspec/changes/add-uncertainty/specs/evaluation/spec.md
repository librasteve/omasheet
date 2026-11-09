# evaluation

Uncertainty in sheet operations.

## MODIFIED Requirements

### Requirement: Sheet operations preserve exactness
Selection, sorting, grouping, joining and aggregation SHALL preserve the exact numeric values and uncertainties of the data they operate on.

#### Scenario: Exact group sum
- **GIVEN** a `Fraction` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10`

#### Scenario: Group sum propagates uncertainty
- **GIVEN** a `Scientific` column with values `10 ± 0.3`, `20 ± 0.4` in one group
- **WHEN** the group is summed
- **THEN** the result is `30 ± 0.5`

#### Scenario: Selection keeps uncertainty
- **GIVEN** a `Scientific` column with values `10 ± 0.3`, `20 ± 0.4`
- **WHEN** the rows with a value above 15 are selected
- **THEN** the row holds `20 ± 0.4`
