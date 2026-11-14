# parquet-interop

## MODIFIED Requirements

### Requirement: A decimal column keeps its scale

A Parquet `Decimal(p, s)` column with `p` at most 38 SHALL be read as `Decimal(s)`, and a `Decimal(n)` column SHALL be written as `Decimal(38, n)`. A `Decimal256` column SHALL be refused, saying it is wider than a `Decimal`.

#### Scenario: A round trip

- **GIVEN** a column `Decimal(2)` holding `19.99`
- **WHEN** it is written to Parquet and read back
- **THEN** it is `Decimal(2)` holding `19.99`

## ADDED Requirements

### Requirement: A Fraction column is written exactly

A `Fraction` column SHALL be written as a struct of two strings, `num` and `den`, holding the reduced numerator and denominator in decimal, with its kind in the file's key-value metadata as `omasheet.kind.<column>`. A file read back with the key SHALL give a `Fraction` column holding the same values. With `--decimal` the column SHALL be written as the nearest `Decimal(38, s)` or `Float64`, and the cells that were rounded or could not be written SHALL be reported.

#### Scenario: A googol survives

- **GIVEN** a `Fraction` column holding `10 ** 100` and `1/3`
- **WHEN** it is written and read back
- **THEN** both values are exactly as they were

#### Scenario: A rounded export reports what it cut

- **GIVEN** the same column
- **WHEN** it is exported with `--decimal`
- **THEN** `1/3` is written rounded, and the report names the cells that were cut or could not be written
