# numerics

What a bitwise operation gives.

## ADDED Requirements

### Requirement: The base of a bitwise result
`+&`, `+|` and `+^` of whole numbers that share a base SHALL give that
base, and of any other mix SHALL give an `Integer`. The prefix `+^` SHALL
keep the base of its operand, and `+<` and `+>` the base of the number on
their left.

#### Scenario: Bases
- **WHEN** `0xF0 +& 0x3C` and `0xF0 +& 60` are evaluated
- **THEN** the first is the `Hex` 48 and the second the `Integer` 48
- **AND** `0xff +> 4` is the `Hex` 15

### Requirement: Complement
`+^n` of a whole number SHALL be `-n - 1`, and SHALL be shown as a
negative of its base is shown.

#### Scenario: Complement of a hex
- **WHEN** `+^0xff` is displayed
- **THEN** it is shown as `0xffffffffffffff00`
