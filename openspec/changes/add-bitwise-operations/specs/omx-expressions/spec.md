# omx-expressions

Bitwise operators.

## ADDED Requirements

### Requirement: Bitwise operators
`+&`, `+|` and `+^` between two whole numbers, and `+^` before one, SHALL
operate on their 64-bit two's complement bits as and, or, xor and
complement. Any operand that is not a whole number SHALL be an error.
`and`, `or` and `not` SHALL remain Boolean.

#### Scenario: Masking
- **WHEN** `0xF0 +& 0x3C`, `0xF0 +| 0x0F` and `0b110 +^ 0b011` are evaluated
- **THEN** they are `0x30`, `0xff` and `0b101`

#### Scenario: Not a whole number
- **WHEN** `true +& 6` or `1.5 +| 1` is checked
- **THEN** it is an error

### Requirement: Shifts
`x +< n` and `x +> n` SHALL shift the whole number `x` by `n` bits. `+>`
SHALL fill with the sign of `x`. An `n` less than 0 or more than 63, or a
`+<` whose result does not fit 64 bits, SHALL be an error.

#### Scenario: Shifting
- **WHEN** `1 +< 4`, `0xff +> 4` and `-16 +> 2` are evaluated
- **THEN** they are `16`, `0xf` and `-4`

#### Scenario: Too far
- **WHEN** `1 +< 64` is evaluated
- **THEN** it is an error

### Requirement: Precedence
`+&`, `+<` and `+>` SHALL bind as `*` does, `+|` and `+^` as `+` does,
and the prefix `+^` as the prefix `-` does.

#### Scenario: A flag test
- **WHEN** `0b110 +& 0b100 == 0b100` is evaluated
- **THEN** it is `true`

#### Scenario: A field of a word
- **WHEN** `0x1234 +> 4 +& 0xf` is evaluated
- **THEN** it is `0x3`
