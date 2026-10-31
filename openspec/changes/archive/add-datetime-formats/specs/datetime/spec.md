# datetime

Showing a date or time by its column's format.

## ADDED Requirements

### Requirement: Formatted dates and times
A value in a `Date`, `Time` or `Datetime` column with a format SHALL be
shown as C `strftime` shows it with that format, in place of the locale's
form. The value itself SHALL be unchanged.

#### Scenario: A date in words
- **GIVEN** a column `A : Date[%a %d %b %Y]` and a cell `2025-01-31`
- **WHEN** the cell is shown
- **THEN** it is shown as `Fri 31 Jan 2025`

#### Scenario: A twelve-hour time
- **GIVEN** a column `A : Time[%I:%M %p]` and a cell `17:05`
- **WHEN** the cell is shown
- **THEN** it is shown as `05:05 PM`

#### Scenario: The value in full in the file
- **GIVEN** a column `A : Datetime[%d/%m %H:%M]` and a cell `2025-01-31T09:30:15`
- **WHEN** the cell is shown and the file is written
- **THEN** it is shown as `31/01 09:30` and written as `2025-01-31T09:30:15`
