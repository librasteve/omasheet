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

#### Scenario: An as-of join

- **GIVEN** `Commission_Rates` with `Sales_Target` and `Commission_Rate`, its rows in any order, and `Sales` with `Sales_Amount`
- **WHEN** `Sales |> join_asof(Commission_Rates, Sales_Amount, Sales_Target)` is calculated
- **THEN** each sale carries the `Commission_Rate` of the row with the highest `Sales_Target` at or below its `Sales_Amount`, and a sale below every target has empty rate columns

#### Scenario: A complex column

- **GIVEN** a column of `Complex` values
- **WHEN** `cum_sum` is applied to it
- **THEN** it is an error saying `cum_sum` is not defined for Complex

### Requirement: Order for the positional reference

A positional reference SHALL take an `order` clause that sets the order its offset or range is read in, and without it SHALL read in file order. A prefix range of a reducing aggregate SHALL cost one pass over the rows, not one per row.

#### Scenario: The previous transaction by date

- **GIVEN** a sheet `Tx` with `Date`, `Member` and `Amount`, its rows not in date order
- **WHEN** `Bal := ([Bal; *-1 by Member order Date] // 0) + Amount` is calculated
- **THEN** each row's `Bal` is its `Amount` plus the `Bal` of the same member's previous transaction by `Date`, wherever the rows stand

#### Scenario: A row inserted out of order

- **GIVEN** the sheet above, and a row added at the bottom with a date between two existing ones
- **WHEN** the file is calculated again
- **THEN** the new row takes its place in date order and every later balance includes it

#### Scenario: Shift

- **GIVEN** the same sheet
- **WHEN** `Prev := Amount.shift(1, Date by Member)` is calculated
- **THEN** each row holds the `Amount` of its member's previous transaction by `Date`, and the first of a member's has an empty `Prev`

#### Scenario: No clause, no change

- **GIVEN** any file written before this change
- **WHEN** it is calculated
- **THEN** every value is as it was, a reference without `order` reading in file order

#### Scenario: A long running total

- **GIVEN** a sheet of 20,000 rows and `Run := [Amt; 0..*].sum()`
- **WHEN** it is calculated in a release build
- **THEN** it takes under a second, and every `Run` equals what the per-row evaluation gives

#### Scenario: The current row's value

- **GIVEN** `ByDate := Tx[Amount; Date <= Date].sum()` in a sheet `Tx`
- **WHEN** the file is linted
- **THEN** there is a warning that both sides are the whole column `Date`, and that `Tx[Date; *]` is this row's

### Requirement: Nearest-match lookup on a column

`lookup_asof(Other, key, value)` SHALL give, for each row, the `value` of the row of `Other` with the nearest `key` at or below the row's own, whatever the order of `Other`'s rows.

#### Scenario: Unsorted rates

- **GIVEN** `Rates` with `Start` and `Rate`, the later start first, and `Pay` with a `Date`
- **WHEN** `Rate := Date.lookup_asof(Rates, Start, Rate)` is calculated
- **THEN** each pay row carries the rate in effect on its date, and a date before every start is empty
