# file-format

Hex, Octal and Binary on a schema line.

## MODIFIED Requirements

### Requirement: Column types
A schema line SHALL accept `Hex`, `Octal` and `Binary` as column types, as
well as the existing ones. When a column's type is inferred from its cells,
cells that all have the same prefix SHALL infer that subtype, and the subtype
SHALL be written into the schema line.

#### Scenario: Inferred and written
- **GIVEN** an undeclared column whose cells are `0x10` and `0xff`
- **WHEN** the file is written
- **THEN** the column is declared `: Hex` and its cells are written `0x10` and `0xff`
