# app

Entering Hex, Octal and Binary values.

## MODIFIED Requirements

### Requirement: Cell entry
A value typed with a `0x`, `0o` or `0b` prefix SHALL be read as a `Hex`,
`Octal` or `Binary`. In a column with no declared type, such values SHALL
infer the column's type, which SHALL be saved to the file and shown in the
column heading.

#### Scenario: Typing hex into a new column
- **GIVEN** an empty, undeclared column
- **WHEN** `0xFF` is typed into a cell
- **THEN** the cell shows `0xff`, the heading shows `Hex`, and the file declares the column `: Hex`
