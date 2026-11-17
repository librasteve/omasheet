# omx-expressions

## MODIFIED Requirements

### Requirement: Arithmetic with durations
A date, a time or a date-time minus its own kind SHALL be a `Duration`. A `Duration` plus or minus a `Duration` SHALL be a `Duration`, and times or over a number a `Duration`. A `Duration` times or over a quantity of another dimension SHALL be a quantity of what the product measures, with the duration as so many seconds. A date, a time or a date-time plus or minus a `Duration` SHALL be as it was, and beside a date only its whole days count. A quantity of time other than a `Duration` beside a date SHALL be read as one; any other unit there SHALL be an error.

#### Scenario: A difference
- **WHEN** `2025-03-01 - 2024-03-01` is calculated
- **THEN** it is a `Duration` of 365 days

#### Scenario: Scaled
- **WHEN** `+01:00 * 3` is calculated
- **THEN** it is `+03:00`

#### Scenario: With another unit
- **WHEN** `8610km / +11:10` is calculated
- **THEN** it is a speed in `km.s^-1`

#### Scenario: Moving a time
- **WHEN** `17:05:00 + 10s` is calculated
- **THEN** it is `17:05:10`

### Requirement: A signed duration after an operator
Where an operand has gone before a `+` or `-`, it SHALL be the operator, so a signed colon duration after it takes a second sign.

#### Scenario: Two signs
- **WHEN** `09:00 + +01:30` is calculated
- **THEN** it is `10:30`

#### Scenario: One sign
- **WHEN** `09:00 +01:30` is written
- **THEN** it is the sum of two times of day, and an error

## REMOVED Requirements

### Requirement: The conversions of a duration
**Reason**: a `Duration` is a quantity of time, so `.Quantity` and `.Duration` have nothing to convert.
**Migration**: delete them; `PT90M.Quantity` is `PT90M`, and `5400s.Duration` is `5400s` in a `Duration` column.
