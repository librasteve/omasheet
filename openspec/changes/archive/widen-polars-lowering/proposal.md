# Proposal: widen what lowers to Polars

**Forthcoming; not scheduled.** It sits beside `polars-function-registry` and `add-group-apply`, whose `apply` lowers where this change lets its body lower (D-W6).

## Why

`omasheet-engine/src/lower.rs` turns an expression into a Polars one, or declines and leaves the column to the engine. The header of that file lists what it declines, and part of the list is stale since the Decimal redesign: exact `+ - *`, small whole powers, quotients up to 21 places, and `sum`/`mean` of exact numbers already lower. What is still declined is declined for three different reasons, and only one of them is real:

- a **percent literal or constant** (`Revenue * 5%`, `Profit * TaxRate`), though a `Percent` column already lowers as the `Decimal` it is (D69a), and the result's dress follows from the static types by `S::wider`;
- a **`Quantity`**, though the unit of the result is worked out once by the checker and the scale factor between two units of one dimension is a constant;
- a **`Scientific`**, though a value with an uncertainty is two numbers, and each has a formula.

A column whose type is **inferred from its values**, a `Fraction`, and anything typed `Any` have no static type to lower from, and stay on the engine (D-W1, D-W2).

## What Changes

- **omx-expressions**: ADDED. A column lowers only if the type of every operand is known statically, from a declared column type or the type of a literal or constant. A column with no declared type never lowers, and neither does a `Fraction`.
- **omx-expressions**: ADDED. `Lit::Percent` and a percent `const` lower as a plain `Decimal`, where the column they are written into is declared. The dress of the result is the checker's, by `S::wider`: two percentages a percentage, any other mix the plain number.
- **omx-expressions**: ADDED. `Quantity op Quantity` (and `Quantity op number`) lowers, the result's dimension and unit taken once from the checker and the right side brought to the left's unit by a constant factor.
- **omx-expressions**: ADDED. `Scientific op Int | Decimal | Float | Scientific` lowers as two `Float64` expressions, the value and the propagated uncertainty, with the engine's own formulas.
- **omasheet-engine**: the module header of `lower.rs` is corrected to say what is declined and why.
- Every case is gated by a test that the lowered answer equals the engine's; the engine stays the oracle (D66).

## Impact

- `omasheet-engine`: `lower.rs`, `dtype.rs` (a quantity and a scientific column held as value fields), `eval.rs` (the agreement tests)
- `omasheet-omx`: `check.rs` (the result type and unit of a lowered operation, handed to the lowerer)
- `docs/`: a short note in `docs/expressions.md` on which columns are calculated in Polars and which are not

## Out of Scope

- Any operand typed `Any`, and a column whose type is inferred: no static type, so no lowering (a per-row callback would be the engine's own evaluation inside Polars, with none of the speed)
- `Fraction`: arbitrary-precision, not a `Decimal(38, s)`
- Offset units (`°C`, `°F`) until the engine's rule for them is read and tested
- Sums, products and means of floating-point numbers, which are declined for non-associativity (`aggregate`, D72)
- `Complex`
