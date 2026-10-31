# cli

`print` by a column's format.

## MODIFIED Requirements

### Requirement: Print
`omasheet print` SHALL show a number in a column with a format by that
format, with the locale's point, and grouped by the locale's separator
where the format has the `'` flag.

#### Scenario: Printing a formatted column
- **GIVEN** a column `A : Decimal['.2f]`, a cell `12345.678`, and a locale that writes `12.345,6`
- **WHEN** the file is printed
- **THEN** the cell is printed as `12.345,68`
