# numerics

What a number is, and what arithmetic on one gives.

## ADDED Requirements

### Requirement: The Complex type

`Complex` SHALL be a number with a real and an imaginary part, each a `Float`. In the type lattice it SHALL be above `Float`: arithmetic with a `Complex` on either side SHALL give a `Complex`, and a `Complex` with text SHALL be `Any`.

`+`, `-`, `*` and `/` SHALL follow the rules of complex numbers, and `z ** w` SHALL be `exp(w * ln z)`, with `0 ** w` the zero `Complex`. A `Complex` SHALL be equal to another when both parts are, and SHALL have no order: `<`, `>`, `min`, `max` and a sort SHALL report an error naming the type.

#### Scenario: Complex arithmetic

- **GIVEN** the expression `(1+2i) * (3-1i)`
- **WHEN** it is evaluated
- **THEN** the result is the `Complex` `5+5i`

#### Scenario: A Complex infects

- **GIVEN** the expressions `1/3 + 1i` and `0.5 + 2`
- **WHEN** each is evaluated
- **THEN** the first is a `Complex`, and the second is a `Decimal`

#### Scenario: A Complex has no order

- **GIVEN** the expression `1i < 2i`
- **WHEN** it is evaluated
- **THEN** it is an error saying a `Complex` has no order

### Requirement: Complex display

A `Complex` SHALL be shown as its real part, the sign of its imaginary part, the magnitude of that, and `i`, each part shown as a `Float`. A non-finite imaginary part SHALL be followed by `\i`, as in Raku. Where the whole is wider than 23 characters, both parts SHALL be cut equally, each marked `…`, until it fits. A sign SHALL NOT be counted in that width. In the app the digits SHALL be grouped and localised as any other number is.

#### Scenario: A Complex that fits

- **GIVEN** `3+4i`, `1.5-2i` and `1i`
- **WHEN** they are displayed
- **THEN** they are shown as `3+4i`, `1.5-2i` and `0+1i`

#### Scenario: A Complex too wide for its parts

- **GIVEN** `(1/3).Float + (1/3).Float * 1i`
- **WHEN** it is displayed
- **THEN** it is shown as `0.3333333333333333+0.3333333333333333i` where that fits in 23 characters, and otherwise with both parts cut and marked

#### Scenario: A non-finite part

- **GIVEN** a `Complex` with real part 0 and imaginary part infinity
- **WHEN** it is displayed
- **THEN** it is shown as `0+Inf\i`

### Requirement: Complex in the lattice

The types SHALL sit in the chain `Integer` < `Decimal` < `Float` < `Complex`. The join of `Complex` with any number, including a `Hex`, `Octal`, `Binary`, `Fraction` or `Percent`, SHALL be `Complex`, and with text SHALL be `Any`. Arithmetic SHALL return the join of its two sides.

#### Scenario: The join with Complex

- **GIVEN** an undeclared column holding `1`, `2.5` and `3+4i`
- **WHEN** its type is worked out
- **THEN** it is `Complex`

#### Scenario: A family member joins as its parent would

- **GIVEN** the expression `0x10 + 1i`
- **WHEN** it is evaluated
- **THEN** the result is the `Complex` `16+1i`

### Requirement: Converting a Complex to a real number

A `Complex` converted to `Float`, `Decimal`, `Integer`, `Fraction` or `Percent` SHALL give its real part, converted, when its imaginary part is within `1e-15` of zero, and SHALL otherwise be an error saying that the imaginary part is not zero. The tolerance on the imaginary part SHALL be absolute, not relative to the real part, and SHALL be used nowhere else: `im(z) = 0` is the exact test.

#### Scenario: A zero imaginary part

- **GIVEN** `(3+0i).Integer` and `(3+1e-16i).Float`
- **WHEN** each is evaluated
- **THEN** the results are the `Integer` `3` and the `Float` `3`

#### Scenario: A non-zero imaginary part

- **GIVEN** `(3+4i).Float` and `(3+1e-15i).Float`
- **WHEN** each is evaluated
- **THEN** each is an error saying that `3+4i`, or `3+1e-15i`, cannot be converted to `Float` because its imaginary part is not zero

### Requirement: Comparison stays exact

`Float` values SHALL be compared exactly, as before, and two `Complex` values SHALL be equal when both of their parts are exactly equal. No tolerance SHALL be used in a comparison. The only use of tolerance is the test of the imaginary part in converting a `Complex` to a real type.

#### Scenario: Rounding error is a difference

- **GIVEN** the expression `0.1e0 + 0.2e0 = 0.3e0`
- **WHEN** it is evaluated
- **THEN** the result is `false`, as it is today

#### Scenario: Complex equality is exact

- **GIVEN** `3+4i = 3+4i` and `3+4i = 3+4.000000000000001i`
- **WHEN** each is evaluated
- **THEN** the results are `true` and `false`

## MODIFIED Requirements

### Requirement: An omasheet type names a Polars dtype

Each type SHALL be a name for one Polars dtype, as before, **except `Complex`**, which SHALL be a name for the pair of `Float64`s of a `Struct{re, im}`: held by the engine as its own value, and stored in Parquet as that struct. See `parquet-interop`.

#### Scenario: Complex is not a dtype of its own

- **GIVEN** the type table
- **WHEN** `Complex` is looked up
- **THEN** its dtype is `Struct{re: Float64, im: Float64}`
