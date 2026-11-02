# evaluation

A filter expression.

## ADDED Requirements

### Requirement: A filter is an expression over a column

A filter SHALL be an OMX expression in which `*` is the value of the
filtered column's cell in each row. It SHALL use only literals, `*`,
comparisons, `in`, `and`, `or`, `not`, `starts_with`, `ends_with`,
`contains` and `is_null`, and anything else SHALL be an error when it is
checked, before any row is read. It SHALL compare values, not the texts the
grid shows. An empty cell SHALL fail every comparison and match `* == ''`
and `*.is_null()`.

#### Scenario: Values, not texts

- **GIVEN** a Decimal column showing `3.00`
- **WHEN** it is filtered by `* == 3`
- **THEN** the row is shown

#### Scenario: Empty cells and `!=`

- **GIVEN** a column holding 3, 4 and an empty cell
- **WHEN** it is filtered by `* != 3`
- **THEN** only the row holding 4 is shown

#### Scenario: A wrong type is caught first

- **GIVEN** an Int column
- **WHEN** `* == 'x'` is checked
- **THEN** it is an error, and no row is read

### Requirement: A table's filter runs in Polars

A filter on a column that Polars holds SHALL be run as one Polars
expression over the column. A text column's values SHALL NOT be formatted
to apply it.

#### Scenario: The same rows either way

- **GIVEN** the same rows held in the text and in a Parquet table
- **WHEN** each is filtered by the same expression
- **THEN** the same rows are shown
