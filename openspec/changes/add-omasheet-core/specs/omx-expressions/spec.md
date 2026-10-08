# omx-expressions

OMX (Omasheet Expressions): the Raku-inspired formula language.

## ADDED Requirements

### Requirement: Standalone expression language
OMX SHALL be a self-contained expression language with its own grammar,
independent of the `.omx` file syntax, so that the same expression is valid in
a cell, a computed column, a constant, a Markdown interpolation and on the
command line. OMX SHALL NOT embed or delegate to another language.

#### Scenario: Same expression, different hosts
- **GIVEN** a sheet defining table `Sales`
- **WHEN** `Sales[Revenue > 1000].Revenue.sum()` is used in a `const` declaration and passed to `omasheet eval`
- **THEN** both produce the same value

### Requirement: Reference forms
OMX SHALL resolve names as follows: a table name denotes the table; `Table.Column`
denotes the whole column as a vector; a bare column name inside an expression
that has a row context denotes that column's value in the current row; a constant
name denotes the constant.

#### Scenario: Whole column
- **WHEN** `Sales.Revenue` is evaluated
- **THEN** the result is a vector with one element per row of `Sales`

#### Scenario: Bare column in row context
- **GIVEN** a computed column `Profit := Revenue - Cost` in table `Sales`
- **WHEN** it is evaluated for a row
- **THEN** `Revenue` and `Cost` are that row's values

#### Scenario: Bare column without row context
- **WHEN** `Revenue` is evaluated by `omasheet eval` with no row context and no constant named `Revenue`
- **THEN** an unresolved-name error is reported

### Requirement: Selection with `[]` and dimension separator `;`
`[]` SHALL be the single selection operator. Within `[]`, `;` SHALL separate
dimensions: the first slot selects rows, the second selects columns, and further
slots select further dimensions of N-dimensional values. A column slot SHALL
accept a column name or a 0-based position. An empty slot SHALL select the whole
of that dimension, and trailing slots MAY be omitted with the same meaning.
`Table[rows].Column` SHALL be equivalent to `Table[rows; Column]`.

#### Scenario: Single cell by position and name
- **WHEN** `Sales[1; Revenue]` is evaluated
- **THEN** the result is the scalar `Revenue` of the second row of `Sales`

#### Scenario: Field access equivalence
- **WHEN** `Sales[1].Revenue` and `Sales[1; Revenue]` are evaluated
- **THEN** both yield the same value

#### Scenario: Two-dimensional slice
- **WHEN** `Sales[2..5; 3..7]` is evaluated
- **THEN** the result is a 4 × 5 matrix of rows 2–5 and columns 3–7

#### Scenario: Empty slot selects the whole dimension
- **WHEN** `Sales[; Revenue]` and `Sales[2; ]` are evaluated
- **THEN** the first equals `Sales.Revenue` and the second is every column of the third row

### Requirement: Positional indexing
Row and column positions SHALL be 0-based. A negative integer position SHALL
count from the end, so `[-1]` is the last element.

#### Scenario: First and last
- **GIVEN** `Sales.Revenue` is `[100, 120, 150]`
- **WHEN** `Sales.Revenue[0]` and `Sales.Revenue[-1]` are evaluated
- **THEN** the results are `100` and `150`

### Requirement: Ranges
`a..b` SHALL denote the inclusive range from `a` to `b`. `a..^b` SHALL denote the
range excluding `b`. Ranges SHALL be first-class values that can be bound to
names and used in any index slot.

#### Scenario: Inclusive and exclusive
- **WHEN** `Sales[0..2]` and `Sales[0..^2]` are evaluated
- **THEN** the first has three rows and the second has two

#### Scenario: Range bound to a name
- **GIVEN** `const Rows = 2..100` and `const Cols = 3..7`
- **WHEN** `Sales[Rows; Cols]` is evaluated
- **THEN** it is equivalent to `Sales[2..100; 3..7]`

### Requirement: Row cursor `*`
`*` in a row slot SHALL always denote the cursor, the current row, and `*+n` /
`*-n` SHALL denote the row `n` positions after / before the current row *of the
indexed table*. `*` SHALL be usable as a range endpoint. `*` SHALL NOT mean "the
whole dimension"; that is written as an empty slot. The current row SHALL be the
row of the cell or computed column that contains the expression, including
inside a row condition, where it SHALL NOT mean the candidate row. A cursor, with
or without an offset, SHALL be an error in an expression with no row context.

#### Scenario: Previous row
- **GIVEN** a computed column `Growth := Revenue / Sales[*-1; Revenue] - 1` in `Sales`
- **WHEN** it is evaluated for the third row
- **THEN** `Sales[*-1; Revenue]` is the second row's `Revenue`

#### Scenario: Year-over-year
- **WHEN** `Revenue / Sales[*-12; Revenue] - 1` is evaluated on monthly data
- **THEN** each row is compared with the row twelve positions earlier

#### Scenario: Running total
- **GIVEN** `Running := Sales[0..*; Revenue].sum()`
- **WHEN** it is evaluated for row `i`
- **THEN** the result is the sum of `Revenue` over rows `0` through `i` inclusive

#### Scenario: Trailing window
- **WHEN** `Sales[*-2..*; Revenue].avg()` is evaluated for row `i ≥ 2`
- **THEN** the result is the mean of `Revenue` over rows `i-2`, `i-1`, `i`

#### Scenario: Offset falls outside the table
- **WHEN** `Sales[*-1; Revenue]` is evaluated for the first row
- **THEN** the result is empty (undefined), not an error

#### Scenario: Cursor offset without row context
- **WHEN** `omasheet eval 'Sales[*-1; Revenue]'` is run
- **THEN** an error states that `*-1` needs a current row

#### Scenario: Cursor inside a row condition
- **GIVEN** a computed column `Count := Orders[Region == ByRegion[*; Region]].count()` in `ByRegion`
- **WHEN** it is evaluated for a row
- **THEN** `ByRegion[*; Region]` is that row's `Region`, compared with the `Region` of each `Orders` row

#### Scenario: Bare cursor without row context
- **WHEN** `omasheet eval 'Sales[*; Revenue]'` is run
- **THEN** an error states that `*` needs a current row

#### Scenario: Semantics survive row insertion
- **GIVEN** a column defined with `Sales[*-1; Revenue]`
- **WHEN** a new row is inserted in the middle of the table source
- **THEN** every row still refers to its own immediate predecessor, with no formula edits

### Requirement: Predicate selection
A row slot SHALL accept a boolean expression, selecting the rows for which it is
true. Inside the predicate, bare column names of the indexed table SHALL refer to
the candidate row. Names not found in the indexed table SHALL resolve in the
enclosing context, including the enclosing row. Predicates SHALL be combinable
with `and`, `or` and `not`.

#### Scenario: Conditional sum (replaces SUMIFS)
- **WHEN** the following is evaluated
  ```
  Sales[
      Region == "UK" and
      Date >= 2025-01-01 and
      Date < 2026-01-01
  ].Revenue.sum()
  ```
- **THEN** the result is the total `Revenue` of UK rows dated within 2025

#### Scenario: Lookup from another table (replaces XLOOKUP)
- **GIVEN** `Customers` with columns `ID`, `Name` and `Sales` with column `CustomerID`
- **WHEN** a computed column in `Sales` is `Customers[ID == CustomerID].Name // "Unknown"`
- **THEN** each row gets the matching customer's `Name`, or `"Unknown"` when no customer matches

### Requirement: Operators
OMX SHALL provide arithmetic `+ - * / **`, comparison `== != < > <= >=`, boolean
`and or not`, membership `in`, and the fallback operator `//`, which yields its
left operand unless that operand is empty, in which case it yields its right
operand. `=` SHALL NOT be a comparison operator.

#### Scenario: Fallback on empty
- **WHEN** `Sales[*-1; Revenue] // 0` is evaluated for the first row
- **THEN** the result is `0`

#### Scenario: Membership
- **WHEN** `Region in ["UK", "US"]` is evaluated for a row whose `Region` is `"US"`
- **THEN** the result is true

#### Scenario: Single equals in a predicate
- **WHEN** `Sales[Region = "UK"]` is parsed
- **THEN** a syntax error suggests `==`

### Requirement: Conditional expression
OMX SHALL provide `if <cond> then <a> else <b>` as an expression that may span
multiple lines.

#### Scenario: Conditional discount
- **WHEN** the following is evaluated for a row with `Revenue` 2000 and `Region` `"UK"`
  ```
  if Revenue > 1000 and Region in ["UK", "US"]
  then Revenue * 90%
  else Revenue
  ```
- **THEN** the result is `1800`

### Requirement: Value shapes
Every OMX value SHALL have a shape that is one of scalar, vector, matrix
(2-D), or table (2-D with named, typed columns). Selection SHALL reduce shape
predictably: a table with a row predicate or row range is a table; a table with a
single column is a vector; a single row and single column is a scalar.

#### Scenario: Shape reduction chain
- **WHEN** `Sales[Region == "UK"]`, `Sales[Region == "UK"].Revenue` and `Sales[Region == "UK"].Revenue.sum()` are evaluated
- **THEN** the results are respectively a table, a vector and a scalar

### Requirement: Aggregation methods
Vectors SHALL provide the methods `.sum()`, `.avg()`, `.min()`, `.max()` and
`.count()`, each returning a scalar.

#### Scenario: Column aggregate
- **GIVEN** `Sales.Revenue` is `[100, 120, 150]`
- **WHEN** `Sales.Revenue.sum()` and `Sales.Revenue.avg()` are evaluated
- **THEN** the results are `370` and exactly `370/3`

### Requirement: Broadcasting
Arithmetic and comparison operators SHALL apply element-wise between a vector and
a scalar and between two vectors of equal length. A comparison over a vector
SHALL yield a boolean vector usable as a row predicate. Operating on vectors of
unequal length SHALL be an error.

#### Scenario: Scalar broadcast
- **WHEN** `[100, 200, 300] * 20%` is evaluated
- **THEN** the result is `[20, 40, 60]`

#### Scenario: Column arithmetic
- **WHEN** `Sales.Revenue - Sales.Cost` is evaluated
- **THEN** the result is a vector of per-row differences

#### Scenario: Boolean mask
- **WHEN** `Sales[Sales.Revenue > 1000]` is evaluated
- **THEN** the result is the table of rows whose `Revenue` exceeds 1000

#### Scenario: Length mismatch
- **WHEN** a vector of length 3 is added to a vector of length 4
- **THEN** a shape error is reported before execution

### Requirement: Pipe operator
OMX SHALL provide `|>`, which passes its left operand as the first argument of
the call on its right, with at least the stages `filter(<predicate>)`,
`select(<columns>)` and the aggregation functions.

#### Scenario: Pipeline equals bracket form
- **WHEN** the following is evaluated
  ```
  Sales
    |> filter(Region == "UK")
    |> select(Revenue)
    |> sum()
  ```
- **THEN** the result equals `Sales[Region == "UK"].Revenue.sum()`
