# numerics

What a number is, and what arithmetic on one gives.

## MODIFIED Requirements

### Requirement: An omasheet type names a Polars dtype

Each type SHALL be a name for one Polars dtype, and SHALL hold exactly what
that dtype holds. `Int` SHALL be `Int64`, `Decimal` and `Percent` SHALL be
`Decimal(38, s)`, `Float` SHALL be `Float64`, `Boolean` SHALL be `Boolean`,
`String` SHALL be `String`, `Binary` SHALL be `Binary`, `Date` SHALL be `Date`,
`Time` SHALL be `Time`, `DateTime` SHALL be `Datetime` with an optional zone,
`Duration` SHALL be `Duration`, and `Null` SHALL be `Null`.

A file MAY declare any of those names. Unsigned dtypes SHALL be read and
preserved but SHALL NOT be namable.

#### Scenario: A column keeps the dtype it was read with

- **GIVEN** a source whose schema says a column is `Int32`
- **WHEN** the file is read and written back
- **THEN** the column is still `Int32`
- **AND** it is reported as an `Int`

#### Scenario: An unsigned value past the signed range is reported

- **GIVEN** a `UInt64` column holding a value above `i64::MAX`
- **WHEN** it is read
- **THEN** the value is preserved
- **AND** a notice says it does not fit a signed 64-bit integer

### Requirement: Exact decimal arithmetic

Adding, subtracting and multiplying two exact numbers SHALL give the exact
result whenever it fits in 38 digits. Division SHALL be exact where it
terminates within 38 digits, and SHALL otherwise round half away from zero at
the widest scale the result will hold.

Arithmetic SHALL NOT wrap. A result that will not fit SHALL be reported.

The last 8 of the 38 digits SHALL be guard digits: held, but neither compared
nor shown. A number SHALL be compared, shown and written at 30 significant
digits, and a number of 30 or fewer significant digits SHALL be untouched by
this.

#### Scenario: Decimal addition is exact

- **GIVEN** the expression `0.1 + 0.2`
- **WHEN** it is evaluated
- **THEN** the result is exactly `0.3`
- **AND** it is not the floating-point `0.30000000000000004`

#### Scenario: A division that does not terminate is rounded, and says so by its width

- **GIVEN** the expression `1 / 3`
- **WHEN** it is evaluated
- **THEN** the result holds 38 digits after the point
- **AND** 30 of them are shown, cut and marked

#### Scenario: The guard digits carry the rounding, so two ways to one number agree

- **GIVEN** the expressions `1/7 + 1/7` and `2/7`
- **WHEN** each is evaluated
- **THEN** the two values differ in the digits they hold
- **AND** `1/7 + 1/7 == 2/7` is true
- **AND** `2/7 + 3/7 == 5/7` is true
- **AND** `(1 / 3) * 3` is `1`

#### Scenario: Guarding is on significant digits, so the smallest numbers survive

- **GIVEN** the exact number `1e-38`, whose every digit is significant
- **WHEN** it is shown and compared
- **THEN** it is itself, and not zero

#### Scenario: A result too wide to hold is reported

- **GIVEN** two exact numbers whose product needs more than 38 digits
- **WHEN** they are multiplied
- **THEN** a diagnostic says the number is too large
- **AND** no wrapped or truncated value is stored

### Requirement: Exactness is decimal, not rational

An exact number SHALL be a decimal. There SHALL be no rational type: a value
shown as a fraction SHALL be a decimal whose fraction was recovered from it,
not a numerator and a denominator that were held.

A fraction of two whole numbers, and a whole number followed by a part, SHALL
remain literal syntax and SHALL be read as the exact decimal they come to,
rounded at 38 digits where they do not terminate.

#### Scenario: A fraction in a cell is an exact decimal

- **GIVEN** a cell holding `2 3/4`
- **WHEN** the file is read
- **THEN** the value is the exact decimal `2.75`
- **AND** the column is a `Decimal`

#### Scenario: A fraction that does not terminate is rounded

- **GIVEN** a cell holding `1/7`
- **WHEN** the file is read
- **THEN** the value holds 38 digits after the point
- **AND** it is shown cut and marked, unless the column is a `Fraction`

## ADDED Requirements

### Requirement: The Fraction type is a display of a Decimal

`Fraction` SHALL be a declarable type holding what a `Decimal` holds, shown as
a fraction of whole numbers with a whole part where there is one. A `Fraction`
column SHALL take any exact number, and anything worked out from a fraction
SHALL be a fraction.

#### Scenario: A Fraction column shows what it holds as a fraction

- **GIVEN** a `Fraction` column holding `1/7`, `2 3/4`, `0.5` and `3`
- **WHEN** the sheet is shown
- **THEN** the cells are `1/7`, `2 3/4`, `1/2` and `3`

#### Scenario: A column worked out from a fraction is a fraction

- **GIVEN** a `Fraction` column `Yield` and a column `Doubled := Yield * 2`
- **WHEN** the sheet is shown
- **THEN** `Doubled` shows fractions without being declared

### Requirement: A fraction is recovered, not remembered

The fraction SHALL be worked out from the number rather than remembered from
where it was written, so that one nobody wrote is found too. Only a
denominator of at most 10000 SHALL be looked for.

#### Scenario: A fraction nobody wrote is found

- **GIVEN** the expression `(2/7 + 3/7).Fraction`
- **WHEN** it is evaluated
- **THEN** it is shown as `5/7`

#### Scenario: Past the denominator looked for, it is a decimal

- **GIVEN** the expression `(1/10001).Fraction`
- **WHEN** it is evaluated
- **THEN** it is shown as a decimal, cut and marked

### Requirement: A fraction is written as a number everywhere but the file

`--exact` SHALL write a fraction as `n/d`, which is how an `.omx` file carries
one. XLSX, CSV, JSON and Parquet SHALL take the number it holds instead.

#### Scenario: A fraction goes to CSV as a decimal

- **GIVEN** a `Fraction` column holding `1/7`
- **WHEN** the sheet is exported as CSV
- **THEN** the field is the decimal, not `1/7`

### Requirement: Null is SQL-style

A blank cell SHALL be `Null`. Arithmetic with `Null` SHALL give `Null`,
comparison with it SHALL be unknown rather than true or false, and an aggregate
SHALL pass over it rather than counting it as zero. `Null` SHALL be shown as
the empty string.

#### Scenario: Null propagates through arithmetic

- **GIVEN** a row whose `Cost` cell is blank
- **WHEN** `Revenue - Cost` is calculated for that row
- **THEN** the result is `Null`
- **AND** the cell is shown empty

#### Scenario: An aggregate passes over nulls

- **GIVEN** a column of three numbers and two blank cells
- **WHEN** its sum and average are taken
- **THEN** the sum is the sum of the three
- **AND** the average divides by three, not five

### Requirement: Duration

`Duration` SHALL be a type with no literal syntax. The difference of two dates,
two times or two date-times SHALL be a `Duration`, and a `Duration` added to or
subtracted from a date SHALL give a date.

A whole number next to a date SHALL continue to count days, and next to a time
or a date-time, seconds.

#### Scenario: Two dates subtract to a duration

- **GIVEN** the expression `2025-03-01 - 2024-03-01`
- **WHEN** it is evaluated
- **THEN** the result is a `Duration` of 365 days
- **AND** it is not the number 365

#### Scenario: A whole number still counts days

- **GIVEN** the expression `2025-01-31 + 1`
- **WHEN** it is evaluated
- **THEN** the result is the date `2025-02-01`

## REMOVED Requirements

### Requirement: Arbitrary-precision integers

`Int` is no longer arbitrary precision. It is `Int64`, and a value past that
range is reported rather than held.

### Requirement: The Complex type

There is no `Complex` type, no imaginary literal (`4i`, `Inf\i`), and no `re`,
`im`, `conj` or `arg`. No Polars dtype holds a complex number; it may return as
a `Struct{re, im}`.
