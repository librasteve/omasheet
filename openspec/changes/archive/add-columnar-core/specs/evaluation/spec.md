# evaluation

What the dataframe does, and what the engine keeps.

## ADDED Requirements

### Requirement: A sheet's data is held as columns

A sheet's values SHALL be held as one Polars column per sheet column, built
once from the authored cells or read once from the sheet's source, rather than
as a value per cell. A column whose rows come from a source SHALL arrive as a
column and SHALL NOT be converted to cells on the way in.

The authored cells SHALL remain as they are: they carry the spans the text is
edited through, and a cell may hold a formula of its own, which no column can.

#### Scenario: A table's rows are not materialised

- **GIVEN** a `table` over a Parquet file of a million rows
- **WHEN** the file is opened
- **THEN** its columns are held as the file's own columns
- **AND** no value of it has been boxed into a cell

#### Scenario: Opening a large table allocates no grid

- **GIVEN** a `table` of a million rows and no row-recursive column
- **WHEN** the engine is built
- **THEN** no memoisation slot is allocated for it

### Requirement: An elementwise column is lowered

A computed column SHALL be evaluated as one Polars expression over the whole
column when two things hold: every reference it makes reaches the same row,
which the dependency analysis already decides; and every part of its
expression has a Polars equivalent.

Whether a part has an equivalent SHALL be decided by a total function: an
expression it does not handle SHALL be declined, never approximated.

#### Scenario: An elementwise column is computed in one step

- **GIVEN** a sheet with `Profit := Revenue - Cost`
- **WHEN** it is calculated
- **THEN** the column is computed by one Polars expression
- **AND** the results equal what the engine gives row by row

#### Scenario: A row-recursive column stays on the engine

- **GIVEN** a running balance, `Balance := Amount + [Balance; *-1]`
- **WHEN** the sheet is calculated
- **THEN** the column is calculated by the engine, in row order
- **AND** its results are correct beside a lowered column in the same sheet

#### Scenario: A column the dataframe has no answer for

- **GIVEN** a column whose formula divides two exact numbers
- **WHEN** the sheet is calculated
- **THEN** the column is calculated by the engine
- **AND** the result is the exact one, to the digits `Decimal` holds

### Requirement: A lowered group succeeds wholly or not at all

Where a lowered group fails, its result SHALL be discarded whole and the group
SHALL be calculated again, row by row, by the engine. No diagnostic SHALL be
filed for the failure of the lowered attempt.

This is because a null and an error are not the same thing: a null propagates
silently and an aggregate passes over it, while an error is reported against
the cell it happened in. A dataframe has no error element, so a lowered column
cannot produce one, and only the row path can say which row failed.

#### Scenario: A column that fails for one row still names that row

- **GIVEN** a sheet whose formula fails for the third row alone
- **WHEN** it is calculated
- **THEN** the third cell shows an error and the others their values
- **AND** the diagnostic names the third row

### Requirement: Selections are pushed down

A row or column selection over a column-held sheet SHALL be given to Polars as
a filter, a projection or a limit, rather than resolved cell by cell. Only the
rows and columns a selection asks for SHALL be read.

#### Scenario: The first rows are read and no more

- **GIVEN** a table over a Parquet file of many row groups
- **WHEN** `Sales |> head(5)` is evaluated
- **THEN** the five rows are returned
- **AND** only the first row group has been read

#### Scenario: Unselected columns are not read

- **GIVEN** a table of twenty columns
- **WHEN** one column is summed
- **THEN** only that column is read from the file

### Requirement: The two paths agree, and either may be asked for

Arithmetic in the engine SHALL give the same result as the Polars kernel gives
for a one-row column of the same dtypes, except where this specification says
otherwise. Which path is used SHALL be selectable, so that the two can be
compared over any file and so that a file which miscalculates can be made to
calculate the other way.

#### Scenario: A file calculates the same either way

- **GIVEN** any file of the example corpus
- **WHEN** it is calculated with lowering refused and with lowering forced
- **THEN** the two outputs are identical
- **AND** both equal the output recorded for that file

#### Scenario: A scalar and a one-row column agree

- **GIVEN** any pair of dtypes and an operator defined over them
- **WHEN** the operation is evaluated as a scalar and as a one-row column
- **THEN** the results are equal

### Requirement: What is not lowered

The following SHALL be calculated by the engine, because Polars and omasheet
do not agree about them:

- division of exact numbers, and the mean of them, since a Polars quotient is
  fixed at a scale where `Decimal` holds up to thirty digits, and a Polars
  mean over a decimal column is a floating-point number;
- the least and the greatest of a floating-point column holding a `NaN`, since
  Polars passes over it where omasheet answers it;
- a date-time column whose values are not all in one time zone, since a Polars
  column carries its zone on its type rather than on each value;
- any value with no dtype to be held as.

A multiplication of exact numbers SHALL be lowered, and the scale of the
result SHALL be checked against the scale the engine would have given.

#### Scenario: An exact division is exact

- **GIVEN** a column `Share := Revenue / 7` over `Decimal` values
- **WHEN** it is calculated
- **THEN** the result carries the digits `Decimal` holds, not six places

#### Scenario: A mean of decimals is exact

- **GIVEN** a `Decimal` column
- **WHEN** its mean is taken
- **THEN** the result is an exact `Decimal`, not a floating-point number

#### Scenario: A column of one zone and a column of several

- **GIVEN** a date-time column moved wholly to one other zone
- **WHEN** it is calculated
- **THEN** it is lowered, the zone being held on the column's type
- **AND** a column whose values are in different zones is calculated by the
  engine

## MODIFIED Requirements

### Requirement: Display is computed by the column

Every cell's shown value SHALL be produced one column at a time rather than
one cell at a time, so that a column's values are read from the dataframe in
one pass and formatted once each.

#### Scenario: A sheet is shown without reading cell by cell

- **GIVEN** a sheet of a hundred thousand rows
- **WHEN** it is shown
- **THEN** each column is read once and its values formatted in that pass
