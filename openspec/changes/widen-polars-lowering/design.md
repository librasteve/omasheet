# Design: widen what lowers to Polars

Forthcoming. D numbers are assigned when the change starts; `D-W#` until then.

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D-W1 | Proposed | A column lowers only if every operand's type is static: a declared column type, or the type of a literal, a constant or a function result | The dress of a result (`Percent`, a unit, an uncertainty) is the checker's, taken once; Polars does the numbers |
| D-W2 | Proposed | A column with no declared type, a `Fraction`, and anything typed `Any` are never lowered | An inferred column takes its dress from its values (D69), which a lowered plan has not seen. `Fraction` is arbitrary-precision and is not a `Decimal(38, s)`. `Any` has no type to choose a lowering by |
| D-W3 | Proposed | `Lit::Percent` and a percent `const` lower as a `Decimal` holding the number (`20%` is `0.2`), in a declared column | `Value::Percent(d)` already holds the plain number and `exact_arith` works on it; `wider` gives `Percent` for two percentages and `Decimal` for any other mix, as `value.rs` does at run time |
| D-W4 | Proposed | `Quantity op Quantity` lowers: the checker gives the result's dimension and unit once, and the right side is multiplied by the constant factor into the left's unit | `5kg + 300g` is `5.3kg`; a product or quotient combines the units; a dimension that does not match is a static error. A column holds its cells in its own unit, so there is no per-row unit to choose |
| D-W5 | Proposed | `Scientific op Int \| Decimal \| Float \| Scientific` lowers as a value expression and an uncertainty expression, both `Float64`, written with the engine's formulas operation by operation | The Parquet form is already a struct of value and uncertainty. An aggregate that adds floats stays declined (non-associative); the operations here are elementwise |
| D-W6 | Proposed | `apply(Fn)` (`add-group-apply`) lowers when the body of `Fn` lowers under D-W1 to D-W5, and otherwise runs per group in the engine | Replaces that change's D-A6 ("`apply` cannot lower"). The two paths must agree on shape, type and error, so its task 3.2 gains the lowered path |
| D-W7 | Proposed | No `map_elements`/`map_groups` bridge for anything that does not lower | It would be the engine's per-row evaluation inside Polars: the same cost, no parallelism, no optimisation, and a conversion each way |

## Quantity and Scientific with `Any`

`Quantity op Quantity` lowers because the unit work is a meta-operation: the checker works out the result's dimension and unit once, and the factor between two units of one dimension is a constant, so Polars only has to multiply, add or divide the numbers, as it already does for `Decimal`. Offset units (`°C`) need an affine step and stay on the engine until the engine's rule for them is checked. A `Scientific` is a value with an uncertainty, which the Parquet form already keeps as separate fields, so `Scientific op` a number or another `Scientific` can lower as two `Float64` expressions, the value and the propagated uncertainty, whose formulas must match the engine's operation by operation. `Scientific op Any` is different: `Any` has no static type, so the checker cannot choose a lowering, and mapping every uncertain form of every operation onto `apply`-style per-row callbacks would only reproduce the engine's per-row evaluation inside Polars, losing parallelism and optimisation and adding conversion cost for no speed. **Recommendation:** lower `Quantity` and `Scientific` operations when both operand types are static and the unit rules are simple; leave anything involving `Any` on the engine; do not build a per-row bridge (D-W7).

## Open

- The engine's uncertainty-propagation formulas, and whether each is elementwise and exact in `Float64` against what the engine computes: read before task 3.1.
- How the engine treats an offset unit and a `Quantity` that carries an uncertainty (a quantity column is a struct of value, uncertainty and unit).
- Whether a `const` is inlined as `Lit::Percent` by the checker or read as a named value, which decides where D-W3 is applied; `examples/budget.omx` (`Tax := Profit * TaxRate`) is the first test.
- Whether a declared `Percent` column that mixes a bare number with percentages (D69a: "still declined") is the same case as an inferred column.

## Risks

- A formula that differs from the engine's in the last digit: every case is gated by an agreement test, and a lowered column is not believed where the two disagree.
- A column whose declared type is wrong for its values: the checker catches it where the column is read in, and the engine calculates it as it does now.
