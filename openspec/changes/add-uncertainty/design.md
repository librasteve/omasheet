# Design: Uncertainty

## Context

Source: `coinvent-omasheet-format-chatgpt.md`, cut down with the owner (2026-11-09) to a practical first slice for scientific and engineering use. D/P/Q numbers continue the numbering of `add-omasheet-core`.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D8 | Decided | Uncertainty is in scope | Raised by owner |
| D90 | Decided | An uncertainty is stored as a non-negative `f64` standard deviation (1σ), beside an `f64` value | Settles Q5. Quadrature needs a square root, which leaves the rationals; uncertainties are estimates, so exactness is not kept. Variance is not stored |
| D91 | Decided | `Scientific` is more infectious than `Float`; any operation with an uncertain operand gives a `Scientific`, and an exact operand counts as σ = 0 | The type lattice gains one rung |
| D92 | Decided | Literals are `10 ± 0.1` and the concise `10.5(3)` | Both give the same value. The digits in brackets align with the last digit of the number, and an exponent scales both |
| D93 | Decided | Independent uncertainties combine in quadrature; operands are always treated as independent | Alternative seen: linear (worst-case) sum. `x - x` is therefore `0 ± √2σ`, a known limitation |
| D94 | Decided | A function of an uncertain value is propagated to first order: `f(x ± σ) = f(x) ± |f′(x)|·σ`; `x ** y` and two-argument functions use the partial derivatives | Covers every `MathFn`. Close to a domain edge (`ln(0.1 ± 0.5)`) the linear answer is poor; this is documented, not warned about |
| D95 | Decided | Where an operation gives a `Complex` from an uncertain operand (a `Complex` beside it, `Complex(re, im)`, `x.Complex`), the uncertain value degrades to a `Float` and the message `uncertain complex not supported` is shown. A real function outside its domain is `NaN` as for a `Float`, not a `Complex`. Uncertain complex is out of scope, now and later | A correct version needs a covariance per value; a shortcut would give wrong numbers. The degraded result carries no uncertainty |
| D96 | Decided | The default display is the concise `(nn)` form, with σ to two significant digits, always; a column format option selects `±` | No fallback to `±` for a large σ. Display never changes the stored value |
| D97 | Decided | `sum`, `mean`, `product` and an even `median` propagate, being arithmetic; `min`, `max`, `first`, `last` and `quantile` return the chosen element with its σ; `std` and `var` work on the values and are `Float` | Aggregation is the arithmetic that already propagates, with no code of its own. Comparison and sorting use the values only |
| D98 | Decided | The degrade message is a warning in the bottom bar, in amber, while the cell is selected | The cell still has a value, so red (error) is wrong. The theme gains a warning colour, next to `error`. Diagnostics have only the error shape, so the engine records the cell and the snapshot carries `w` for it |
| D99 | Decided | `±` binds more tightly than every other operator, `**` included; each side is a single term | `10 ± 0.1 ** 2` is `(10 ± 0.1) ** 2`. A negative uncertainty is written in brackets and is an error at evaluation |
| D100 | Decided | A `Scientific` column's format is a number format as a `Float` takes, `[±][flags][width][.N][f\|e\|g][K]`, with `±` before it for `value ± uncertainty` | `.Nf` puts the uncertainty at that place for every cell so the brackets stand in line, `eK` gives every cell one exponent, `e` its own, `g` whichever reads better. A sign, width and grouping are as for a `Float`; with `±` and an exponent the exponent is factored out, `(1.21 ± 0.03)e-2`. What is not named is worked out per cell. Nothing is worked out across a column. The first form, `[±][.N][eK]`, was a grammar of its own and is replaced |

## Open Questions

None for this slice. The old Q7 (range versus interval syntax) is out of scope: intervals need their own syntax.
