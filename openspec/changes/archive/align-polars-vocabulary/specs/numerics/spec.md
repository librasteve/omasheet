# numerics

The order of a `NaN`, which is now Polars' order and not Raku's.

## ADDED Requirements

### Requirement: The order of a NaN

A `NaN` SHALL be ordered greater than every other value of its type, SHALL
sort last, and SHALL be equal to itself, as Polars orders it. This replaces
the sentence of the `Exact arithmetic` requirement that a `NaN` is not equal
to, less than or more than any value, itself included, which was Raku's rule.

`Inf` and `-Inf` SHALL be unchanged: the arithmetic and the order they already
have are the ones Polars has.

The aggregates `min` and `max` over a column holding a `NaN` SHALL answer the
`NaN`, which for `max` follows from a `NaN` being greatest and for `min` SHALL
be the specified behaviour of the Polars kernel the engine is configured with.

#### Scenario: A NaN is equal to itself

- **WHEN** `NaN == NaN`, `NaN != NaN`, `NaN < 1` and `1 <= NaN` are evaluated
- **THEN** the results are `true`, `false`, `false` and `true`

#### Scenario: A NaN is greatest

- **WHEN** `[1, NaN, 3].max()` and `[1, NaN, 3].min()` are evaluated
- **THEN** the results are `NaN` and `1`

#### Scenario: The infinities are unchanged

- **WHEN** `1/0`, `-1/0`, `0/0`, `Inf - Inf`, `1 / Inf` and `-Inf < 1` are
  evaluated
- **THEN** the results are `Inf`, `-Inf`, `NaN`, `NaN`, `0` and `true`

#### Scenario: Both paths agree

- **GIVEN** a column of `Float` values one of which is a `NaN`
- **WHEN** it is aggregated by the engine and by the dataframe
- **THEN** the two answers are the same
