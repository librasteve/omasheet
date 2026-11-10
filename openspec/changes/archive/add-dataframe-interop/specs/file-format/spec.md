# file-format

The `table` statement: a sheet whose rows live outside the file.

## ADDED Requirements

### Requirement: A dataframe-backed table

A file MAY declare a sheet whose rows live in a Parquet file rather than in
its own text:

```omx
table Sales := dataframe["data/sales.parquet"]
```

The statement SHALL take the same place as `sheet` and SHALL be read the same
way: it closes the sheet before it, and names a sheet that may then carry
schema lines and computed columns. The string SHALL be the path of one
Parquet file, read from beside the `.omx` file.

The sheet SHALL otherwise behave as any other: its columns may be selected and
indexed, computed columns may be defined over them, and the dependency graph
and calculation order SHALL be unchanged (D27, D28).

#### Scenario: A table is declared and read

- **GIVEN** a file holding `table Sales := dataframe["data/sales.parquet"]`
- **WHEN** the file is checked
- **THEN** there is a sheet `Sales` with a column for each column of the file
- **AND** each column's type comes from the Parquet schema

#### Scenario: A computed column over a table

- **GIVEN** a dataframe-backed table with `Revenue` and `Cost` columns
- **WHEN** `Profit := Revenue - Cost` is added
- **THEN** `Profit` calculates for every row
- **AND** it is a column of the table like any other

#### Scenario: A missing source is reported

- **GIVEN** a table whose file is not there
- **WHEN** the file is checked
- **THEN** a diagnostic says there is no such file, naming it
- **AND** it points at the string in the statement

### Requirement: A column the file names otherwise

A Parquet column name is any string; an omasheet name is an identifier. A
schema line of a table MAY therefore carry a `from` clause, naming the
column as the file holds it:

```omx
SiblingsSpousesAboard : Int from "Siblings/Spouses Aboard"
```

The name on the left SHALL be the one formulas use. The quoted name SHALL be
the one the column is found and checked by, and nothing in the file SHALL be
renamed (D50). A `from` on a sheet that holds its own rows SHALL be a
diagnostic.

#### Scenario: A column named as the file names it

- **GIVEN** a Parquet file with a column `Siblings/Spouses Aboard`
- **WHEN** a table declares
  `SiblingsSpousesAboard : Int from "Siblings/Spouses Aboard"`
- **THEN** the column is read as `SiblingsSpousesAboard`
- **AND** a formula over `SiblingsSpousesAboard` calculates for every row

#### Scenario: A column no schema line names

- **GIVEN** a Parquet file holding a column `Units/Boxes Sold` that the
  schema does not name
- **WHEN** the file is checked
- **THEN** a diagnostic names the column
- **AND** the help shows the `from` form, with a name formed from the
  file's own: `UnitsBoxesSold`

#### Scenario: A column of nulls no schema line names

- **GIVEN** a Parquet file holding a column of nothing but nulls that the
  schema does not name
- **WHEN** the file is checked
- **THEN** a diagnostic names the column
- **AND** the help says to add it without naming a type, since the file holds
  none and a schema line cannot be written without one

#### Scenario: A `from` where no file is read

- **GIVEN** a sheet that holds its own rows, with a `from` on a schema line
- **WHEN** the file is checked
- **THEN** a diagnostic says the sheet reads no file

### Requirement: The schema is kept in the text and checked

The column names and types of a dataframe-backed table SHALL be written into
the `.omx` text as the table's schema lines, so the file says what it expects
without reading the data.

On load, that schema SHALL be checked against the files' footers. A mismatch
SHALL be a diagnostic naming the column, and SHALL NOT be silently resolved in
favour of either side (D29).

A mismatch is a declaration the file's column does not fit, not one that
differs from it. A type the column's values can be held as SHALL stand: every
`Int` is a `Decimal`, and a `Fraction` and a `Percent` hold the same numbers a
`Decimal` does, so an exact column may be declared as any of them. A narrower
declaration than the file holds SHALL remain a mismatch.

A column the file holds as nulls alone has no type of its own: Parquet names
such a column `null`, and it contradicts no declaration. It SHALL be read as
the type the text declares, and where the text declares nothing it SHALL be a
column of empty cells.

#### Scenario: The written schema matches the files

- **GIVEN** a table whose schema lines match its Parquet files
- **WHEN** the file is loaded
- **THEN** no diagnostic is reported
- **AND** the data is not read to establish the types

#### Scenario: A changed source is reported

- **GIVEN** a table whose files have gained a column since the schema was written
- **WHEN** the file is loaded
- **THEN** a diagnostic names the column that the schema does not mention

#### Scenario: A declaration the file's type fits

- **GIVEN** a table whose Parquet file holds `Revenue` as an integer
- **WHEN** the schema declares `Revenue : Decimal`
- **THEN** no diagnostic is reported
- **AND** the column is read as a `Decimal`

#### Scenario: A declaration narrower than the file is reported

- **GIVEN** a table whose Parquet file holds `Price` as a decimal
- **WHEN** the schema declares `Price : Int`
- **THEN** a diagnostic names the column, the type declared and the type held

#### Scenario: A column of nulls fits the declared type

- **GIVEN** a table whose Parquet file holds a column of nothing but nulls
- **WHEN** the schema declares a type for it
- **THEN** no diagnostic is reported
- **AND** the column is read as the type declared, its cells empty

### Requirement: The row count is read from the footer

The number of rows SHALL be taken from the Parquet metadata, without reading
the data, because it is fixed before evaluation begins.

#### Scenario: A large table opens without reading its rows

- **GIVEN** a table over a Parquet file of a million rows
- **WHEN** the file is opened
- **THEN** the sheet reports a million rows
- **AND** no row group has been read

### Requirement: A sheet of formulas alone

A sheet written as `:=` lines with no table of its own SHALL hold no rows in
the text: its columns SHALL be those lines, in the order they are written,
and no header row or `*` cell SHALL be required or allowed.

```omx
sheet PerHead

Fare      := Passengers[Fare; *]
GroupSize := Passengers[Family; *] + 1
```

Its row count SHALL be the greatest row count of the sheets whose rows its
formulas pick with the cursor, which for a table is known only once the
source has been read. A sheet whose formulas pick no row of another sheet
SHALL be one row long. A row past the end of a shorter sheet it reads SHALL
be empty, as any row outside a sheet is.

An edit that needs a cell or a row of such a sheet's own SHALL be refused,
naming the sheet: there is no text behind a cell, and no row to add or
delete. Its `:=` lines SHALL be edited as any sheet's are, since they are
its text: the expression, the name of a column, and a column added after
the others.

#### Scenario: A derived sheet over a table

- **GIVEN** a table of 891 rows and a sheet of `:=` lines alone whose
  formulas read it with `*`
- **WHEN** the file is calculated
- **THEN** the sheet has 891 rows
- **AND** no row of `*` was written in the text

#### Scenario: A sheet of aggregates is one row

- **GIVEN** a sheet of `:=` lines alone whose formulas are aggregates
- **WHEN** the file is calculated
- **THEN** the sheet has one row

#### Scenario: Such a sheet takes no cell or row

- **GIVEN** a sheet of `:=` lines alone
- **WHEN** a literal cell of it is written, or a row added or deleted, or a
  data column added or deleted
- **THEN** the edit is refused, naming the sheet
- **AND** the text of the file is unchanged

#### Scenario: Its formulas are edited as any sheet's

- **GIVEN** a sheet of `:=` lines alone
- **WHEN** a cell of one of its columns is typed into
- **THEN** that column's formula is changed, as it is for a computed column
  of any sheet
- **AND** the column may be renamed, and another `:=` column added
