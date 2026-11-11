# file-format

## MODIFIED Requirements

### Requirement: Column types
A column type MAY be `Quantity` or the name of a quantity dimension (`Length`, `Mass`, `Force`, ...). Its format bracket MAY be a unit of that dimension: `Weight : Mass[g]` shows and reads the column in grams.

#### Scenario: Display unit
- **GIVEN** a column `Weight : Mass[g]` and a cell `5kg`
- **THEN** the cell is shown as `5000g`
