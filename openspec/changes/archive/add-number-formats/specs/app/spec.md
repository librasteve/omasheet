# app

The format in a column heading.

## MODIFIED Requirements

### Requirement: Column headings
A column heading SHALL show the column's type, and, where the column has a
format, the format after it in brackets, without its `%`.

#### Scenario: A formatted column's heading
- **GIVEN** a column `A : Integer['d]`
- **WHEN** the sheet is shown
- **THEN** the heading of `A` shows `Integer['d]`

### Requirement: Setting a format
The app SHALL let a format be set on, changed on and taken off an
`Integer`, `Decimal` or `Float` column, and SHALL refuse one that does not
read. The cell editor SHALL show a formatted cell's value in full.

#### Scenario: Editing a formatted cell
- **GIVEN** a column `A : Decimal[.2f]` and a cell `19.999`, shown as `20.00`
- **WHEN** the cell is edited
- **THEN** the editor shows `19.999`
