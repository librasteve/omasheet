# numerics

## ADDED Requirements

### Requirement: Numeric tower
OMX SHALL provide the numeric types `Int` (arbitrary-precision integer), `Fraction`
(arbitrary-precision rational, numerator and denominator both unbounded, always
in lowest terms), `Decimal` (a `Fraction` whose decimal expansion terminates),
`Percent` (a `Fraction` displayed as a percentage), `Num` (IEEE 754 double)
and `Complex` (pair of `Num`). OMX SHALL NOT expose fixed-width integer types.

#### Scenario: Large integers do not overflow
- **WHEN** `10**1000` is evaluated
- **THEN** the result is an `Int` with 1001 digits

#### Scenario: Large rationals stay exact
- **WHEN** `999999999999999999999 / 7` is evaluated
- **THEN** the result is the exact `Fraction` `999999999999999999999/7`

### Requirement: Exact subtypes
`Decimal` SHALL be a subtype of `Fraction`, and `Int` of `Decimal`: an `Int` SHALL
be accepted where a `Decimal` is declared, and a `Decimal` where a `Fraction` is.
A `Decimal` SHALL have no fixed number of digits. A `Percent` SHALL hold any
exact number. An `Int`, a `Decimal` or a `Fraction` SHALL be accepted where a
`Percent` is declared, and SHALL become a `Percent` of the same value; a
`Percent` SHALL be accepted where a `Fraction` is declared, and SHALL become
the plain number.

#### Scenario: A whole number where a Decimal is declared
- **WHEN** a column declared `Decimal` holds `9` and `4.50`
- **THEN** both are accepted

#### Scenario: A number where a Percent is declared
- **WHEN** a column declared `Percent` holds `0.5`
- **THEN** the cell is the `Percent` `50%`

### Requirement: Number literals
An integer literal SHALL be an `Int`, and a literal with a decimal point the
exactly equal `Decimal`. A literal with an exponent (`1e-100`) SHALL be a
`Num`, and so SHALL `Inf` and `NaN`; text of the same SHALL be written in
quotes, `"Inf"`. Digits MAY be grouped with `_`.

#### Scenario: Decimal literal is exact
- **WHEN** `42.5` is evaluated
- **THEN** the result is the `Decimal` `42.5`, exactly `85/2`

#### Scenario: Digit separators
- **WHEN** `1_000_000.50` is evaluated
- **THEN** the result is the `Decimal` exactly equal to `2000001/2`

#### Scenario: Exponent literal is approximate
- **WHEN** `1e-100` is evaluated
- **THEN** the result is a `Num`

### Requirement: Fraction literals
A fraction of two whole numbers written in a cell, `1/7`, SHALL be a
`Fraction`, whether or not it is more than one, `22/7`; so SHALL a whole
number, a space and a fraction of less than one, `2 3/4`, which SHALL be their
sum, `11/4`.

#### Scenario: A fraction in a cell
- **WHEN** a cell holds `1/7`
- **THEN** it is a `Fraction`, and in a column declared `Decimal` it is shown as a decimal, `0.14285714285714285714…`

#### Scenario: A top-heavy fraction
- **WHEN** a cell holds `22/7`
- **THEN** it is the `Fraction` exactly equal to `22/7`, displayed as `3 1/7`

#### Scenario: A whole number and a part
- **WHEN** a cell holds `2 3/4`
- **THEN** it is the `Fraction` exactly equal to `11/4`, displayed as `2 3/4`
- **AND** `2 7/4` is not a number

#### Scenario: A Fraction column
- **WHEN** a column declared `Fraction` holds `2.75` and `0.5`
- **THEN** the cells are displayed as `2 3/4` and `1/2`

### Requirement: Percent and Complex literals
A number immediately followed by `%` SHALL be that number divided by 100,
exactly, as a `Percent`. A number immediately followed by `i` SHALL be an
imaginary `Complex`, so that `3+4i` is the `Complex` with real part 3 and
imaginary part 4; a cell holding one, with no `=`, SHALL be a `Complex`
literal, and a real number in a column declared `Complex` SHALL be that number
with no imaginary part.

#### Scenario: Percent literal
- **WHEN** `20%` is evaluated
- **THEN** the result is the `Percent` exactly equal to `1/5`, displayed as `20%`

#### Scenario: Complex literal
- **WHEN** `(1+2i) * (3-1i)` is evaluated
- **THEN** the result is the `Complex` `5+5i`

### Requirement: Time literals
Two digits on each side of a `:`, `16:10`, SHALL be a `Time`, and any other
pair of whole numbers with a `:` between SHALL be an error.

#### Scenario: A time, not a ratio
- **WHEN** `16:10` is evaluated
- **THEN** the result is the `Time` ten past four in the afternoon
- **AND** `3:2` is an error

### Requirement: Exact arithmetic
`+`, `-`, `*` and `/` over `Int`, `Decimal` and `Fraction` operands SHALL be exact.
Division of two `Int`s SHALL yield a `Fraction` (or an `Int` when the division is
exact) and SHALL NOT truncate or produce a `Num`, except that division by zero,
which no exact number is, SHALL yield the `Num` `Inf`, `-Inf` by the sign of
what is divided, or `NaN` when that is zero. A `NaN` SHALL NOT be equal to,
less than or more than any value, itself included.

#### Scenario: Division by zero
- **WHEN** `1/0`, `-1/0` and `0/0` are evaluated
- **THEN** the results are the `Num`s `Inf`, `-Inf` and `NaN`
- **AND** `NaN == NaN` is `false` and `NaN != NaN` is `true`

The type of a sum, difference or product SHALL be the wider of its operands'
types, so that decimals stay `Decimal`. A quotient or a power of `Int`s or
`Decimal`s need not terminate, and SHALL be a `Fraction`; so SHALL an average of
them. A power whose exponent is written as a whole number, `2 ** 64`, cannot
be negative, and SHALL keep the type of its base. A formula whose type is
`Fraction` SHALL NOT be accepted in a column declared `Decimal` without a
conversion.

The sum or difference of two `Percent`s, the negation or absolute value of
one, and the sum, average, least or greatest of a vector of them SHALL be a
`Percent`. Any other arithmetic with a `Percent` SHALL treat it as the number
it stands for, and SHALL give a `Fraction` next to an exact operand.

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

#### Scenario: A quotient is a Decimal
- **WHEN** `19.99 / 3` is checked
- **THEN** its type is `Decimal`, and a column declared `Decimal` takes it
- **AND** it is held exactly, `1999/300`, and shown cut and marked, `6.66333333333333333333…`

#### Scenario: Decimal sum
- **WHEN** `0.1 + 0.2` is evaluated
- **THEN** the result is exactly `3/10` and `0.1 + 0.2 == 0.3` is true

#### Scenario: Thirds round-trip
- **WHEN** `1 / 3 * 3` is evaluated
- **THEN** the result is exactly `1`

#### Scenario: Money arithmetic
- **WHEN** `19.99 * 3` is evaluated
- **THEN** the result is exactly `5997/100`

#### Scenario: A power with a whole exponent
- **WHEN** `2 ** 64` and `1.1 ** 2` are checked
- **THEN** their types are `Int` and `Decimal`
- **AND** `2 ** -1` is a `Decimal`, and so is a power whose exponent is read
  from a column

### Requirement: Numeric infectiousness
As in Raku, arithmetic on numbers of two types SHALL give the more infectious
of the two. From least to most infectious the types SHALL be `Int`, `Decimal`,
`Fraction` and `Percent` alike, `Num`, then `Complex`. The exact types SHALL
be held as unbounded rationals, so that, unlike a Raku `Rat`, no exact number
SHALL become a `Num` because its denominator grows. A `Percent` SHALL stay a
`Percent` only when added to or taken from another, and SHALL otherwise act
as the plain number, a `Fraction`.

#### Scenario: A Complex or a Num infects
- **WHEN** `1/3 + 0.5.Num` and `1/3 + 1i` are evaluated
- **THEN** the results are a `Num` and a `Complex`

#### Scenario: A Fraction infects
- **WHEN** a column declared `Fraction` holds `0.5`, and another doubles it
- **THEN** the double is displayed as `1`, and `0.25` doubled as `1/2`

#### Scenario: Percentages and other numbers
- **WHEN** `20% + 5%` and `100 * 20%` are evaluated
- **THEN** the results are the `Percent` `25%` and the plain number `20`

#### Scenario: Whole numbers and decimals
- **WHEN** `2 * 1.5`, `7 / 2` and `2 ** 64` are checked
- **THEN** their types are `Decimal`, `Fraction` and `Int`

### Requirement: Type and value
Each expression SHALL have a type before it is calculated, by the rules of
numeric infectiousness: the type shown under a column's name, and checked
against a declared type. A value SHALL be held as the narrowest exact type that
holds it: a whole number as an `Int`, one that terminates as a `Decimal`, and
any other as a `Fraction`, unless a `Fraction` operand keeps it one. A value
MAY so be narrower than the type of its expression, and SHALL NOT be wider.

#### Scenario: The narrowest value in a wider type
- **WHEN** `4/2`, `7/4` and `7/3` are evaluated
- **THEN** the results are `2`, `1.75` and `2 1/3`
- **AND** each expression is of type `Fraction`, where Raku would give the `Rat` `2.0` for `4/2`

### Requirement: No silent loss of exactness
The engine SHALL NOT convert an `Int` or `Fraction` to a `Num` implicitly, regardless
of the size of the numerator or denominator. Conversion to `Num` SHALL happen
only through `Num(x)`, also written `x.Num`, or when an operand is already a
`Num`. Any operation with a `Num` operand SHALL yield a `Num`. Otherwise only
a literal with an exponent, `Inf` or `NaN`, a division by zero, or a function
with no exact result SHALL make a `Num` of exact operands.

#### Scenario: Explicit approximation
- **WHEN** `(1/3).Num` is evaluated
- **THEN** the result is a `Num` close to `0.3333333333333333`

#### Scenario: Num is contagious
- **WHEN** `Num(1) / 3` is evaluated
- **THEN** the result is a `Num`

#### Scenario: Growing denominators stay rational
- **WHEN** `1/3 + 1/7 + 1/11 + 1/13 + 1/17` is evaluated
- **THEN** the result is an exact `Fraction`

### Requirement: Conversions
The name of each type SHALL be a conversion to that type, written `Int(x)`,
`x.Int()` or `x.Int`, applied to one value or to each value of a vector, with
empty staying empty. `Int` SHALL drop the fractional part. `Fraction` or
`Decimal` of a `Num` SHALL be the decimal the `Num` is shown as in full.
`Decimal` of a `Fraction` SHALL be that number when it terminates, and an
error when it does not. A conversion between types that have none SHALL be
reported before anything is calculated.

#### Scenario: A fraction as a whole number
- **WHEN** `(-19.99).Int` is evaluated
- **THEN** the result is the `Int` `-19`

#### Scenario: A Num as an exact number
- **WHEN** `Fraction(1e-1)` is evaluated
- **THEN** the result is exactly `1/10`

#### Scenario: A number as a Percent
- **WHEN** `(1/8).Percent` is evaluated
- **THEN** the result is the `Percent` `12.5%`
- **AND** `(1/3).Percent` is the `Percent` exactly equal to `1/3`, not an error

#### Scenario: A Fraction as a Decimal
- **WHEN** `(1/8).Decimal` is evaluated
- **THEN** the result is the `Decimal` `0.125`
- **AND** `(1/3).Decimal` is the `Decimal` exactly equal to `1/3`, shown as `0.33333333333333333333…`

#### Scenario: No conversion
- **WHEN** `Int(2025-01-31)` is checked
- **THEN** an error reports that `Int` cannot be applied to a `Date`

### Requirement: Conversions of text, truth and time
`Text` SHALL write a value in full, the way a file writes it, and text SHALL
be read the way a cell is, an error when it is not a value of the type. `Bool`
of a number SHALL be whether it is not zero, and a number of a `Bool` SHALL be
`1` or `0`. `Date` and `Time` SHALL take that half of a `DateTime`, and
`DateTime` of a `Date` SHALL be its midnight.

#### Scenario: Text read as a number
- **WHEN** `"20%".Fraction` is evaluated
- **THEN** the result is exactly `1/5`
- **AND** `"abc".Int` is an error

#### Scenario: A number as a Bool
- **WHEN** `0.Bool` and `0.5.Bool` are evaluated
- **THEN** the results are `false` and `true`

### Requirement: Value is independent of display
The stored value of a number SHALL NOT be altered by how it is displayed, and
how a number is displayed SHALL NOT change its type. A value declared or
converted `Fraction`, a fraction written in a cell, and the result of
arithmetic with one SHALL be a `Fraction`. The result of other exact
arithmetic SHALL be a `Decimal` when it terminates and a `Fraction` when it
does not.

#### Scenario: Rounded display keeps exact value
- **GIVEN** a cell holding `100/3`
- **WHEN** another cell multiplies it by `3`
- **THEN** the result is exactly `100`

### Requirement: Display in full
On request, and as text, a number SHALL display in full, as it is written in
a file: an exact number exactly, and a `Num` with the fewest digits that give
the same `Num` back. In the app, pointing at a number shown in fewer
characters than that SHALL show it in full.

#### Scenario: In full on request
- **WHEN** `omasheet eval --exact '(5/12).Percent'` is run
- **THEN** the output is `125/3%`
- **AND** `omasheet eval --exact '11/4'` outputs `11/4`

### Requirement: Display width and bands
A number SHALL be displayed in at most 23 characters. The rules SHALL apply to
the magnitude of a number, and a negative one SHALL be shown with `-` before
it. Every type SHALL have the same three bands, by magnitude: Small, less than
`1e-15` and not zero; Medium, zero and from `1e-15` to less than `1e15`; and
Large, `1e15` or more. A number in the Medium band SHALL be shown plain, as a
decimal with no exponent, and one in the Small or Large band in e notation.

#### Scenario: Small whole numbers in full
- **WHEN** `999999999999999` is displayed
- **THEN** it is shown as `999999999999999`
- **AND** `10**15` is shown as `1e+15`

#### Scenario: A small Decimal does not look like zero
- **WHEN** `0.0000000000000000000000000000000000000000001` is displayed
- **THEN** it is shown as `1e-43`

### Requirement: E notation
A number in e notation SHALL be shown as a mantissa of one digit, a point and
the rest of its significant digits, then `e`, a sign and an exponent of at
least two digits, as `1.25e-16`; a mantissa of one significant digit SHALL
have no point, as `1e+15`. An `Int`, a `Decimal` or a `Fraction` SHALL keep
every significant digit, with trailing zeros in the exponent.

#### Scenario: An exact large Int
- **WHEN** `123456789012345678` is displayed
- **THEN** it is shown as `1.23456789012345678e+17`, every digit kept
- **AND** `10**60` is shown as `1e+60`

### Requirement: A number too wide
A number whose plain form is wider than 23 characters SHALL be shown in e
notation, whatever its band. Where that is still wider than 23 characters, its
mantissa SHALL be cut, not rounded, and marked `…`; the exponent SHALL NOT be
cut.

#### Scenario: An Int cut to fit
- **WHEN** `2**64` is displayed
- **THEN** it is shown as `1.8446744073709551…e+19`

#### Scenario: A Decimal too wide to be plain
- **WHEN** `0.12345678901234567890123` is displayed
- **THEN** it is shown as `1.2345678901234567…e-01`

### Requirement: Digit grouping
In the app, the digits of a plain number before its point SHALL be grouped in
threes where there are five of them or more, `12,345.678`, and the
separators SHALL count toward the 23 characters. A number in e notation, the
part of a `Fraction` after its whole number, and digits after the point SHALL
NOT be grouped. A file, `print`, `eval`, text, export, copying and the full
display SHALL NOT group digits; in a file digits MAY be grouped with
`_`, `10_000`, as a literal allows.

#### Scenario: Grouped in the app only
- **WHEN** `12345` and `2025` are shown in the app, in a British locale
- **THEN** they are shown as `12,345` and `2025`
- **AND** `omasheet eval '12345'` outputs `12345`

### Requirement: Numbers in the locale
In the app, a number SHALL be shown with the decimal point and the digit
group separator of the user's locale (`LC_NUMERIC`), as dates and times
follow its `LC_TIME`. Where the locale has no separator, digits SHALL be
grouped by `,`, or by a narrow space where its point is `,`. A file, `print`,
`eval`, text, export, copying and the full display SHALL always write a
number with `.` and no separator.

#### Scenario: A German locale
- **WHEN** `12345.5` and `5/12` as a `Percent` are shown in the app, in a German locale
- **THEN** they are shown as `12.345,5` and `41,67%`
- **AND** copying the first gives `12345.5`

#### Scenario: Grouping can make a number too wide to be plain
- **WHEN** `999999999999999.9999999` is shown in the app
- **THEN** it is shown as `9.9999999999999999…e+14`, as `999,999,999,999,999.9999999` would be 27 characters

### Requirement: Fraction display
A `Fraction` SHALL be shown as a fraction when that fits in 23 characters: one
of less than one as `x/y`, and one of more than one as a whole number and a
part with a space between, `2 3/4`. One that does not fit SHALL be shown in e
notation as the decimal it stands for; as that does not terminate, it SHALL
always be cut and marked `…`.

#### Scenario: A fraction that fits
- **WHEN** `1000/3` is displayed
- **THEN** it is shown as `333 1/3`
- **AND** `1/3 + 1/3` is shown as `2/3`, and `175/4` as the `Decimal` `43.75`

#### Scenario: A fraction too wide to be one
- **WHEN** `12345666543218/76543141111111` is displayed
- **THEN** it is shown as `1.6129030457865418…e-01`

### Requirement: Percent display
A `Percent` SHALL be shown as its value times 100, rounded to two digits after
the point, a half away from zero, with trailing zeros and point dropped, then
`%`. Its bands SHALL be those of the value times 100, and the `%` SHALL count
toward the 23 characters. It SHALL NOT be marked `…`: one of less than
`0.005%` SHALL be shown as `0%`, and one of `1e15%` or more in e notation with
its mantissa rounded to two digits after the point.

#### Scenario: A Percent is rounded, not marked
- **WHEN** `(5/12).Percent` is displayed
- **THEN** it is shown as `41.67%`
- **AND** `(1/8).Percent` is shown as `12.5%`, `20%` as `20%`, and `0.000001%` as `0%`

#### Scenario: A large Percent
- **WHEN** `(10**18 / 7).Percent` is displayed
- **THEN** it is shown as `1.43e+19%`

### Requirement: Num display
A `Num` SHALL be shown with the fewest digits that give the same `Num` back,
never rounded, and SHALL never be marked `…`. Not-a-number and the infinities
SHALL be shown as `NaN`, `Inf` and `-Inf`.

#### Scenario: A Num in full, never rounded
- **WHEN** `0.1.Num + 0.2.Num` is displayed
- **THEN** it is shown as `0.30000000000000004`
- **AND** `sqrt(2)` is shown as `1.4142135623730951`, `(3/2).Num` as `1.5`, and `4.Num` as `4`

#### Scenario: A Num switches to e notation at the band edges
- **WHEN** `1e15` and `999999999999999.9.Num` are displayed
- **THEN** they are shown as `1e+15` and `999999999999999.9`
- **AND** `1e-15` is shown as `0.000000000000001`, and `9.999999999999999e-16` as itself

#### Scenario: A Num too wide to be plain
- **WHEN** `1.2345678e-15` is displayed
- **THEN** it is shown as `1.2345678e-15`, its plain form being 24 characters

### Requirement: Complex display
A `Complex` SHALL be shown as its real part, the sign of its imaginary part,
the magnitude of that, and `i`, each part shown as a `Num`. Where that is
wider than 23 characters, both parts SHALL be cut equally, each marked `…`,
until it fits.

#### Scenario: A Complex that fits
- **WHEN** `3+4i` and `1i` are displayed
- **THEN** they are shown as `3+4i` and `0+1i`

#### Scenario: A Complex too wide for its parts
- **WHEN** `(1/3).Num + (1/3).Num * 1i` is displayed
- **THEN** it is shown as `0.3333333…+0.3333333…i`
