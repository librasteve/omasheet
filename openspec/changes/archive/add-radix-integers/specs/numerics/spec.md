# numerics

Hex, Octal and Binary integers.

## ADDED Requirements

### Requirement: Based integer literals
`0x` followed by hex digits, `0o` followed by octal digits, and `0b` followed by
binary digits SHALL each be an integer, of type `Hex`, `Octal` or `Binary`.
The prefix and the hex digits SHALL be read in either case. A digit outside
the base, or a prefix with no digit after it, SHALL be an error.

#### Scenario: Literals
- **WHEN** `0xFF`, `0o17` and `0b101` are evaluated
- **THEN** they are the `Hex` 255, the `Octal` 15 and the `Binary` 5

#### Scenario: A digit outside the base
- **WHEN** `0b102` is evaluated
- **THEN** it is an error at the `2`

### Requirement: Digit separator
`_` SHALL be read anywhere among the digits of a number, before or after the
point and in a `Hex`, `Octal` or `Binary`, and SHALL NOT be read in an
exponent.

#### Scenario: Separators
- **WHEN** `10_00_000`, `0.000_1` and `0xFF_FF` are read
- **THEN** they are a million, `0.0001` and the `Hex` 65535
- **AND** `1e1_0` is an error

### Requirement: Subtypes of Integer
`Hex`, `Octal` and `Binary` SHALL hold the numbers an `Integer` holds and
SHALL be assignable to and from `Integer`. Arithmetic on two values of the
same base SHALL give that base, and on any other mix SHALL give `Integer`.

#### Scenario: Arithmetic
- **WHEN** `0xF0 + 0x0F` and `0xF0 + 1` are evaluated
- **THEN** the first is the `Hex` 255 and the second the `Integer` 241

### Requirement: Based integer display
A `Hex`, `Octal` or `Binary` SHALL be shown as its lower-case prefix followed
by its digits in that base, in lower case. A negative one SHALL be shown as
its 64-bit two's complement, and SHALL be written in the file as `-` and then
the prefix. A format on its column SHALL replace this display.

#### Scenario: Display
- **WHEN** `0xFF`, `-0xFF` and `Binary(5)` are displayed
- **THEN** they are shown as `0xff`, `0xffffffffffffff01` and `0b101`
- **AND** `-0xFF` is written in the file as `-0xff`
