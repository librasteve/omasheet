# numerics

The units a time and a date-time are held in.

## MODIFIED Requirements

### Requirement: A time is held in nanoseconds and a date-time in microseconds

A `Time` SHALL be held as nanoseconds since midnight and a `Datetime` as
microseconds, which are the units Polars holds them in, so that a value
carried in a Parquet file arrives with every digit it was written with.

What a whole number means next to them SHALL NOT change: a number added to a
`Time` or a `Datetime` is that many **seconds**, as it is today.

The difference of two of them is left exactly as it is, in seconds, which this
change neither relies on nor alters. It is not the end state: `add-polars-dtypes`
D22 makes the difference of two dates, times or date-times a `Duration`, written
and shown as an ISO 8601 duration, and its task 5.2 is what replaces the
requirement below.

#### Scenario: A sub-second time survives the boundary

- **GIVEN** a Parquet column holding `23:59:59.999999999`
- **WHEN** it is read and written back
- **THEN** every digit is the one it was written with

#### Scenario: A whole number is still seconds

- **WHEN** `2025-01-31T09:30 + 1` and `09:30 + 90` are evaluated
- **THEN** the results are `2025-01-31T09:30:01` and `09:31:30`

#### Scenario: Two date-times subtract to seconds, until `Duration` exists

- **WHEN** `2025-01-31T09:30 - 2025-01-31T09:00` is evaluated
- **THEN** the result is `1800`
- **AND** this holds only until `add-polars-dtypes` task 5.2 lands, after which
  the result is a `Duration` of 30 minutes and not the number `1800`
