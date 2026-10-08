# evaluation

Checking and exactness of the matrix product.

## ADDED Requirements

### Requirement: Matrix shapes are checked before execution
Where the shapes of both operands of `@` are known from the source — literals,
constants, and tables with a fixed number of rows — a mismatch SHALL be reported
by `lint` and SHALL block execution. Where a shape depends on the data, such as
a filtered table, the mismatch SHALL be reported when the product is evaluated.

#### Scenario: Known shapes
- **WHEN** `lint` checks a sheet containing `[[1, 2, 3]] @ [[1, 2]]`
- **THEN** a shape error is reported and nothing is executed

#### Scenario: Shape known only from the data
- **GIVEN** `const W = [1, 2, 3]`
- **WHEN** `Sales[Revenue > 100; 1..2] @ W` is evaluated
- **THEN** an error at the product states the two shapes

### Requirement: The matrix product preserves exactness
A product of matrices or vectors of `Int` and `Rational` elements SHALL be exact.
An element of type `Num` in either operand SHALL make the result `Num`, and one
of type `Complex` SHALL make it `Complex`.

#### Scenario: Rational elements
- **WHEN** `[[1/3, 1/6]] @ [[3], [6]]` is evaluated
- **THEN** the result is the 1 × 1 matrix holding exactly `2`
