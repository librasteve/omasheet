# Tasks: Complex results from the maths functions

## 1. Functions (`omx-expressions`)
- [x] 1.1 `MathFn::{Arcsinh, Arccosh, Arctanh}`: directory entries, `apply`, `slope`, `casts_to_float`, `of_complex`, xlsx names
- [x] 1.2 `complex_math`: `log10 log2 sin cos tan arcsin arccos arctan sinh cosh tanh arcsinh arccosh arctanh sign`, C99 branches (D208, D210)
- [x] 1.3 `math`: a real argument outside the domain goes through `complex_math` (D207, D209)
- [x] 1.4 `**` of a negative base and non-whole exponent (D212)

## 2. Checks
- [x] 2.1 Tests against `cmath` values, identities, branch cuts, boundaries, `NaN`, `Scientific` (D213), declared and undeclared columns
- [x] 2.2 Existing tests that expected `NaN` updated
- [x] 2.3 fmt, clippy -D warnings, workspace tests

## 3. Docs and examples
- [x] 3.1 `docs/expressions.md`, `skill/omasheet/SKILL.md`, `README.md`
- [x] 3.2 `examples/types.omx` Limits sheet shows `sqrt(-1)` and `ln(-1)` (`.expected` regenerated); `functions.omx` is user functions and needs nothing
