# numerics

The OMX numeric tower, lifted from Raku: exact by default, approximate by
explicit choice.

## ADDED Requirements

### Requirement: Numeric tower
OMX SHALL provide the numeric types `Int` (arbitrary-precision integer), `Ratio`
(arbitrary-precision rational, numerator and denominator both unbounded, always
in lowest terms), `Num` (IEEE 754 double) and `Complex` (pair of `Num`). OMX SHALL
NOT expose fixed-width integer types.

#### Scenario: Large integers do not overflow
- **WHEN** `10**1000` is evaluated
- **THEN** the result is an `Int` with 1001 digits

#### Scenario: Large rationals stay exact
- **WHEN** `999999999999999999999 / 7` is evaluated
- **THEN** the result is the exact `Ratio` `999999999999999999999/7`

### Requirement: Literal types
An integer literal SHALL be an `Int`. A decimal literal SHALL be the exactly
equal `Ratio`. A literal with an exponent (`1e-100`) SHALL be a `Num`. Digits MAY
be grouped with `_`. A number immediately followed by `%` SHALL be that number
divided by 100, exactly. A number immediately followed by `i` SHALL be an
imaginary `Complex`, so that `3+4i` is the `Complex` with real part 3 and
imaginary part 4. A cell holding such a number, with no `=`, SHALL be a `Complex`
literal, and a real number in a column declared `Complex` SHALL be that number
with no imaginary part.

#### Scenario: Complex literal
- **WHEN** `(1+2i) * (3-1i)` is evaluated
- **THEN** the result is the `Complex` `5+5i`

#### Scenario: Decimal literal is exact
- **WHEN** `42.5` is evaluated
- **THEN** the result is the `Ratio` `85/2`

#### Scenario: Digit separators
- **WHEN** `1_000_000.50` is evaluated
- **THEN** the result is the `Ratio` `2000001/2`

#### Scenario: Percent literal
- **WHEN** `20%` is evaluated
- **THEN** the result is exactly `1/5`

#### Scenario: Exponent literal is approximate
- **WHEN** `1e-100` is evaluated
- **THEN** the result is a `Num`

### Requirement: Exact arithmetic
`+`, `-`, `*` and `/` over `Int` and `Ratio` operands SHALL be exact. Division of
two `Int`s SHALL yield a `Ratio` (or an `Int` when the division is exact) and SHALL
NOT truncate or produce a `Num`.

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
empty staying empty. `Int` SHALL drop the fractional part. `Ratio` of a `Num`
SHALL be the decimal the `Num` is shown as. `Text` SHALL write a value in full,
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

#### Scenario: Text read as a number
- **WHEN** `"20%".Ratio` is evaluated
- **THEN** the result is exactly `1/5`
- **AND** `"abc".Int` is an error

#### Scenario: No conversion
- **WHEN** `Int(2025-01-31)` is checked
- **THEN** an error reports that `Int` cannot be applied to a `Date`

### Requirement: Value is independent of display
The stored value of a number SHALL NOT be altered by how it is displayed.
A `Ratio` SHALL display by default as a decimal with up to five digits after the
point. One with more SHALL be rounded to five, a half away from zero, and
marked with `…`. On request a `Ratio` SHALL display in full: as a decimal when
its expansion terminates, otherwise as a fraction. A `Num` SHALL display as
Raku displays one: a whole number with no decimal point, any other as a decimal,
and one of magnitude `1e15` or more, or less than `1e-4`, with a signed exponent
of at least two digits. By default more than five digits after the point, or
after the point of the part before the exponent, SHALL be rounded and marked as
a `Ratio`'s are; on request a `Num` SHALL display in full, with the fewest
digits that give the same `Num` back. Not-a-number and the infinities SHALL display as `NaN`, `Inf` and
`-Inf`. A `Complex` SHALL display each part as a `Num`, as `3+4i`. How a number
is displayed SHALL NOT change its type.

#### Scenario: Terminating rational
- **WHEN** the exact value `175/4` is displayed with default formatting
- **THEN** it is shown as `43.75`

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
