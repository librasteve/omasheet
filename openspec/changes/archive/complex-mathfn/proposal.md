# Proposal: Complex results from the maths functions

## Why

`restore-complex` gave `Complex` a few functions (`abs sqrt exp ln` and the part functions). The rest of the maths functions refuse a `Complex`, and a real number outside a function's domain is `NaN`, so `sqrt(-4)` has no answer in a sheet. Mathematically these all have a principal complex value: `sqrt(-4)` is `2i`, `ln(-1)` is `πi`, `arccos(2)` is `-1.317i`, `(-2) ** 0.5` is `1.414i`. The owner asked for them (2026-11-10).

## What Changes

- **omx-expressions**: MODIFIED. `log10 log2 sin cos tan arcsin arccos arctan sinh cosh tanh sign` accept a `Complex` and give a `Complex` (principal branch, C99 cuts). ADDED `arcsinh arccosh arctanh`, for a real or a `Complex`.
- **omx-expressions**: MODIFIED. A real argument outside the real domain gives the principal `Complex` instead of `NaN`: `sqrt ln log10 log2` of a negative number, `arcsin arccos arctanh` beyond `[-1, 1]`, `arccosh` below 1. The boundary values stay real (`ln(0)` is `-Inf`, `arctanh(1)` is `Inf`). A `NaN` stays `NaN`.
- **numerics**: MODIFIED. A negative real base to a non-whole real exponent gives a `Complex`: `(-2) ** 0.5`.
- **numerics**: The type checker keeps the static type of these functions as it is (`Float` for a `Float`); the `Complex` appears in the **value**. An undeclared column holding one widens to `Complex` by the join (D191); a column declared `Float` (or any real type) rejects the cell with the conversion error of D203 (`3+4i` cannot be a `Float`). `x.Float` of a non-real result is the same error (D201).
- `round floor ceil radians degrees` still refuse a `Complex`.
- An uncertain number (`Scientific`) is unchanged: there is no uncertain complex, so `sqrt(-4 ± 0.4)` stays `NaN ± NaN`.

## Impact

- `omasheet-omx`: `MathFn` (new variants, `of_complex`, `slope`, `apply`, the directory entries); `check.rs` needs no change to its typing
- `omasheet-engine`: `value.rs` (`math`, `complex_math`, real `**`)
- `omasheet-interop`: `xlsx/write.rs` (new functions map to `ASINH`, `ACOSH`, `ATANH`; a complex result is already written as its value and reported)
- `docs/expressions.md`, `skill/omasheet/SKILL.md`, `README.md`, `examples/functions.omx` and its `.expected`
- Tests that expect `sqrt(-1)` to be `NaN` change

## Out of Scope

- A value-dependent static type: the checker does not know `sqrt(x)` may be `Complex`
- Uncertain complex numbers; Polars pushdown of `Complex`
- Branch-selecting arguments (a second argument choosing the branch)
