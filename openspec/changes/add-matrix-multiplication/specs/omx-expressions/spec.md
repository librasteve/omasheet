# omx-expressions

Matrix values, the matrix product and `transpose`.

## ADDED Requirements

### Requirement: Matrix values
OMX SHALL have a matrix value: a rectangle of numbers with rows and columns
known by position, not by name. A vector literal whose elements are vectors of
equal length SHALL be a matrix, each inner vector a row; inner vectors of
unequal length SHALL be an error. A matrix SHALL be indexed as `m[row; column]`
with the positions, ranges and empty slots of the core indexing model: one row
and one column give a scalar, one of them alone gives a vector, and ranges give
a matrix. A matrix SHALL NOT be the value of a cell.

#### Scenario: Literal and element
- **GIVEN** `const M = [[1, 2], [3, 4]]`
- **WHEN** `M[1; 0]`, `M[0]` and `M[; 1]` are evaluated
- **THEN** the results are `3`, `[1, 2]` and `[2, 4]`

#### Scenario: Ragged literal
- **WHEN** `[[1, 2], [3]]` is checked
- **THEN** an error says the rows of a matrix must be the same length

### Requirement: A table as a matrix
A table whose selected columns are all numeric SHALL be usable wherever a matrix
is wanted, its rows and columns in order, and `.matrix()` SHALL give that matrix
explicitly. A table with a column that is not numeric SHALL be an error that
names the column.

#### Scenario: Selected columns
- **GIVEN** `Sales` with columns `Month`, `Revenue`, `Cost` and two rows
- **WHEN** `Sales[; 1..2].matrix()` is evaluated
- **THEN** the result is a 2 × 2 matrix of the `Revenue` and `Cost` values

#### Scenario: A column that is not numeric
- **WHEN** `Sales.matrix()` is checked
- **THEN** an error says column `Month` is Text

### Requirement: Matrix product
OMX SHALL provide the infix operator `@` for the matrix product, with the
precedence of `*` and grouping to the left. For an m × n matrix and an n × p
matrix the result SHALL be the m × p matrix whose element at row i, column j is
the sum over k of the products of the left operand's element (i, k) and the
right operand's element (k, j). A vector as the right operand SHALL be taken as
a column and a vector as the left operand as a row, the result being a vector;
two vectors of equal length SHALL give their dot product, a scalar. Operands
whose inner dimensions differ SHALL be an error that states both shapes. `*`
SHALL remain element-wise.

#### Scenario: Two matrices
- **WHEN** `[[1, 2], [3, 4]] @ [[5, 6], [7, 8]]` is evaluated
- **THEN** the result is `[[19, 22], [43, 50]]`

#### Scenario: Matrix and vector
- **WHEN** `[[1, 2], [3, 4]] @ [1, 1]` and `[1, 1] @ [[1, 2], [3, 4]]` are evaluated
- **THEN** the results are `[3, 7]` and `[4, 6]`

#### Scenario: Dot product
- **WHEN** `[1, 2, 3] @ [4, 5, 6]` is evaluated
- **THEN** the result is the scalar `32`

#### Scenario: Weighted total from a table
- **GIVEN** `Scores` with only the numeric columns `Exam` and `Essay`, and `const Weights = [60%, 40%]`
- **WHEN** a constant is defined as `Scores @ Weights`
- **THEN** it is a vector with one weighted score for each row of `Scores`

#### Scenario: Not the element-wise product
- **WHEN** `[[1, 2], [3, 4]] * [[5, 6], [7, 8]]` is evaluated
- **THEN** the result is `[[5, 12], [21, 32]]`

#### Scenario: Shapes that do not fit
- **WHEN** a 2 × 3 matrix is multiplied with `@` by a 2 × 2 matrix
- **THEN** an error says a 2 × 3 matrix cannot be multiplied by a 2 × 2 matrix

### Requirement: Transpose
OMX SHALL provide `transpose`, usable as `transpose(m)` or `m.transpose()`,
giving the matrix with rows and columns exchanged, and SHALL list it in the
function directory.

#### Scenario: Transpose
- **WHEN** `[[1, 2, 3], [4, 5, 6]].transpose()` is evaluated
- **THEN** the result is `[[1, 4], [2, 5], [3, 6]]`

## MODIFIED Requirements

### Requirement: Broadcasting
Arithmetic and comparison operators SHALL apply element-wise between a vector and
a scalar and between two vectors of equal length, and likewise between a matrix
and a scalar and between two matrices of the same shape. A comparison over a
vector SHALL yield a boolean vector usable as a row predicate. Operating on
vectors of unequal length, or on matrices of different shapes, SHALL be an error.

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

#### Scenario: Matrix and scalar
- **WHEN** `[[1, 2], [3, 4]] * 2` is evaluated
- **THEN** the result is `[[2, 4], [6, 8]]`
