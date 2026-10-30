# file-format

One type name, which is now spelled as Polars spells it.

## MODIFIED Requirements

### Requirement: The date-time type is written `Datetime`

A column of date-times SHALL be declared `Datetime`, as Polars spells the
dtype it names, and `DateTime` SHALL NOT be accepted. The conversion SHALL
follow the type name, so it is written `x.Datetime`, `x.Datetime()` or
`Datetime(x)`.

The other type names SHALL be unchanged. `Decimal`, `String`, `Boolean`,
`Date` and `Time` are already the Polars spellings; `Int` and `Float` keep
theirs until there is more than one width of each to name; and `Fraction` and
`Percent` name no dtype at all, being displays of a `Decimal`.

#### Scenario: A date-time column

- **GIVEN** a schema line `Placed : Datetime`
- **WHEN** the file is checked
- **THEN** the column holds date-times

#### Scenario: The old spelling

- **GIVEN** a schema line `Placed : DateTime`
- **WHEN** the file is checked
- **THEN** the type is reported unknown
- **AND** the diagnostic lists the types, among them `Datetime`
