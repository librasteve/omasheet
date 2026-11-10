# omx-expressions

The expression language: its literals, its operators and its functions.

## ADDED Requirements

### Requirement: Imaginary literals

A number immediately followed by `i` SHALL be an imaginary `Complex`, so that `3+4i` is the sum of `3` and `4i`, a `Complex` with real part 3 and imaginary part 4. `Inf\i` and `NaN\i` SHALL be the imaginary infinity and not-a-number, as in Raku. A cell holding `3+4i` with no `=` SHALL be a `Complex` literal.

#### Scenario: An imaginary literal

- **GIVEN** the expression `3+4i`
- **WHEN** it is evaluated
- **THEN** the result is the `Complex` with real part 3 and imaginary part 4

### Requirement: Complex functions

`re`, `im`, `conj` and `arg` SHALL give the real part, imaginary part, conjugate and angle (in radians, from `-pi` to `pi`) of a `Complex`; given a real number, `re` and `conj` SHALL give it back, `im` SHALL give zero, and `arg` SHALL give `0` or `pi`. `abs`, `sqrt`, `exp` and `ln` SHALL accept a `Complex`, `sqrt` giving the root with a real part that is not negative, and `ln` the principal value. Every other function SHALL refuse a `Complex` with an error naming the function.

`x.Complex` and `Complex(re, im)` SHALL convert: the first from a real number, with no imaginary part, and the second from two.

#### Scenario: Parts

- **GIVEN** `re(3+4i)`, `im(3+4i)`, `abs(3+4i)` and `conj(3+4i)`
- **WHEN** each is evaluated
- **THEN** the results are `3`, `4`, `5` and `3-4i`

#### Scenario: The square root of minus one

- **GIVEN** the expression `sqrt(-1 + 0i)`
- **WHEN** it is evaluated
- **THEN** the result is `0+1i`

#### Scenario: A function with no complex form

- **GIVEN** the expression `floor(1.5+2i)`
- **WHEN** it is checked
- **THEN** it is an error saying `floor` is not defined for `Complex`

#### Scenario: Converting

- **GIVEN** `2.Complex` and `Complex(1, -1)`
- **WHEN** each is evaluated
- **THEN** the results are `2+0i` and `1-1i`
