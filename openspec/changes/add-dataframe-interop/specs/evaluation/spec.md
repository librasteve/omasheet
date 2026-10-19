# evaluation

What Polars does, and what the engine keeps.

## ADDED Requirements

### Requirement: Selections are pushed down

A row or column selection over a dataframe-backed table SHALL be given to
Polars as a limit, a filter or a projection, rather than resolved cell by cell.
Only the rows and columns a selection asks for SHALL be read (D31).

#### Scenario: The first rows are read and no more

- **GIVEN** a table over a Parquet file of many row groups
- **WHEN** `Sales.head(5)` is evaluated
- **THEN** the five rows are returned
- **AND** only the first row group has been read

#### Scenario: Unselected columns are not read

- **GIVEN** a table of twenty columns
- **WHEN** one column is summed
- **THEN** only that column is read from the file

### Requirement: Elementwise columns are lowered to Polars

A computed column whose expression is elementwise over columns of the same
sheet SHALL be evaluated as a Polars expression over the whole column, rather
than row by row through the engine.

Whether a column qualifies SHALL be decided from the dependency analysis the
checker already performs: a column qualifies when every reference it makes
reaches the same row (D30).

#### Scenario: An elementwise column is computed as one expression

- **GIVEN** a dataframe-backed table with `Profit := Revenue - Cost`
- **WHEN** the sheet is calculated
- **THEN** the column is computed by one Polars expression
- **AND** the results equal what the engine gives row by row

#### Scenario: A row-recursive column stays on the engine

- **GIVEN** a running balance, `Balance := Amount + [Balance; *-1]`
- **WHEN** the sheet is calculated
- **THEN** the column is calculated by the engine, in row order
- **AND** its results are correct beside a lowered column in the same sheet

### Requirement: The two arithmetic paths agree

Arithmetic on a single value, in the engine, SHALL give the same result as the
Polars kernel gives for a one-row series of the same dtypes. Where they cannot
agree, the engine's result SHALL be the specified one and the difference SHALL
be reported.

#### Scenario: A scalar and a one-row column agree

- **GIVEN** any pair of dtypes and an operator defined over them
- **WHEN** the operation is evaluated as a scalar and as a one-row column
- **THEN** the two results are equal
