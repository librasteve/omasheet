# parquet-interop

Parquet in both directions: one sheet of typed columns.

Since `add-polars-dtypes`, an omasheet type *is* a Polars dtype, so the mapping
below is a naming rather than a conversion. The earlier version of this spec
converted `Int` and `Fraction` to the nearest Parquet type and accepted the
loss; that paragraph is gone with those types.

## ADDED Requirements

### Requirement: The type mapping

A column SHALL be written as the Parquet type its dtype names, and read as the
omasheet type that names the Parquet type it finds.

| Omasheet type | Parquet / Arrow |
|---|---|
| `Int` | `int64`, or the narrower width the schema had |
| `Decimal`, `Percent` | `decimal(38, s)` |
| `Float` | `double`, or `float` where the schema had it |
| `Boolean` | `boolean` |
| `String` | `byte_array`, UTF-8 |
| `Binary` | `byte_array` |
| `Date` | `date32` |
| `Time` | `time64`, nanoseconds |
| `DateTime` | `timestamp`, microseconds, with the file's zone where it has one |
| `Duration` | `duration`, microseconds |
| `Null` | a null in the column's own type |

The `Binary`, `Duration` and zoned `DateTime` rows, and keeping a column's
width and signedness, come with those types in `add-polars-dtypes`. Until
then a `Binary` or `Duration` column is left out on import with a notice, a
zone is not kept, and an integer column is written as `int64`.

An unsigned column SHALL be read as `Int` and written back unsigned. A
`Percent` SHALL be written as the `decimal` it holds, its display being no part
of the value.

#### Scenario: A decimal column round-trips exactly

- **GIVEN** a `Decimal` column holding `19.99` and `5.5`
- **WHEN** the sheet is exported and imported again
- **THEN** both values are unchanged
- **AND** the column is a `Decimal` again

#### Scenario: A date column round-trips as a date

- **GIVEN** a `Date` column
- **WHEN** the sheet is exported
- **THEN** the column is `date32`
- **AND** importing it gives a `Date` column, not a number

#### Scenario: An unsigned column keeps its signedness

- **GIVEN** a Parquet file with a `uint32` column
- **WHEN** it is imported and exported again
- **THEN** the column is `uint32` again
- **AND** it was reported as an `Int` in between

### Requirement: Types come from the schema

On import, each column's type SHALL be taken from the file's schema and SHALL
NOT be inferred from the values. The `.omx` source produced SHALL carry a
`Name : Type` line for each column (D33).

#### Scenario: A column of digits held as text stays text

- **GIVEN** a Parquet column of type `byte_array` holding `"42"` and `"7"`
- **WHEN** the file is imported
- **THEN** the column is declared `String`
- **AND** the values are not read as numbers

#### Scenario: Import names the sheet after the file

- **WHEN** `sales.parquet` is imported
- **THEN** the output contains `sheet sales` with a type line for each column

### Requirement: A failed cell, and what Parquet cannot hold

A cell whose calculation failed SHALL be written as a null, with a notice
saying so. A nested column — a list, a struct, a map — SHALL be dropped on
import with a notice naming it.

Constants, computed-column formulas and functions SHALL NOT be carried by a
plain Parquet file; `--meta` is what carries them.

#### Scenario: A nested column is dropped with a notice

- **GIVEN** a Parquet file with a `list` column
- **WHEN** it is imported
- **THEN** the other columns are read
- **AND** a notice names the column that was dropped

#### Scenario: A failed cell becomes a null

- **GIVEN** a sheet with a cell whose formula failed
- **WHEN** it is exported
- **THEN** that cell is a null in the Parquet file
- **AND** a notice says a failed cell was written as null
