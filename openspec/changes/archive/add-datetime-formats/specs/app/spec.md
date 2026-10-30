# app

The format of a date or time column.

## MODIFIED Requirements

### Requirement: Column headings
A column heading SHALL show the column's type, and, where the column has a
format, the format after it in brackets.

#### Scenario: A formatted date column's heading
- **GIVEN** a column `A : Date[%d %b %Y]`
- **WHEN** the sheet is shown
- **THEN** the heading of `A` shows `Date[%d %b %Y]`

### Requirement: Setting a format
The app SHALL let a format be set on, changed on and taken off a `Date`,
`Time` or `Datetime` column, and SHALL refuse one that does not read.
