# evaluation

Table operations and incremental recalculation.

## ADDED Requirements

### Requirement: Grouping
The system SHALL support grouping a table by one or more columns and aggregating
each group, yielding a table with one row per distinct key.

#### Scenario: Revenue by region
- **WHEN** `Sales |> group(Region) |> sum(Revenue)` is evaluated
- **THEN** the result is a table with one row per distinct `Region` and that region's total `Revenue`

### Requirement: Joining
The system SHALL support joining two tables on a key column, at minimum as inner
and left joins.

#### Scenario: Left join on key
- **WHEN** `Sales.join(Customers, CustomerID)` is evaluated as a left join
- **THEN** the result has one row per `Sales` row with the matching `Customers` columns, empty where no customer matches

### Requirement: Table operations preserve exactness
Selection, sorting, grouping, joining and aggregation SHALL preserve the exact
numeric values of the data they operate on.

#### Scenario: Exact group sum
- **GIVEN** a `Rational` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10`

### Requirement: Recalculation
When an input changes, the system SHALL be able to recalculate only the values
that depend on it, and the result SHALL be identical to a full recalculation.

#### Scenario: Unrelated table untouched
- **GIVEN** tables `Sales` and `Inventory` with no references between them
- **WHEN** a `Sales` cell changes
- **THEN** no `Inventory` value is recalculated
