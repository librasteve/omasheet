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

### Requirement: Sheet time zone
A sheet SHALL be able to name the time zone its `DateTime` values are in with a
line `zone <Area/City>`, using a name from the time zone database, placed before
its first table. A sheet with no such line SHALL be in the zone of the machine
that evaluates it. A second `zone` line, or a zone that does not exist, SHALL be
an error.

#### Scenario: A sheet in New York
- **GIVEN** a sheet that begins `zone America/New_York` and holds `2025-03-03T09:12`
- **WHEN** it is evaluated on a machine in London
- **THEN** that cell is 09:12 in New York, which is 14:12 in London

#### Scenario: Unknown zone
- **GIVEN** a sheet that begins `zone Mars/Base`
- **WHEN** it is linted
- **THEN** an error at that line says there is no time zone `Mars/Base`

### Requirement: Column schema
A table SHALL be able to declare the type of each column with lines of the form
`<Column> : <Type>` placed between the `table` line and the header row. A type
SHALL be a base type (`Int`, `Ratio`, `Num`, `Complex`, `Text`, `Date`, `Time`, `DateTime`, `Bool`).
A column with no declaration SHALL have its type inferred from its cells.

#### Scenario: Typed columns
- **GIVEN** the source
  ```omx
  table Items

  Qty   : Int
  Price : Ratio

  Qty | Price
  2   | 19.99
  5   | 0.50
  ```
- **WHEN** the file is parsed
- **THEN** `Items.Qty` has type `Int`, `Items.Price` has type `Ratio`, and the first row's `Price` is exactly `1999/100`

#### Scenario: Schema names a column not in the header
- **WHEN** a schema line declares a column that does not appear in the header and is not a computed column
- **THEN** parsing reports an error

### Requirement: Computed columns
A table SHALL be able to define a column once for all rows with
`<Column> := <OMX expression>`. The expression SHALL be evaluated once per row
with that row as the cursor. The header row SHALL name every computed column,
which gives its place among the other columns, and each of its cells SHALL be
written `*`. A definition with no column of its name in the header row SHALL be
an error, and so SHALL anything but `*` in a cell of a computed column.

#### Scenario: Profit column
- **GIVEN** a table `Sales` with data columns `Revenue` and `Cost`, a column `Profit` whose cells are `*`, and the line `Profit := Revenue - Cost`
- **WHEN** the sheet is evaluated
- **THEN** every row has a `Profit` equal to that row's `Revenue` minus that row's `Cost`

#### Scenario: A computed column among data columns
- **GIVEN** the header row `Month | Revenue | Profit | Cost | Tax`, rows with `*` under `Profit` and `Tax`, and the lines `Profit := Revenue - Cost` and `Tax := Profit * TaxRate`
- **WHEN** the sheet is evaluated
- **THEN** the columns are `Month`, `Revenue`, `Profit`, `Cost`, `Tax` in that order, and `Sales[2; 0]` is the first row's `Profit`

#### Scenario: Computed column not in the header row
- **WHEN** `Profit := Revenue - Cost` is declared and the header row has no column `Profit`
- **THEN** an error reports that the formula column is not in the header row

#### Scenario: Computed column redeclared as data
- **WHEN** `Profit := Revenue - Cost` is declared, `Profit` also appears in the header row, and a cell under it is not `*`
- **THEN** an error reports that the cells of a formula column are written `*`

### Requirement: Cell content
A cell SHALL be either empty, a literal, or a formula. A cell whose first
non-whitespace character is `=` SHALL be a formula: the rest of the cell SHALL be
parsed as an OMX expression and evaluated with its row as the cursor. Every other
non-empty cell SHALL be a literal and SHALL NOT be evaluated as an expression. A
literal cell in a column declared `Text` SHALL be read as literal text. A literal
cell in a column of any other declared type SHALL be a literal of that type, and
SHALL be an error otherwise. In an undeclared column, a cell that is a valid OMX
literal SHALL take that literal's type and any other literal cell SHALL be text.
A cell holding two whole numbers separated by `/`, such as `1/7`, optionally
negated and with a non-zero denominator, SHALL be a `Ratio` literal.
Text that itself begins with `=` SHALL be written as a quoted string.

#### Scenario: Text cell
- **GIVEN** an undeclared column `Month`
- **WHEN** a cell contains `Jan`
- **THEN** its value is the text `Jan`

#### Scenario: Per-cell formula in a typed column
- **GIVEN** a column declared `Tax : Ratio` and a constant `TaxRate`
- **WHEN** a cell in `Tax` contains `= Revenue * TaxRate`
- **THEN** its value is that row's `Revenue` multiplied by `TaxRate`

#### Scenario: Fraction cell
- **GIVEN** an undeclared column `Share`
- **WHEN** a cell contains `1/7`
- **THEN** its value is the exact `Ratio` one seventh

#### Scenario: Unmarked expression in a typed column
- **GIVEN** a column declared `Tax : Ratio`
- **WHEN** a cell in `Tax` contains `Revenue * TaxRate` with no leading `=`
- **THEN** an error reports that the cell is not a `Ratio` literal

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

### Requirement: Functions
A sheet SHALL be able to define a function with `fn <Name>(<parameters>) =
<OMX expression>` outside any table, with zero or more comma-separated
parameter names. The expression SHALL be able to refer to its parameters, the
sheet's constants and tables, and the sheet's other functions, and SHALL NOT see
the row it is called from. A parameter SHALL take a value of any shape, a table
included, and a parameter's name SHALL come before a column, constant or table
of the same name. A function SHALL NOT take the name of a built-in function or
of another function of the sheet, and SHALL NOT be defined in terms of itself,
directly or through other functions. The comment lines directly above a
definition SHALL be kept as its description.

#### Scenario: A function in a computed column
- **GIVEN** `fn Margin(revenue, cost) = (revenue - cost) / revenue`
- **WHEN** a computed column is defined as `Margin := Margin(Revenue, Cost)`
- **THEN** each row holds that row's exact margin

#### Scenario: A function does not see the caller's row
- **GIVEN** `fn Bad(x) = x + Cost` and a table with a column `Cost`
- **THEN** `lint` reports that `Cost` is an unknown name

#### Scenario: A function that calls itself
- **GIVEN** `fn F(x) = F(x) + 1`
- **THEN** `lint` reports that `F` calls itself

### Requirement: No grid coordinates in authored files
The `.omx` format SHALL NOT define A1-style or `$A$1`-style cell references.
All references SHALL be by table name, column name, row position, cursor offset
or predicate.

#### Scenario: A1 reference is not special
- **WHEN** an expression contains `B2` and no constant, table or column named `B2` exists
- **THEN** an unresolved-name error is reported
