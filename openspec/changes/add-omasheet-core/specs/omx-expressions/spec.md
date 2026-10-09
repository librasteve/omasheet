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
- **WHEN** `Sales[; Revenue > 1000].Revenue.sum()` is used in a `const` declaration and passed to `omasheet eval`
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
dimensions: the first slot selects columns, the second selects rows, and further
slots select further dimensions of N-dimensional values. A column slot SHALL
accept a column name or a 0-based position. An empty slot SHALL select the whole
of that dimension, and trailing slots MAY be omitted with the same meaning, so a
table index with one slot selects columns: `Table[Column]` is the whole column,
and rows alone are selected with `Table[; rows]`. A vector has one dimension,
and its single slot selects elements.
`Table[; rows].Column` SHALL be equivalent to `Table[Column; rows]`. Inside a
table, a selection written with no table name SHALL select from that table when
it has a `;` or its first slot is a cursor; any other `[...]` with no table name
SHALL be a vector. Outside a table it SHALL be an error.

In a formula's own table, a column slot of `*`, `*+n` or `*-n` SHALL be the
column of the formula, or the one `n` columns to its right or left.

#### Scenario: Column counted from the formula
- **GIVEN** a cell formula in the second column of a table
- **WHEN** it contains `[*-1; *]`
- **THEN** it is the cell to its left

#### Scenario: Selection with no table name
- **GIVEN** a formula in table `Sales`
- **WHEN** it contains `[Revenue; *-1]`
- **THEN** it means `Sales[Revenue; *-1]`

#### Scenario: Single cell by position and name
- **WHEN** `Sales[Revenue; 1]` is evaluated
- **THEN** the result is the scalar `Revenue` of the second row of `Sales`

#### Scenario: Field access equivalence
- **WHEN** `Sales[; 1].Revenue` and `Sales[Revenue; 1]` are evaluated
- **THEN** both yield the same value

#### Scenario: Two-dimensional slice
- **WHEN** `Sales[3..7; 2..5]` is evaluated
- **THEN** the result is an array of cells, rows 2–5 by columns 3–7: four rows by five columns

#### Scenario: Empty or missing slot selects the whole dimension
- **WHEN** `Sales[Revenue]`, `Sales[Revenue; ]` and `Sales[; 2]` are evaluated
- **THEN** the first two equal `Sales.Revenue` and the third is every column of the third row

#### Scenario: One slot is columns
- **WHEN** `Sales[Region == "UK"]` is checked
- **THEN** an error says a column selector must be a column name, a position or a range

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
- **WHEN** `Sales[; 0..2]` and `Sales[; 0..^2]` are evaluated
- **THEN** the first has three rows and the second has two

#### Scenario: Range bound to a name
- **GIVEN** `const Rows = 2..100` and `const Cols = 3..7`
- **WHEN** `Sales[Cols; Rows]` is evaluated
- **THEN** it is equivalent to `Sales[3..7; 2..100]`

### Requirement: Row cursor `*`
`*` in a row slot SHALL always denote the cursor, the current row, and `*+n` /
`*-n` SHALL denote the row `n` positions after / before the current row *of the
indexed table*. `*` SHALL be usable as a range endpoint. `*` SHALL NOT mean "the
whole dimension"; that is written as an empty slot. The current row SHALL be the
row of the cell or computed column that contains the expression, including
inside a row condition, where it SHALL NOT mean the candidate row. A cursor, with
or without an offset, SHALL be an error in an expression with no row context.

#### Scenario: Previous row
- **GIVEN** a computed column `Growth := Revenue / Sales[Revenue; *-1] - 1` in `Sales`
- **WHEN** it is evaluated for the third row
- **THEN** `Sales[Revenue; *-1]` is the second row's `Revenue`

#### Scenario: Year-over-year
- **WHEN** `Revenue / Sales[Revenue; *-12] - 1` is evaluated on monthly data
- **THEN** each row is compared with the row twelve positions earlier

#### Scenario: Running total
- **GIVEN** `Running := Sales[Revenue; 0..*].sum()`
- **WHEN** it is evaluated for row `i`
- **THEN** the result is the sum of `Revenue` over rows `0` through `i` inclusive

#### Scenario: Trailing window
- **WHEN** `Sales[Revenue; *-2..*].avg()` is evaluated for row `i ≥ 2`
- **THEN** the result is the mean of `Revenue` over rows `i-2`, `i-1`, `i`

#### Scenario: Offset falls outside the table
- **WHEN** `Sales[Revenue; *-1]` is evaluated for the first row
- **THEN** the result is empty (undefined), not an error

#### Scenario: Cursor offset without row context
- **WHEN** `omasheet eval 'Sales[Revenue; *-1]'` is run
- **THEN** an error states that `*-1` needs a current row

#### Scenario: Cursor inside a row condition
- **GIVEN** a computed column `Count := Orders[; Region == ByRegion[Region; *]].count()` in `ByRegion`
- **WHEN** it is evaluated for a row
- **THEN** `ByRegion[Region; *]` is that row's `Region`, compared with the `Region` of each `Orders` row

#### Scenario: Bare cursor without row context
- **WHEN** `omasheet eval 'Sales[Revenue; *]'` is run
- **THEN** an error states that `*` needs a current row

#### Scenario: Semantics survive row insertion
- **GIVEN** a column defined with `Sales[Revenue; *-1]`
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
  Sales[;
      Region == "UK" and
      Date >= 2025-01-01 and
      Date < 2026-01-01
  ].Revenue.sum()
  ```
- **THEN** the result is the total `Revenue` of UK rows dated within 2025

#### Scenario: Lookup from another table (replaces XLOOKUP)
- **GIVEN** `Customers` with columns `ID`, `Name` and `Sales` with column `CustomerID`
- **WHEN** a computed column in `Sales` is `Customers[; ID == CustomerID].Name // "Unknown"`
- **THEN** each row gets the matching customer's `Name`, or `"Unknown"` when no customer matches

### Requirement: Operators
OMX SHALL provide arithmetic `+ - * / **`, comparison `== != < > <= >=`, boolean
`and or not`, membership `in`, and the fallback operator `//`, which yields its
left operand unless that operand is empty, in which case it yields its right
operand. `=` SHALL NOT be a comparison operator.

#### Scenario: Fallback on empty
- **WHEN** `Sales[Revenue; *-1] // 0` is evaluated for the first row
- **THEN** the result is `0`

#### Scenario: Membership
- **WHEN** `Region in ["UK", "US"]` is evaluated for a row whose `Region` is `"US"`
- **THEN** the result is true

#### Scenario: Single equals in a predicate
- **WHEN** `Sales[; Region = "UK"]` is parsed
- **THEN** a syntax error suggests `==`

### Requirement: Dates and times
OMX SHALL provide the types `Date`, `Time` (a time of day) and `DateTime`,
written with no time zone or offset as `2025-01-31`, `09:30` or `09:30:15`, and
`2025-01-31T09:30` or `2025-01-31T09:30:15`. Each SHALL compare only with its
own kind. `+` and `-` SHALL combine them as follows, and no other arithmetic
SHALL apply to them:

- a `Date` plus or minus an `Int` is the `Date` that many days away;
- a `Time` or `DateTime` plus or minus an `Int` is the value that many seconds
  away, a `Time` going round midnight;
- the difference of two `Date` values is an `Int` number of days, and of two
  `Time` or two `DateTime` values an `Int` number of seconds;
- a `Date` plus a `Time` is a `DateTime`.

The methods `.year()`, `.month()`, `.day()` and `.weekday()` (1 for Monday to 7
for Sunday) SHALL apply to a `Date` or `DateTime`; `.hour()`, `.minute()` and
`.second()` to a `Time` or `DateTime`; `.date()` and `.time()` SHALL give the two
halves of a `DateTime`. `today()` SHALL give the `Date` and `now()` the
`DateTime` at the time of evaluation.

#### Scenario: Days between dates
- **WHEN** `2025-03-01 - 2024-03-01` is evaluated
- **THEN** the result is `365`

#### Scenario: Adding days crosses a month
- **WHEN** `2025-01-31 + 1` is evaluated
- **THEN** the result is `2025-02-01`

#### Scenario: A time goes round midnight
- **WHEN** `23:30 + 3600` is evaluated
- **THEN** the result is `00:30`

#### Scenario: Date and time make a date-time
- **WHEN** `2025-01-31 + 09:30` is evaluated
- **THEN** the result is `2025-01-31T09:30`

#### Scenario: Kinds do not mix
- **WHEN** `2025-01-31T09:30 - 2025-01-31` is checked
- **THEN** a type error says `-` cannot be applied to DateTime and Date

#### Scenario: Parts of a date
- **WHEN** `2026-10-08.weekday()` is evaluated
- **THEN** the result is `4`

### Requirement: Time zones
A `DateTime` SHALL be a time on the clocks of the sheet's time zone: the zone
the sheet names, or else the zone of the machine. `.to_zone("<name>")` SHALL give
the same instant on the clocks of the named zone, `.utc()` on those of UTC and
`.local()` on those of the machine, each following the daylight saving rules of
the time zone database. The result SHALL be a `DateTime` that keeps its zone and
is printed with its offset from UTC, unless the zone is the sheet's own.
`.offset()` SHALL give the seconds a `DateTime` is ahead of UTC and `.zone()` the
name of its zone. Two `DateTime` values in different zones SHALL compare and
subtract as instants; two in the sheet's own zone as the clocks show them. A
zone the time zone database does not have SHALL be an error. `now()` SHALL be on
the clocks of the sheet's zone.

#### Scenario: Convert to another zone
- **GIVEN** the sheet's zone is `Europe/London`
- **WHEN** `2025-07-15T12:00.to_zone("Asia/Tokyo")` is evaluated
- **THEN** the result is `2025-07-15T20:00+09:00`

#### Scenario: Daylight saving
- **GIVEN** the sheet's zone is `Europe/London`
- **WHEN** `2025-07-15T12:00.offset()` and `2025-01-15T12:00.offset()` are evaluated
- **THEN** the results are `3600` and `0`

#### Scenario: The same instant is equal
- **GIVEN** the sheet's zone is `Europe/London`
- **WHEN** `2025-07-15T12:00.to_zone("Asia/Tokyo") == 2025-07-15T12:00` is evaluated
- **THEN** the result is true

#### Scenario: Elapsed time across a clock change
- **GIVEN** the sheet's zone is `Europe/London`
- **WHEN** `2025-03-30T12:00 - 2025-03-29T12:00` and `2025-03-30T12:00.utc() - 2025-03-29T12:00.utc()` are evaluated
- **THEN** the results are `86400` and `82800`

#### Scenario: Unknown zone
- **WHEN** `now().to_zone("Mars/Base")` is evaluated
- **THEN** an error says there is no time zone `Mars/Base`

### Requirement: Mathematical functions
OMX SHALL provide functions of one real number, each usable as `f(x)` or
`x.f()` and applied to each element of a vector. `abs`, `sign`, `round`, `floor`
and `ceil` SHALL give an exact result for an exact number, `round` taking a half
away from zero. `sqrt`, `exp`, `ln`, `log10`, `log2`, `sin`, `cos`, `tan`,
`asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `radians` and `degrees` SHALL
give a `Num`, with angles in radians, and `pi()` SHALL give π and `e()` the
base of the natural logarithm, each as a `Num`. A
number outside a function's domain SHALL be an error. `re`, `im`, `conj` and
`arg` SHALL give the real part, imaginary part, conjugate and angle of a
`Complex`, and `abs`, `sqrt`, `exp` and `ln` SHALL accept one; the other
functions SHALL NOT. Every function SHALL be
listed, with its category, usage and a summary, in one function directory. The
usage SHALL be written with placeholders, such as `Table.Column.sum()`, rather
than with the names of any particular sheet.

#### Scenario: Exact rounding
- **WHEN** `round(5/2)` and `floor(-7/2)` are evaluated
- **THEN** the results are the `Int` values `3` and `-4`

#### Scenario: Square root
- **WHEN** `sqrt(16)` is evaluated
- **THEN** the result is the `Num` `4.0`

#### Scenario: Outside the domain
- **WHEN** `sqrt(-1)` is evaluated
- **THEN** an error says `sqrt` is not defined for that number

### Requirement: Calling a sheet's functions
A function a sheet defines SHALL be called like a built-in one: as `F(a, b)`,
as `a.F(b)`, and as `a |> F(b)`. Each call SHALL be checked before execution
for the types and shapes of its own arguments, so one function serves numbers,
vectors and tables alike. A call with the wrong number of arguments SHALL be an
error that shows the function's usage. What is wrong inside a function for the
arguments of a call, before or during execution, SHALL be reported at that call
and SHALL name the function. The sheet's functions SHALL be listed in the
function directory, ahead of the built-in ones, with the description written
above each or else its expression.

#### Scenario: Three ways to call
- **GIVEN** `func WithTax(x) = x * 120%`
- **WHEN** `WithTax(100)`, `100.WithTax()` and `100 |> WithTax()` are evaluated
- **THEN** each result is `120`

#### Scenario: A table as an argument
- **GIVEN** `func Total(t) = t.Revenue.sum()`
- **WHEN** `Total(Sales |> filter(Region == "UK"))` is evaluated
- **THEN** the result is the sum of the UK rows' `Revenue`

#### Scenario: A type error is reported at the call
- **GIVEN** `func WithTax(x) = x * 120%`
- **WHEN** `WithTax("ten")` is checked
- **THEN** an error at the call says that in `WithTax` `*` cannot be applied to Text

#### Scenario: Wrong number of arguments
- **GIVEN** `func Margin(revenue, cost) = (revenue - cost) / revenue`
- **WHEN** `Margin(1)` is checked
- **THEN** an error says `Margin` takes 2 arguments and shows `Margin(revenue, cost)`

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
Every OMX value SHALL have a shape that is one of scalar, vector, or table
(2-D with named, typed columns). Selection SHALL reduce shape
predictably: a table with a row predicate or row range is a table; a table with a
single column is a vector; a single row and single column is a scalar.

#### Scenario: Shape reduction chain
- **WHEN** `Sales[; Region == "UK"]`, `Sales[; Region == "UK"].Revenue` and `Sales[; Region == "UK"].Revenue.sum()` are evaluated
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
- **WHEN** `Sales[; Sales.Revenue > 1000]` is evaluated
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
- **THEN** the result equals `Sales[; Region == "UK"].Revenue.sum()`
