# omx-expressions

## ADDED Requirements

### Requirement: Polars-backed functions

A function with a Polars version SHALL be named as that Polars method is, take its arguments positionally in Polars' order, and give the result Polars gives, on a table and on a plain sheet alike. A type Polars cannot hold SHALL be refused with an error naming the function.

#### Scenario: A running total in date order

- **GIVEN** a sheet `Ledger` with the columns `Date` and `Amount`, its rows not in date order
- **WHEN** `Running := Amount.cum_sum(Date)` is calculated
- **THEN** each row holds the sum of the `Amount` of every row up to it in `Date` order, the rows staying where they are, and ties in `Date` kept in row order

#### Scenario: A running total per account

- **GIVEN** the same with an `Account` column
- **WHEN** `Running := Amount.cum_sum(Date by Account)` is calculated
- **THEN** each account has its own running total

#### Scenario: A complex column

- **GIVEN** a column of `Complex` values
- **WHEN** `cum_sum` is applied to it
- **THEN** it is an error saying `cum_sum` is not defined for Complex
