# omx-expressions

Strings, chained comparisons and the text functions.

## ADDED Requirements

### Requirement: Code takes single quotes and a cell double

A string in OMX code SHALL be written in single quotes, with the escapes
`\'`, `\\`, `\n` and `\t`. A double-quoted string in code SHALL be an error
that suggests the single-quoted form. Text in a table cell SHALL keep its
double quotes. Code means everything outside a table cell.

#### Scenario: A formula's string

- **WHEN** a formula is `Name // 'Unknown'`
- **THEN** it reads `Unknown` where `Name` is empty

#### Scenario: A double-quoted string in code

- **WHEN** a formula is `Name // "Unknown"`
- **THEN** checking it fails with "strings in OMX are in single quotes"
- **AND** the help suggests `'Unknown'`

#### Scenario: A cell keeps its double quotes

- **GIVEN** a table cell `"2 * 3 * 4"`
- **THEN** it is the text `2 * 3 * 4`

#### Scenario: Written code takes single quotes

- **WHEN** an Excel formula with a string is converted, or a calculated
  sheet is sorted from the app
- **THEN** the code written has its strings in single quotes

### Requirement: Comparisons chain

`a op b op c`, with each `op` one of `== != < > <= >=`, SHALL mean
`a op b and b op c`, with `b` evaluated once.

#### Scenario: A range test

- **WHEN** `x` is 5
- **THEN** `0 < x <= 10` is true and `0 < x <= 4` is false

### Requirement: The Polars tests are OMX functions

The operations Polars has that give a boolean SHALL be OMX functions under
Polars' names, also called as methods: `is_in`, `is_null`, `is_not_null`,
`is_nan`, `is_not_nan`, `is_finite`, `is_infinite`, `starts_with`,
`ends_with`, `contains`, `matches`, `contains_any`, `is_leap_year`,
`is_unique`, `is_duplicated`, `is_first_distinct` and `is_last_distinct`,
with `len_chars`, `to_lowercase` and `to_uppercase` to test text by.
`contains` SHALL match its part literally and `matches` SHALL take a
regex. `starts_with` SHALL also take a range of single characters, and is
then true when the first character is in the range. `x.is_in(list)` SHALL
be `x in list`.

#### Scenario: A range of first letters

- **THEN** `'Bob'.starts_with('A'..'C')` is true
- **AND** `'Dan'.starts_with('A'..'C')` is false

#### Scenario: Literal and regex

- **THEN** `'a.b'.contains('.')` is true and `'ab'.contains('.')` is false
- **AND** `'ab'.matches('^a.$')` is true

#### Scenario: Over the whole column

- **GIVEN** a column holding 1, 2, 2
- **THEN** `is_unique` gives true, false, false
- **AND** `is_first_distinct` gives true, true, false
