# omx-expressions

Partitioned cursor offsets and the `group` pipe stage.

## ADDED Requirements

### Requirement: Partitioned cursor offsets
A cursor offset SHALL accept a `by <Column>` clause that restricts the offset to
rows sharing the current row's value of that column.

#### Scenario: Previous row within the same region
- **GIVEN** `Sales` rows ordered UK-Jan, UK-Feb, US-Jan, US-Feb
- **WHEN** `Sales[Revenue; *-1 by Region]` is evaluated for US-Jan
- **THEN** the result is empty, because there is no earlier US row
- **AND** for US-Feb the result is US-Jan's `Revenue`

## MODIFIED Requirements

### Requirement: Pipe operator
OMX SHALL provide `|>`, which passes its left operand as the first argument of
the call on its right, with at least the stages `filter(<predicate>)`,
`select(<columns>)`, `group(<columns>)` and the aggregation functions.

#### Scenario: Pipeline equals bracket form
- **WHEN** the following is evaluated
  ```
  Sales
    |> filter(Region == "UK")
    |> select(Revenue)
    |> sum()
  ```
- **THEN** the result equals `Sales[; Region == "UK"].Revenue.sum()`

#### Scenario: Group stage
- **WHEN** `Sales |> group(Region) |> sum(Revenue)` is evaluated
- **THEN** the result is the grouped table specified by the `evaluation` Grouping requirement
