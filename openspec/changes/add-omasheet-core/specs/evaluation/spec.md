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
Types and shapes SHALL be checked for the whole sheet before any
calculation runs. If checking reports any error, the system SHALL NOT execute any
part of the sheet.

#### Scenario: One type error blocks execution
- **GIVEN** a sheet with ten valid computed columns and one defined as `Revenue + Month` where `Revenue` is `Rat` and `Month` is `Text`
- **WHEN** the sheet is evaluated
- **THEN** the type error is reported and no column is calculated

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
