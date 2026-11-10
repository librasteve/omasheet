# parquet-interop

## MODIFIED Requirements

### Requirement: A failed cell, and what Parquet cannot hold

A nested column — a list, a struct, a map — SHALL still be dropped on import with a notice naming it, **except a struct of exactly two `Float64` fields named `re` and `im`, in that order**, which SHALL be read as a `Complex` column.

#### Scenario: A Complex column is a struct

- **GIVEN** a sheet with a `Complex` column
- **WHEN** it is exported to Parquet
- **THEN** the column is a struct with `Float64` fields `re` and `im`
- **AND** importing the file gives a `Complex` column with the same values

#### Scenario: Another struct is still dropped

- **GIVEN** a Parquet file with a struct column of fields `x` and `y`
- **WHEN** it is imported
- **THEN** the other columns are read
- **AND** a notice names the column that was dropped

#### Scenario: A struct with a null field

- **GIVEN** a `re`/`im` struct column where one row has a null `im`
- **WHEN** it is imported
- **THEN** that cell is invalid, with a notice
- **AND** a row whose whole struct is null is `Null`
