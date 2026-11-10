# file-format

## MODIFIED Requirements

### Requirement: Inferring a column's type
A column with no type line SHALL take the join of its cells' types. A cell
SHALL be stored as it was entered: a number as written, or the text of a
formula.

#### Scenario: A mixed column
- **WHEN** an undeclared column holds `1/1` and `1.0`
- **THEN** it is shown as a `Decimal` column and the cells are unchanged in the file

### Requirement: Undeclared column type inference
The two families of number subtypes, `Decimal` with `Fraction` and `Percent` and `Integer` with `Binary`, `Octal` and `Hex`, SHALL be inferred alike. The type of a column SHALL never be set by the values of its cells: only a type line in the `.omx` file, or the app's set-type action, sets it. The type of an undeclared column SHALL be derived entirely from the cells it holds now, as the join of their types, and SHALL be worked out as cells are entered: the first cell gives the type, and each cell after it widens it if it needs to. A cell typed into an undeclared column SHALL NOT write a type line.

#### Scenario: A first cell does not lock the column
- **GIVEN** an empty, undeclared column
- **WHEN** `0b11` is typed into a cell
- **THEN** the heading shows `Binary` as inferred and the file has no type line
- **AND** typing `255` into the next cell leaves both as typed and widens the column to `Integer`

### Requirement: Each cell's format in an undeclared column
In an undeclared column each cell SHALL be shown in the format its own contents give it, and SHALL NOT be altered by what is entered in other cells. `1`, `1.1`, `1/2`, `23%`, `1e5` and text SHALL be able to stand together in one column, each shown as it was written.

#### Scenario: Mixed bases
- **GIVEN** an undeclared column holding `1`, `255` and `7`
- **WHEN** `0b11` is typed into the first cell
- **THEN** that cell is shown `0b11` and the others as they were, and the column is an `Integer` column

### Requirement: Setting a column's type
Setting a column's type SHALL apply one common format to its cells, which is the format of that type. A value that is compatible with the type SHALL be shown in it, whichever way it was written or entered, and the file SHALL keep the text of a cell as it was entered. This is the difference from an undeclared column, where each cell has its own format.

#### Scenario: Conversion to a base
- **GIVEN** a column whose type is set to `Binary`
- **WHEN** the `Integer` `15` is entered
- **THEN** the cell is shown `0b1111`, which is a way to convert a base on the fly
