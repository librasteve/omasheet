# Design: Complex results from the maths functions

D numbers continue from `restore-complex` (D206).

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D207 | Decided | A real argument outside the real domain gives the principal `Complex`; the static type is unchanged | Owner, 2026-11-10. The column's type is the join of its cells when undeclared (D191), so it widens to `Complex`; declared `Float`, the cell is an error by D203. The checker cannot know the data, so it types `sqrt(Float)` as `Float` and a downstream function that refuses a `Complex` reports the cell error |
| D208 | Decided | The principal branch follows C99 (`csqrt`, `casin`, `cacosh`, `catanh`): a real argument is `x + 0i`, so the cut is approached from above for `x > 1` | `arcsin(2)` is `1.5707963267948966+1.3169578969248166i`, as Python's `cmath` and Raku's `asin(2+0i)` give |
| D209 | Decided | Domain boundaries stay real: `ln(0)` is `-Inf`, `sqrt(0)` is `0`, `arctanh(±1)` is `±Inf`, `arcsin(±1)` is `±π/2`. `NaN` in is `NaN` out | A boundary is not outside the domain |
| D210 | Decided | `sign` of a `Complex` is `z/|z|`; `round floor ceil radians degrees` refuse a `Complex` | `sign` has a natural unit-vector meaning; the others do not |
| D211 | Decided | `arcsinh arccosh arctanh` are added for real and `Complex` | They are the functions the table asks for and were missing |
| D212 | Decided | `x ** y` with `x < 0` and a non-whole finite `y` is `exp(y ln x)`, a `Complex` | Raku has `(-2) ** 0.5` as `1.414i` |
| D213 | Decided | `Scientific` stays real: the result of a function outside its domain is `NaN ± NaN` | There is no uncertain complex (existing rule) |

## Risks

- A sheet that relied on `sqrt(negative)` being `NaN` now gets a `Complex`, which then fails a downstream `floor`, comparison or `Float` column. This is the point of the change, and is stated in `docs/expressions.md`.
- Precision near cuts and for large imaginary parts: the formulas are the standard stable ones, tested against `cmath` values.
