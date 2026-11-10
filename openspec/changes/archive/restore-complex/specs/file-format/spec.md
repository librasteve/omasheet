# file-format

## MODIFIED Requirements

### Requirement: The declarable type names

`Complex` SHALL be a declarable type name, beside the Polars ones. A real number in a column declared `Complex` SHALL be that number with no imaginary part. In an undeclared column `Complex` SHALL be the join (D191) of a cell like `3+4i` with any number.

### Requirement: A cell in a declared column

A cell in a column with a declared type SHALL be accepted when its conversion to that type succeeds, and SHALL then be shown in the format of that type, the file keeping the text as it was entered. Otherwise it SHALL be an error naming the conversion that failed. A `Complex` cell with an imaginary part of zero SHALL therefore be accepted in a `Float` column, and one without SHALL NOT.

#### Scenario: A base is converted by the column

- **GIVEN** a column declared `Integer` holding `0b1` and `0x1F`
- **WHEN** the file is read
- **THEN** the cells are shown `1` and `31`
- **AND** the file still holds `0b1` and `0x1F`

#### Scenario: A Complex in a Float column

- **GIVEN** a column declared `Float` holding `3+0i` and `3+4i`
- **WHEN** the file is read
- **THEN** the first is shown `3`
- **AND** the second is an error saying its imaginary part is not zero

#### Scenario: A real number in a Complex column

- **GIVEN** a column declared `Complex` holding `3+4i` and `2.5`
- **WHEN** the file is read
- **THEN** the cells are `3+4i` and `2.5+0i`

#### Scenario: An undeclared column is a join

- **GIVEN** an undeclared column holding `3+4i` and `2.5`
- **WHEN** the file is read
- **THEN** its heading says `Complex`
