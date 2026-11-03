# file-format

## MODIFIED Requirements

### Requirement: Inferring a column's type
A column with no type line SHALL take the join of its cells' types. A cell
SHALL be stored as it was entered: a number as written, or the text of a
formula.

#### Scenario: A mixed column
- **WHEN** an undeclared column holds `1/1` and `1.0`
- **THEN** it is shown as a `Decimal` column and the cells are unchanged in the file
