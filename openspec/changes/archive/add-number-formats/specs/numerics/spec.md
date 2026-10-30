# numerics

Showing a number by its column's format.

## ADDED Requirements

### Requirement: Formatted numbers
A number in a column with a format SHALL be shown as C `printf` shows it
with that conversion, in place of the adaptive display rules: no limit of
width, no e notation unless the conversion asks for it, and no `…`. Its
point SHALL be the locale's, and the `'` flag SHALL group its whole part in
threes by the locale's separator. The value itself SHALL be unchanged.

#### Scenario: Grouping and places
- **GIVEN** a column `A : Decimal['.2f]`, a cell `12345.678`, and a locale that writes `12,345.6`
- **WHEN** the cell is shown
- **THEN** it is shown as `12,345.68`

#### Scenario: The locale's point
- **GIVEN** a column `A : Float[.2f]`, a cell `0.5`, and a locale that writes `0,5`
- **WHEN** the cell is shown
- **THEN** it is shown as `0,50`

#### Scenario: The value in full in the file
- **GIVEN** a column `A : Decimal[.2f]` and a cell `19.999`
- **WHEN** the cell is shown and the file is written
- **THEN** it is shown as `20.00` and written as `19.999`

#### Scenario: Hex and binary
- **GIVEN** a column `A : Integer[#x]` and cells `255` and `-255`, and a column `B : Integer[#b]` and a cell `5`
- **WHEN** the cells are shown
- **THEN** `A` shows `0xff` and `0xffffffffffffff01`, and `B` shows `0b101`
