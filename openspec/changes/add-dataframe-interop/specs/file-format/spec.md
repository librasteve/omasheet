# file-format

The `table` statement: a sheet whose rows live outside the file.

## ADDED Requirements

### Requirement: A dataframe-backed table

A file MAY declare a sheet whose rows live in Parquet files rather than in its
own text:

```omx
table Sales := dataframe["data/*.parquet"]
```

The statement SHALL take the same place as `sheet` and SHALL be read the same
way: it closes the sheet before it, and names a sheet that may then carry
schema lines and computed columns. The string SHALL be a path, and MAY be a
glob matching several files, which are read as one frame.

The sheet SHALL otherwise behave as any other: its columns may be selected and
indexed, computed columns may be defined over them, and the dependency graph
and calculation order SHALL be unchanged (D27, D28).

#### Scenario: A table is declared and read

- **GIVEN** a file holding `table Sales := dataframe["data/*.parquet"]`
- **WHEN** the file is checked
- **THEN** there is a sheet `Sales` with a column for each column of the files
- **AND** each column's type comes from the Parquet schema

#### Scenario: A computed column over a table

- **GIVEN** a dataframe-backed table with `Revenue` and `Cost` columns
- **WHEN** `Profit := Revenue - Cost` is added
- **THEN** `Profit` calculates for every row
- **AND** it is a column of the table like any other

#### Scenario: A missing source is reported

- **GIVEN** a table whose glob matches no file
- **WHEN** the file is checked
- **THEN** a diagnostic says no file matched, naming the pattern
- **AND** it points at the string in the statement

### Requirement: The schema is kept in the text and checked

The column names and types of a dataframe-backed table SHALL be written into
the `.omx` text as the table's schema lines, so the file says what it expects
without reading the data.

On load, that schema SHALL be checked against the files' footers. A mismatch
SHALL be a diagnostic naming the column, and SHALL NOT be silently resolved in
favour of either side (D29).

#### Scenario: The written schema matches the files

- **GIVEN** a table whose schema lines match its Parquet files
- **WHEN** the file is loaded
- **THEN** no diagnostic is reported
- **AND** the data is not read to establish the types

#### Scenario: A changed source is reported

- **GIVEN** a table whose files have gained a column since the schema was written
- **WHEN** the file is loaded
- **THEN** a diagnostic names the column that the schema does not mention

### Requirement: The row count is read from the footer

The number of rows SHALL be taken from the Parquet metadata, without reading
the data, because it is fixed before evaluation begins.

#### Scenario: A large table opens without reading its rows

- **GIVEN** a table over a Parquet file of a million rows
- **WHEN** the file is opened
- **THEN** the sheet reports a million rows
- **AND** no row group has been read
