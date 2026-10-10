# numerics

The OMX numeric tower, lifted from Raku: exact by default, approximate by
explicit choice.

## ADDED Requirements

### Requirement: Numeric tower
OMX SHALL provide the numeric types `Int` (arbitrary-precision integer), `Ratio`
(arbitrary-precision rational, numerator and denominator both unbounded, always
in lowest terms), `Decimal` (a `Ratio` whose decimal expansion terminates),
`Percent` (a `Ratio` displayed as a percentage), `Num` (IEEE 754 double) and
`Complex` (pair of `Num`). OMX SHALL NOT expose fixed-width integer types.

`Decimal` SHALL be a subtype of `Ratio`, and `Int` of `Decimal`: an `Int` SHALL
be accepted where a `Decimal` is declared, and a `Decimal` where a `Ratio` is.
A `Decimal` SHALL have no fixed number of digits.

A `Percent` SHALL hold any exact number. An `Int`, a `Decimal` or a `Ratio`
SHALL be accepted where a `Percent` is declared, and SHALL become a `Percent`
of the same value; a `Percent` SHALL be accepted where a `Ratio` is declared,
and SHALL become the plain number.

#### Scenario: Large integers do not overflow
- **WHEN** `10**1000` is evaluated
- **THEN** the result is an `Int` with 1001 digits

#### Scenario: Large rationals stay exact
- **WHEN** `999999999999999999999 / 7` is evaluated
- **THEN** the result is the exact `Ratio` `999999999999999999999/7`

### Requirement: Literal types
An integer literal SHALL be an `Int`. A literal with a decimal point SHALL be
the exactly equal `Decimal`. A fraction of two whole numbers written in a cell,
`1/7`, SHALL be a `Ratio`. A literal with an exponent (`1e-100`) SHALL be a `Num`. Digits MAY
be grouped with `_`. A number immediately followed by `%` SHALL be that number
divided by 100, exactly, as a `Percent`. A number immediately followed by `i` SHALL be an
imaginary `Complex`, so that `3+4i` is the `Complex` with real part 3 and
imaginary part 4. A cell holding such a number, with no `=`, SHALL be a `Complex`
literal, and a real number in a column declared `Complex` SHALL be that number
with no imaginary part.

#### Scenario: Complex literal
- **WHEN** `(1+2i) * (3-1i)` is evaluated
- **THEN** the result is the `Complex` `5+5i`

#### Scenario: Decimal literal is exact
- **WHEN** `42.5` is evaluated
- **THEN** the result is the `Decimal` `42.5`, exactly `85/2`

#### Scenario: Digit separators
- **WHEN** `1_000_000.50` is evaluated
- **THEN** the result is the `Decimal` exactly equal to `2000001/2`

#### Scenario: Percent literal
- **WHEN** `20%` is evaluated
- **THEN** the result is the `Percent` exactly equal to `1/5`, displayed as `20%`

#### Scenario: A fraction in a cell
- **WHEN** a cell holds `1/7`
- **THEN** it is a `Ratio`, and is an error in a column declared `Decimal`

#### Scenario: Exponent literal is approximate
- **WHEN** `1e-100` is evaluated
- **THEN** the result is a `Num`

### Requirement: Exact arithmetic
`+`, `-`, `*` and `/` over `Int`, `Decimal` and `Ratio` operands SHALL be exact.
Division of two `Int`s SHALL yield a `Ratio` (or an `Int` when the division is
exact) and SHALL NOT truncate or produce a `Num`.

The type of a sum, difference or product SHALL be the wider of its operands'
types, so that decimals stay `Decimal`. A quotient or a power of `Int`s or
`Decimal`s need not terminate, and SHALL be a `Ratio`; so SHALL an average of
them. A formula whose type is `Ratio` SHALL NOT be accepted in a column declared
`Decimal` without a conversion.

The sum or difference of two `Percent`s, the negation or absolute value of
one, and the sum, average, least or greatest of a vector of them SHALL be a
`Percent`. Any other arithmetic with a `Percent` SHALL treat it as the number
it stands for, and SHALL give a `Ratio` next to an exact operand.

#### Scenario: Percentages add up
- **WHEN** `20% + 5%` is evaluated
- **THEN** the result is the `Percent` `25%`

#### Scenario: A percentage of a number
- **WHEN** `100 * 20%` is evaluated
- **THEN** the result is `20`, not a `Percent`
- **AND** `20% == 0.2` is true

#### Scenario: A Percent column
- **WHEN** a column declared `Percent` has the formula `Profit / Revenue`, with `Profit` 5000 and `Revenue` 12000
- **THEN** the cell is the `Percent` exactly equal to `5/12`, displayed as `41.67…%`

#### Scenario: Decimals stay decimal
- **WHEN** `19.99 * 3 - 0.5` is checked
- **THEN** its type is `Decimal`

#### Scenario: A quotient is a Ratio
- **WHEN** `19.99 / 3` is checked
- **THEN** its type is `Ratio`, and a column declared `Decimal` rejects it

#### Scenario: Decimal sum
- **WHEN** `0.1 + 0.2` is evaluated
- **THEN** the result is exactly `3/10` and `0.1 + 0.2 == 0.3` is true

#### Scenario: Thirds round-trip
- **WHEN** `1 / 3 * 3` is evaluated
- **THEN** the result is exactly `1`

#### Scenario: Money arithmetic
- **WHEN** `19.99 * 3` is evaluated
- **THEN** the result is exactly `5997/100`

### Requirement: No silent loss of exactness
The engine SHALL NOT convert an `Int` or `Ratio` to a `Num` implicitly, regardless
of the size of the numerator or denominator. Conversion to `Num` SHALL happen
only through `Num(x)`, also written `x.Num`, or when an operand is already a
`Num`. Any operation with a `Num` operand SHALL yield a `Num`.

#### Scenario: Explicit approximation
- **WHEN** `(1/3).Num` is evaluated
- **THEN** the result is a `Num` close to `0.3333333333333333`

#### Scenario: Num is contagious
- **WHEN** `Num(1) / 3` is evaluated
- **THEN** the result is a `Num`

#### Scenario: Growing denominators stay rational
- **WHEN** `1/3 + 1/7 + 1/11 + 1/13 + 1/17` is evaluated
- **THEN** the result is an exact `Ratio`

### Requirement: Conversions
The name of each type SHALL be a conversion to that type, written `Int(x)`,
`x.Int()` or `x.Int`, applied to one value or to each value of a vector, with
empty staying empty. `Int` SHALL drop the fractional part. `Ratio` or `Decimal`
of a `Num` SHALL be the decimal the `Num` is shown as. `Decimal` of a `Ratio`
SHALL be that number when it terminates, and an error when it does not. `Text` SHALL write a value in full,
the way a sheet writes it. Text SHALL be read the way a cell is, and SHALL be an
error when it is not a value of the type. `Bool` of a number SHALL be whether it
is not zero, and a number of a `Bool` SHALL be `1` or `0`. `Date` and `Time`
SHALL take that half of a `DateTime`, and `DateTime` of a `Date` SHALL be its
midnight. A conversion between types that have none SHALL be reported before
anything is calculated.

#### Scenario: A fraction as a whole number
- **WHEN** `(-19.99).Int` is evaluated
- **THEN** the result is the `Int` `-19`

#### Scenario: A Num as an exact number
- **WHEN** `Ratio(1e-1)` is evaluated
- **THEN** the result is exactly `1/10`

#### Scenario: A number as a Percent
- **WHEN** `(1/8).Percent` is evaluated
- **THEN** the result is the `Percent` `12.5%`
- **AND** `(1/3).Percent` is the `Percent` exactly equal to `1/3`, not an error

#### Scenario: A Ratio as a Decimal
- **WHEN** `(1/8).Decimal` is evaluated
- **THEN** the result is the `Decimal` `0.125`
- **AND** `(1/3).Decimal` is an error saying that `1/3` does not end as a decimal

#### Scenario: Text read as a number
- **WHEN** `"20%".Ratio` is evaluated
- **THEN** the result is exactly `1/5`
- **AND** `"abc".Int` is an error

#### Scenario: No conversion
- **WHEN** `Int(2025-01-31)` is checked
- **THEN** an error reports that `Int` cannot be applied to a `Date`

### Requirement: Value is independent of display
The stored value of a number SHALL NOT be altered by how it is displayed.
An exact number SHALL always display in full: a `Decimal`, a `Ratio` whose
expansion terminates, as that decimal, and any other `Ratio` as a fraction
`x/y` in lowest terms. A `Percent` SHALL display as its value times 100
followed by `%`: in full when that terminates, and otherwise rounded to two
digits after the point, a half away from zero, and marked with `…` before the
`%`; on request it SHALL display in full as a fraction followed by `%`. A `Num` SHALL display as
Raku displays one: a whole number with no decimal point, any other as a decimal,
and one of magnitude `1e15` or more, or less than `1e-4`, with a signed exponent
of at least two digits. By default more than five digits after the point, or
after the point of the part before the exponent, SHALL be rounded to five, a
half away from zero, and marked with `…`; on request a `Num` SHALL display in full, with the fewest
digits that give the same `Num` back. Not-a-number and the infinities SHALL display as `NaN`, `Inf` and
`-Inf`. A `Complex` SHALL display each part as a `Num`, as `3+4i`. How a number
is displayed SHALL NOT change its type.

#### Scenario: Terminating rational
- **WHEN** the exact value `175/4` is displayed with default formatting
- **THEN** it is shown as `43.75`

#### Scenario: A rational that does not end
- **WHEN** `omasheet eval '1/3 + 1/3'` is run
- **THEN** the output is `2/3`

#### Scenario: A Percent that does not end
- **WHEN** `omasheet eval '(5/12).Percent'` is run
- **THEN** the output is `41.67…%`
- **AND** `omasheet eval --exact '(5/12).Percent'` outputs `125/3%`

#### Scenario: A Num is shown as Raku shows it
- **WHEN** `omasheet eval '(3/2).Num'` is run
- **THEN** the output is `1.5`
- **AND** `omasheet eval '4.Num'` outputs `4`, and `omasheet eval '1e21'` outputs `1e+21`

#### Scenario: A Num with many digits
- **WHEN** `omasheet eval 'sqrt(2)'` is run
- **THEN** the output is `1.41421…`
- **AND** `omasheet eval --exact 'sqrt(2)'` outputs `1.4142135623730951`

#### Scenario: Non-terminating rational
- **WHEN** `omasheet eval '1/3 + 1/3'` is run
- **THEN** the output is `0.66667…`
- **AND** `omasheet eval --exact '1/3 + 1/3'` outputs `2/3`

#### Scenario: Rounded display keeps exact value
- **GIVEN** a cell holding `100/3` displayed to two decimal places as `33.33`
- **WHEN** another cell multiplies it by `3`
- **THEN** the result is exactly `100`
