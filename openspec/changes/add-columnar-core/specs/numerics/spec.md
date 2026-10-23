# numerics

The units a time and a date-time are held in.

## MODIFIED Requirements

### Requirement: A time is held in nanoseconds and a date-time in microseconds

A `Time` SHALL be held as nanoseconds since midnight and a `Datetime` as
microseconds, which are the units Polars holds them in, so that a value
carried in a Parquet file arrives with every digit it was written with.

What a whole number means next to them SHALL NOT change: a number added to a
`Time` or a `Datetime` is that many **seconds**, as it is today, and the
difference of two of them is in seconds.

#### Scenario: A sub-second time survives the boundary

- **GIVEN** a Parquet column holding `23:59:59.999999999`
- **WHEN** it is read and written back
- **THEN** every digit is the one it was written with

#### Scenario: A whole number is still seconds

- **WHEN** `2025-01-31T09:30 + 1` and `09:30 + 90` are evaluated
- **THEN** the results are `2025-01-31T09:30:01` and `09:31:30`

#### Scenario: Two date-times subtract to seconds

- **WHEN** `2025-01-31T09:30 - 2025-01-31T09:00` is evaluated
- **THEN** the result is `1800`
