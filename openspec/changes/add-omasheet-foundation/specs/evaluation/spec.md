# evaluation

How a sheet is compiled, checked and calculated.

## ADDED Requirements

### Requirement: Compiled, not string-evaluated
The system SHALL compile sheet source and OMX expressions through parsing,
semantic analysis and a typed intermediate representation before execution.
Table, column and constant names SHALL be resolved to identifiers at compile
time, and the runtime SHALL NOT parse or look up expression text during
calculation.

#### Scenario: Unresolved name is a compile error
- **WHEN** a computed column refers to `Revnue` and no such column, table or constant exists
- **THEN** an error is reported at that source location and no calculation is performed

### Requirement: Static checking before execution
Types, units and shapes SHALL be checked for the whole sheet before any
calculation runs. If checking reports any error, the system SHALL NOT execute any
part of the sheet.

#### Scenario: One unit error blocks execution
- **GIVEN** a sheet with ten valid computed columns and one defined as `Revenue + Weight` where units are `GBP` and `kg`
- **WHEN** the sheet is evaluated
- **THEN** the unit error is reported and no column is calculated

#### Scenario: All errors reported together
- **WHEN** a sheet contains three independent type errors
- **THEN** all three are reported in one run

### Requirement: Dependency-ordered calculation
The system SHALL derive a dependency graph between constants, cells and computed
columns and calculate each after everything it depends on, regardless of the
order of declarations in the file.

#### Scenario: Declaration order does not matter
- **GIVEN** `Margin := Profit / Revenue` declared before `Profit := Revenue - Cost`
- **WHEN** the sheet is evaluated
- **THEN** `Profit` is calculated first and `Margin` is correct

#### Scenario: Row-wise self reference is allowed
- **GIVEN** `Balance := (Ledger[*-1; Balance] // 0) + Amount`
- **WHEN** the sheet is evaluated
- **THEN** each row's `Balance` is calculated after the previous row's, yielding a running balance

### Requirement: Cycle detection
A dependency cycle SHALL be reported as an error that names the participants in
the cycle.

#### Scenario: Two columns depend on each other
- **GIVEN** `A := B + 1` and `B := A + 1` in the same table
- **WHEN** the sheet is checked
- **THEN** an error reports the cycle `A → B → A`

### Requirement: Recalculation
When an input changes, the system SHALL be able to recalculate only the values
that depend on it, and the result SHALL be identical to a full recalculation.

#### Scenario: Unrelated table untouched
- **GIVEN** tables `Sales` and `Inventory` with no references between them
- **WHEN** a `Sales` cell changes
- **THEN** no `Inventory` value is recalculated

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

### Requirement: Table operations preserve exactness and units
Selection, sorting, grouping, joining and aggregation SHALL preserve the exact
numeric values, units and uncertainties of the data they operate on, whatever
library executes them.

> This constrains how Polars may be used — see design Q2.

#### Scenario: Exact group sum
- **GIVEN** a `Rat<GBP>` column with values `0.1`, `0.2` in one group
- **WHEN** the group is summed
- **THEN** the result is exactly `3/10 GBP`
