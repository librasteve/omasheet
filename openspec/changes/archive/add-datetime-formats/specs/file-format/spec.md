# file-format

A date and time format on a column.

## MODIFIED Requirements

### Requirement: Column schema
A column of type `Date`, `Time` or `Datetime` MAY be declared with a format
after its type, in brackets: `<Column> : <Type>[<format>]`. The format SHALL
be a C `strftime` format. A file SHALL be written out with the format as it
was read, and its values in ISO form.

#### Scenario: A formatted date column
- **GIVEN** the source
  ```omx
  sheet Invoices

  Due : Date[%d %b %Y]

  Item | Due
  Tea  | 2025-01-31
  ```
- **WHEN** the file is parsed
- **THEN** `Invoices.Due` has type `Date` with the format `%d %b %Y`, and the first row's `Due` is 2025-01-31

#### Scenario: A format that does not read
- **GIVEN** a schema line `Due : Date[%d %q %Y]`
- **WHEN** the file is parsed
- **THEN** parsing reports an error at the format

#### Scenario: A conversion its type does not take
- **GIVEN** a schema line `Due : Date[%d %H:%M]`
- **WHEN** the file is parsed
- **THEN** parsing reports an error: a `Date` takes no time of day
