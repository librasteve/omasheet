# cli

`print` by a date or time column's format.

## MODIFIED Requirements

### Requirement: Print
`omasheet print` SHALL show a date or time in a column with a format by that
format.

#### Scenario: Printing a formatted date column
- **GIVEN** a column `A : Date[%d %b %Y]` and a cell `2025-01-31`
- **WHEN** the file is printed
- **THEN** the cell is printed as `31 Jan 2025`
