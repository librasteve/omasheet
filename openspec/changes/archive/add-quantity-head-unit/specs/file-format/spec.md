# file-format

## MODIFIED Requirements

### Requirement: A quantity column says its unit in its head
A `Quantity` column that has a unit, named in its format or given by its kind, SHALL show each cell as a number and its head as the type with the unit in brackets, in full: `Mass[kg]`. A column with no unit SHALL show each value with its own.

#### Scenario: A kind
- **WHEN** a `Weight : Mass` column holds `5kg` and `300g`
- **THEN** its head is `Mass[kg]` and its cells are `5` and `0.3`

#### Scenario: A unit in the format
- **WHEN** the column is `Mass[g]`
- **THEN** its head is `Mass[g]` and its cells are `5000` and `300`

#### Scenario: A value that does not fit
- **WHEN** a formula column holds a value that cannot be put in the column's unit
- **THEN** it is shown with its own unit

## ADDED Requirements

### Requirement: The unit in each row
The flag `v` SHALL be accepted first in a `Quantity` number format, after any `±` or `~`, and SHALL show the unit after each number as it is written. It SHALL be an error where the column has no unit and its kind names none. With a unit, `_` SHALL be an error without `v`.

#### Scenario: Down in the rows
- **WHEN** `5kg` is shown under `Mass[g; v.1f]`
- **THEN** it is `5000.0g` and the head is still `Mass[g]`

#### Scenario: Nowhere to put it
- **WHEN** a column is declared `Quantity[; v.9s]`
- **THEN** it is an error that `v` needs a unit

#### Scenario: A gap needs the rows
- **WHEN** a column is declared `Mass[g; _.1f]`
- **THEN** it is an error, and `Mass[g; v_.1f]` shows `5000.0 g`

### Requirement: A table names the unit after the column
A table printed or written to Markdown or HTML SHALL name a column's unit after its name, `Weight [kg]`, where the column's cells are numbers. A value shown in a sentence SHALL keep its unit.

#### Scenario: A printed table
- **WHEN** a `Mass` column named Weight is printed
- **THEN** its head is `Weight [kg]`
