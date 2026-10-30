# file-format

`+|` in a table cell.

## MODIFIED Requirements

### Requirement: Table cells
A `|` SHALL divide a table's cells except inside a string, where it is
followed by `>`, and where it follows `+` in a cell that starts with `=`.

#### Scenario: A bitwise or in a cell
- **GIVEN** the row `| a | = 0x10 +| 0x01 |`
- **WHEN** the file is read
- **THEN** the row has two cells, the second the formula `= 0x10 +| 0x01`
