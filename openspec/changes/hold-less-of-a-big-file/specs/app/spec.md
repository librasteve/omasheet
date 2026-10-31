# app

What the grid is drawn from.

## MODIFIED Requirements

### Requirement: The snapshot carries the rows the grid shows

The snapshot the app draws from SHALL hold the rows the grid is showing
rather than every row of the sheet, and the app SHALL ask for more as it
scrolls.

A cell SHALL be shown exactly as it is shown today: the rows carried are
fewer, and nothing about one of them changes.

#### Scenario: Opening a large table formats a screenful

- **GIVEN** a `table` over a file of a million rows
- **WHEN** it is opened in the app
- **THEN** the rows the grid shows are formatted
- **AND** the rest are not

#### Scenario: Scrolling asks for what it reaches

- **GIVEN** such a table, open
- **WHEN** the grid is scrolled to a row that has not been formatted
- **THEN** that row is formatted and shown
