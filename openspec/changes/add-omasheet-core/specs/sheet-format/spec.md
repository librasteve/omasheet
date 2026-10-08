# sheet-format

The `.omx` plain-text file format.

## ADDED Requirements

### Requirement: Plain-text source file
A sheet SHALL be a UTF-8 plain-text file with the extension `.omx` that is the
complete source of truth for its data, schema and formulas. The format SHALL be
line-oriented so that adding, removing or changing one row produces a one-line
diff.

#### Scenario: Row change is a one-line diff
- **GIVEN** an `.omx` file under version control containing a table of ten rows
- **WHEN** one cell in one row is edited
- **THEN** the textual diff contains exactly one changed line

### Requirement: Table declaration
A table SHALL be introduced by a line `table <Name>`, followed by an optional
schema, a header row of column names separated by `|`, an optional separator row
of dashes, and zero or more data rows with cells separated by `|`. Leading and
trailing whitespace around a cell SHALL be insignificant, so columns may be
aligned for readability.

#### Scenario: Minimal table
- **GIVEN** the source
  ```omx
  table Sales

  Month | Revenue | Cost
  Jan   | 10000   | 6000
  Feb   | 12000   | 7000
  ```
- **WHEN** the file is parsed
- **THEN** a table `Sales` exists with columns `Month`, `Revenue`, `Cost` and two rows

#### Scenario: Separator row is optional
- **GIVEN** the same table with a line `------|---------|-----` after the header
- **WHEN** the file is parsed
- **THEN** the result is identical to the table without the separator row

#### Scenario: Ragged row
- **WHEN** a data row has a different number of cells than the header
- **THEN** parsing reports an error naming the table and the line number

### Requirement: Multiple tables per file
A sheet SHALL be able to contain any number of tables. Table names SHALL be
unique within a sheet, and any table SHALL be referable by name from expressions
in any other table of the same sheet.

#### Scenario: Cross-table reference
- **GIVEN** a sheet with tables `Sales` and `Summary`
- **WHEN** a cell in `Summary` contains `= Sales.Revenue.sum()`
- **THEN** it evaluates to the sum of the `Revenue` column of `Sales`

#### Scenario: Duplicate table name
- **WHEN** two tables in one sheet are both named `Sales`
- **THEN** parsing reports an error at the second declaration

### Requirement: Column schema
A table SHALL be able to declare the type of each column with lines of the form
`<Column> : <Type>` placed between the `table` line and the header row. A type
SHALL be a base type (`Int`, `Rat`, `Num`, `Text`, `Date`, `Bool`). A column
with no declaration SHALL have its type inferred from its cells.

#### Scenario: Typed columns
- **GIVEN** the source
  ```omx
  table Items

  Qty   : Int
  Price : Rat

  Qty | Price
  2   | 19.99
  5   | 0.50
  ```
- **WHEN** the file is parsed
- **THEN** `Items.Qty` has type `Int`, `Items.Price` has type `Rat`, and the first row's `Price` is exactly `1999/100`

#### Scenario: Schema names a column not in the header
- **WHEN** a schema line declares a column that does not appear in the header and is not a computed column
- **THEN** parsing reports an error

### Requirement: Computed columns
A table SHALL be able to define a column once for all rows with
`<Column> := <OMX expression>`. The expression SHALL be evaluated once per row
with that row as the cursor. A computed column SHALL NOT also appear with data in
the header row.

#### Scenario: Profit column
- **GIVEN** a table `Sales` with data columns `Revenue` and `Cost` and the line `Profit := Revenue - Cost`
- **WHEN** the sheet is evaluated
- **THEN** every row has a `Profit` equal to that row's `Revenue` minus that row's `Cost`

#### Scenario: Computed column redeclared as data
- **WHEN** `Profit := Revenue - Cost` is declared and `Profit` also appears in the header row
- **THEN** parsing reports an error

### Requirement: Cell content
A cell SHALL be either empty, a literal, or a formula. A cell whose first
non-whitespace character is `=` SHALL be a formula: the rest of the cell SHALL be
parsed as an OMX expression and evaluated with its row as the cursor. Every other
non-empty cell SHALL be a literal and SHALL NOT be evaluated as an expression. A
literal cell in a column declared `Text` SHALL be read as literal text. A literal
cell in a column of any other declared type SHALL be a literal of that type, and
SHALL be an error otherwise. In an undeclared column, a cell that is a valid OMX
literal SHALL take that literal's type and any other literal cell SHALL be text.
Text that itself begins with `=` SHALL be written as a quoted string.

#### Scenario: Text cell
- **GIVEN** an undeclared column `Month`
- **WHEN** a cell contains `Jan`
- **THEN** its value is the text `Jan`

#### Scenario: Per-cell formula in a typed column
- **GIVEN** a column declared `Tax : Rat` and a constant `TaxRate`
- **WHEN** a cell in `Tax` contains `= Revenue * TaxRate`
- **THEN** its value is that row's `Revenue` multiplied by `TaxRate`

#### Scenario: Unmarked expression in a typed column
- **GIVEN** a column declared `Tax : Rat`
- **WHEN** a cell in `Tax` contains `Revenue * TaxRate` with no leading `=`
- **THEN** an error reports that the cell is not a `Rat` literal

#### Scenario: Unmarked expression in a text column
- **GIVEN** a column declared `Note : Text`
- **WHEN** a cell in `Note` contains `Revenue - Cost`
- **THEN** its value is the text `Revenue - Cost`

#### Scenario: Text beginning with an equals sign
- **GIVEN** a column declared `Note : Text`
- **WHEN** a cell in `Note` contains `"= see appendix"`
- **THEN** its value is the text `= see appendix`

#### Scenario: Empty cell
- **WHEN** a cell contains only whitespace
- **THEN** its value is empty (undefined)

### Requirement: Constants
A sheet SHALL be able to declare named constants with `const <Name> = <OMX
expression>` outside any table. A constant SHALL be referable by bare name from
any expression in the sheet and SHALL NOT depend on any row cursor.

#### Scenario: Global tax rate
- **GIVEN** `const TaxRate = 20%`
- **WHEN** a computed column is defined as `Tax := Revenue * TaxRate`
- **THEN** every row uses the same `TaxRate` value of exactly `1/5`

### Requirement: No grid coordinates in authored files
The `.omx` format SHALL NOT define A1-style or `$A$1`-style cell references.
All references SHALL be by table name, column name, row position, cursor offset
or predicate.

#### Scenario: A1 reference is not special
- **WHEN** an expression contains `B2` and no constant, table or column named `B2` exists
- **THEN** an unresolved-name error is reported
