# Proposal: Cast to Float for the trigonometric, exponential and logarithmic functions

## Why

`sin(1)` and `exp(1/2)` work, because every number type already widens to a `Float`. But `sin(true)`, `sin('1')` and `ln('20')` are errors (`sin cannot be applied to Boolean`), though `true.Float` is `1` and `'1.5'.Float` is `1.5`. A column read from a CSV as text, or a flag column, needs `.Float` written out before it can go into `sin`. The functions already know how to produce a `Float`; they should take anything that converts to one, by the same rule `.Float` uses.

## What Changes

- **omx-expressions** — MODIFIED: `sin`, `cos`, `tan`, the inverse and hyperbolic forms, `exp`, `ln`, `log10` and `log2` SHALL cast their argument to `Float` first, accepting whatever `x.Float` accepts: any number type, a `Boolean`, text that reads as a number, and a `Complex` whose imaginary part is zero to within `1e-15`
- A value `x.Float` refuses (a `Date`, `Time`, `Datetime`, `Duration`, or a `Complex` with an imaginary part) is still an error that names the function
- `exp` and `ln` of a `Complex` stay `Complex`, as now: the cast applies to what is not a `Complex`

## Impact

- The function dispatch in `omasheet-engine` (`value.rs`) and the type check in `omasheet-omx` (`check.rs`), which both say `cannot be applied to`
- `docs/expressions.md`: the Maths line and the Complex section
- A column of these calls over text or `Boolean` cells is a `Float` column, as it is for numbers now

## Out of Scope

- `sqrt`, `abs`, `round`, `floor` and `ceil`. They keep their present argument types; casting them is a separate decision, since `round('4.5')` and `abs(true)` change what a column of text means
- Dates and durations. `Duration.total_seconds()` and the other `total_*` functions already give a number
- Any other conversion on cells, such as reading `'1,5'` as `1.5`
