# evaluation

Checking and exactness of the matrix product, inverse and determinant.

## ADDED Requirements

### Requirement: Matrix shapes are checked before execution
Where the shapes of both operands of `@` are known from the source — literals,
constants, and tables with a fixed number of rows — a mismatch SHALL be reported
by `lint` and SHALL block execution. Where a shape depends on the data, such as
a filtered table, the mismatch SHALL be reported when the product is evaluated.

#### Scenario: Known shapes
- **WHEN** `lint` checks a sheet containing `[[1, 2, 3]] @ [[1, 2]]`
- **THEN** a shape error is reported and nothing is executed

#### Scenario: A matrix that is not square
- **WHEN** `lint` checks a sheet containing `inverse([[1, 2, 3], [4, 5, 6]])`
- **THEN** a shape error is reported and nothing is executed

#### Scenario: Shape known only from the data
- **GIVEN** `const W = [1, 2, 3]`
- **WHEN** `Sales[Revenue > 100; 1..2] @ W` is evaluated
- **THEN** an error at the product states the two shapes

### Requirement: The matrix product preserves exactness
A product of matrices or vectors of `Int` and `Ratio` elements SHALL be exact.
An element of type `Num` in either operand SHALL make the result `Num`, and one
of type `Complex` SHALL make it `Complex`.

#### Scenario: Ratio elements
- **WHEN** `[[1/3, 1/6]] @ [[3], [6]]` is evaluated
- **THEN** the result is the 1 × 1 matrix holding exactly `2`

### Requirement: The inverse and determinant preserve exactness
The determinant of a matrix of `Int` elements SHALL be an `Int`, and of `Int`
and `Ratio` elements an exact `Ratio`. The inverse of a matrix of `Int`
and `Ratio` elements SHALL be exact, so that its product with the matrix is
exactly the identity, and such a matrix SHALL be singular only when its
determinant is exactly `0`. An element of type `Num` SHALL make either result
`Num`, and one of type `Complex` SHALL make it `Complex`.

#### Scenario: Exact round trip
- **GIVEN** `const M = [[1/3, 1/7], [2, 5]]`
- **WHEN** `M @ M.inverse()` is evaluated
- **THEN** the result is exactly `[[1, 0], [0, 1]]`

#### Scenario: Where floating point fails
- **WHEN** `det([[1/10, 2/10], [3/10, 6/10]])` is evaluated
- **THEN** the result is exactly `0`
