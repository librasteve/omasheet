# xlsx-interop

## ADDED Requirements

### Requirement: Quantities on export
A quantity SHALL be exported as text such as `5kg`, with a note that the unit is not a number format.

#### Scenario: Export
- **WHEN** a sheet with the cell `5kg` is exported
- **THEN** the cell holds the text `5kg` and the note is reported
