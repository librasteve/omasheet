# omx-expressions

## MODIFIED Requirements

### Requirement: Inexact numbers are shown without the noise of their arithmetic

A `Float`, a quantity with no uncertainty, and each part of a `Complex` SHALL be shown rounded so that digits beyond the fifteenth significant one, which are the noise of f64 arithmetic, are not shown, at every magnitude, and marked with `·` where that changed the number. The value and a file keep every digit.

#### Scenario: A large converted quantity

- **GIVEN** `(2g).to('mg')`, which is `2000.0000000000002mg` in f64
- **WHEN** it is shown in the grid
- **THEN** it is `2000mg·`, not `2000.000mg·`

#### Scenario: A small float

- **GIVEN** `sin(pi)`
- **WHEN** it is shown
- **THEN** it is `0·`, as before
