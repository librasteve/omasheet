# omx-expressions

The function names, which are now the Polars names.

## MODIFIED Requirements

### Requirement: Aggregation methods

Vectors SHALL provide the methods `.sum()`, `.mean()`, `.min()`, `.max()`,
`.count()`, `.len()`, `.median()`, `.std()`, `.var()`, `.quantile()`,
`.product()`, `.first()` and `.last()`, each returning a scalar and each named
as Polars names it. `.count()` SHALL count the values that are not empty and
`.len()` SHALL count every element, empty or not. Over a sheet, `.len()` SHALL
give the row count and `.count()` SHALL NOT be accepted. `.std()` and `.var()`
SHALL take an optional second argument, the delta degrees of freedom,
defaulting to 1 as Polars does.

#### Scenario: The mean of a column

- **GIVEN** `Sales.Revenue` is `[100, 120, 150]`
- **WHEN** `Sales.Revenue.sum()` and `Sales.Revenue.mean()` are evaluated
- **THEN** the results are `370` and exactly `370/3`

#### Scenario: Counting values and counting rows

- **GIVEN** a sheet `Sales` of three rows whose `Note` column is empty in one
- **WHEN** `Sales.Note.count()`, `Sales.Note.len()` and `Sales.len()` are
  evaluated
- **THEN** the results are `2`, `3` and `3`

#### Scenario: Counting the rows of a sheet

- **WHEN** `Sales.count()` is evaluated
- **THEN** it is refused, and the diagnostic names `len`

#### Scenario: Degrees of freedom

- **WHEN** `Readings.Value.std()` and `Readings.Value.std(0)` are evaluated
- **THEN** the first is the sample standard deviation and the second the
  population one

### Requirement: Mathematical functions

The inverse trigonometric functions SHALL be named `arcsin`, `arccos` and
`arctan`, as Polars names them.

`round(x, n)` SHALL round to `n` decimal places, as Polars does, and
`round(x)` to a whole number, a half away from zero. The nearest-multiple
form, where the second argument named a multiple rather than a count of
places, SHALL NOT be accepted: a second argument that is not a non-negative
integer SHALL be a diagnostic naming the meaning the argument now has.

#### Scenario: Rounding to a number of places

- **WHEN** `round(1.2345, 2)` and `round(5/2)` are evaluated
- **THEN** the results are `1.23` and `3`

#### Scenario: The withdrawn form is named

- **WHEN** `round(1.2345, 0.01)` is evaluated
- **THEN** it is refused before execution
- **AND** the diagnostic says the second argument is a number of decimal
  places

#### Scenario: The inverse trigonometric names

- **WHEN** `arcsin(1)`, `arccos(0)` and `arctan(1)` are evaluated
- **THEN** each gives the angle in radians
- **AND** `asin`, `acos` and `atan` are unknown functions

### Requirement: Time zones

The zone functions SHALL be named as Polars names them:
`convert_time_zone(<zone>)` moves a date-time to another zone,
`base_utc_offset()` gives its offset from UTC, and `time_zone()` gives the
name of its zone. `utc()` and `local()` keep their names, which Polars has no
counterpart for.

#### Scenario: Moving a date-time to another zone

- **WHEN** `Orders.Placed.convert_time_zone("Asia/Tokyo")` is evaluated
- **THEN** each date-time is shown as the clocks of that zone show it

#### Scenario: The old name is gone

- **WHEN** `Orders.Placed.to_zone("Asia/Tokyo")` is evaluated
- **THEN** it is refused as an unknown function

### Requirement: Dates and times

The end-of-month function SHALL be named `month_end`, as Polars names it, and
`month_start` SHALL give the first day of the month. The parts a date-time has
SHALL also include `quarter`, `week` and `ordinal_day`.

#### Scenario: The ends of a month

- **WHEN** `month_end(2025-02-10)` and `month_start(2025-02-10)` are evaluated
- **THEN** the results are `2025-02-28` and `2025-02-01`

## ADDED Requirements

### Requirement: Selecting the first or last rows

`head(n)` SHALL select the first `n` rows of a sheet and `tail(n)` the last
`n`, each as the selection it is equivalent to rather than as a new operation.
`shift(n)` SHALL give the value `n` rows earlier, the cursor offset written as
a function, and SHALL be empty where that row is outside the sheet. It reads
the current row, so it SHALL be written in a sheet's own formula, as `*` is.

#### Scenario: The first rows

- **GIVEN** a sheet `Sales` of twelve rows
- **WHEN** `Sales |> head(5)` is evaluated
- **THEN** the result is the first five rows
- **AND** it equals `Sales[; ^5]`

#### Scenario: The last rows

- **WHEN** `Sales |> tail(3)` is evaluated
- **THEN** the result is the last three rows

#### Scenario: A shifted column

- **GIVEN** a sheet whose `Revenue` is `[100, 120, 150]`
- **WHEN** the computed column `Prev := Sales.Revenue.shift(1)` is calculated
- **THEN** `Prev` is empty, `100`, `120`
- **AND** it equals `Sales[Revenue; *-1]`

### Requirement: The null functions

`is_null(x)` and `is_not_null(x)` SHALL say whether a value is empty, of one
value or of each of a column's.

`fill_null(x, v)` SHALL give `v` where `x` is empty, and `is_in(x, v)` SHALL
say whether `x` is among `v`. Each SHALL mean exactly what the operators `//`
and `in` already mean, so that a Polars reader finds the name and an OMX
reader keeps the operator — and each SHALL therefore take one value, as those
operators do, rather than a whole column.

#### Scenario: A function and its operator agree

- **WHEN** `fill_null(Sales[Note; 1], "none")` and
  `Sales[Note; 1] // "none"` are evaluated
- **THEN** the results are equal
- **AND** `is_in(Sales[Region; 0], ["UK", "US"])` equals
  `Sales[Region; 0] in ["UK", "US"]`

#### Scenario: Each cell of a column

- **GIVEN** a column `Note` whose second cell is empty
- **WHEN** `Sales.Note.is_null()` is evaluated
- **THEN** the result is `[false, true, false]`

### Requirement: Casting

`cast(x, T)` SHALL convert `x` to the type `T`, the Polars spelling of a
conversion, alongside `T(x)`, `x.T()` and `x.T`, which are unchanged.

#### Scenario: The four spellings agree

- **WHEN** `cast(19.99, Int)`, `Int(19.99)`, `(19.99).Int()` and `(19.99).Int`
  are evaluated
- **THEN** every result is the `Int` `19`
