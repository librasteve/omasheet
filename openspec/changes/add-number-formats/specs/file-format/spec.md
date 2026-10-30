# file-format

A number format on a column.

## MODIFIED Requirements

### Requirement: Column schema
A column of type `Integer`, `Decimal` or `Float` MAY be declared with a
format after its type, in brackets: `<Column> : <Type>[<format>]`. The
format SHALL be the flags, width, precision and conversion of a C `printf`
conversion, without its `%`. A file SHALL be written out with the format as
it was read.

#### Scenario: A formatted column
- **GIVEN** the source
  ```omx
  sheet Items

  Price : Decimal['012.2f]

  Item | Price
  Tea  | 1234.5
  ```
- **WHEN** the file is parsed
- **THEN** `Items.Price` has type `Decimal` with the format `'012.2f`, and the first row's `Price` is exactly `1234.5`

#### Scenario: A format that does not read
- **GIVEN** a schema line `Price : Decimal[012.2q]`
- **WHEN** the file is parsed
- **THEN** parsing reports an error at the format

#### Scenario: A format on a type that takes none
- **GIVEN** a schema line `Name : String[5d]`
- **WHEN** the file is parsed
- **THEN** parsing reports an error: only `Integer`, `Decimal` and `Float` take a format

#### Scenario: A conversion its type does not take
- **GIVEN** a schema line `Qty : Integer[.2f]`
- **WHEN** the file is parsed
- **THEN** parsing reports an error: an `Integer` takes `d`, `i`, `x`, `X`, `o`, `b` or `B`
