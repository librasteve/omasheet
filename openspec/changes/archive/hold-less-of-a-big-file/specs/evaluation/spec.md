# evaluation

What is read of a source, and what a whole column is held as.

## MODIFIED Requirements

### Requirement: Only the row groups an answer needs are read

A selection that takes a row range from a sheet whose rows come from a
source SHALL carry that range into the plan that reads the source, so that
only the row groups holding those rows are read.

The projection already works this way: a column nothing asks for is never
read. The row range SHALL work the same way, and a column read for a range
SHALL answer no differently from the same column read whole.

#### Scenario: The first rows come from the first row group

- **GIVEN** a `table` over a Parquet file of five row groups of a thousand
  rows
- **WHEN** the first five rows of one of its columns are asked for
- **THEN** one row group is read
- **AND** the five values are those the whole column would have given

#### Scenario: A sum still reads every row group

- **GIVEN** the same table
- **WHEN** a sum over one of its columns is asked for
- **THEN** every row group of that column is read, the sum needing all of
  them
- **AND** no row group of any other column is read

## ADDED Requirements

### Requirement: A whole column may be held as a column

A value that stands for a whole column MAY be the column itself rather than
a value per row. Where something that cannot read a column asks for one, it
SHALL be turned into a vector of values at that point, and the answer SHALL
be the same either way.

#### Scenario: Counting a column does not read its values

- **GIVEN** a `table` whose rows come from a file
- **WHEN** the rows kept by a condition over one column are counted
- **THEN** the count is answered without a value being made for each row of
  the column counted
