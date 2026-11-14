# Design: `Decimal(s)` and `Fraction`

Decisions are numbered T1 onward. They reverse parts of `add-polars-dtypes` D15 (decimal scale wider than Polars'), D17 (the `Fraction` type goes) and D25 (a number past 38 digits moves the point), and restore what D14 removed (`num-bigint`, `num-rational`) for one type.

## Why one global scale cannot work

A `Decimal128` has 38 digits to share between the two sides of the point.

| Scale | Digits before the point | Largest | Smallest step |
|---|---|---|---|
| 0 | 38 | ~10³⁸ | 1 |
| 10 | 28 | ~10²⁸ | 10⁻¹⁰ |
| 18 | 20 | ~10²⁰ | 10⁻¹⁸ |
| 30 | 8 | ~10⁸ | 10⁻³⁰ |

A column holding a googol and `1e-30` has no scale. So the scale is the column's to choose, and what does not fit is `Fraction`.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| T1 | Proposed | `Decimal` is `Decimal(38, s)`. The scale `s`, 0 to 38, belongs to the column and is written `Decimal(s)`; bare `Decimal` is `Decimal(10)` | Owner, 2026-11-14: `Decimal(38, 10)`. It holds ~10²⁸, so a million trillion (10¹⁸) fits, and ten places. The syntax follows `Datetime("Europe/London")`. Brackets stay for display formats |
| T2 | Proposed | A `Decimal(s)` value has exactly the column's scale. The point does not move | Replaces D25's "past 38 digits it moves the point" |
| T3 | Proposed | The result of an operation is rounded to the scale it is given by T5, half away from zero | The rule `round` already has (D54). Ties are away from zero, not to even |
| T4 | Proposed | A result that needs more than 38 digits at its scale is an error cell: "does not fit Decimal(s); use Fraction or a smaller scale" | Like an integer overflow. It is never silently widened, so the type of a formula is static |
| T5 | Proposed | `Decimal(a) ± Decimal(b)` and `*` are `Decimal(max(a, b))`. `/` and `mean` take the left operand's scale. `Integer` counts as `Decimal(0)`; `Percent` is a `Decimal` | `Integer / Integer` is `Decimal(10)`, not `Fraction`, so ordinary formulas stay lowerable |
| T6 | Proposed | `Fraction` is an exact rational of arbitrary-precision integers (`num-bigint`, `num-rational`): reduced, denominator positive, equality is structural | Raku's `FatRat`, with no degrade to a float. Backing is `BigRational` first |
| T7 | Proposed | Lattice: `Integer` < `Decimal(s)` < `Fraction`; `Decimal(a)` join `Decimal(b)` is `Decimal(max)`; `Fraction` join `Float` is `Float`; `Scientific`, `Quantity`, `Complex` as now | The join of a `Fraction` with a `Decimal` is exact, with a `Float` it is the float |
| T8 | Proposed | `+ - * /` on a `Fraction` are exact. `sqrt`, `ln`, `exp`, the trigonometry and a fractional power give `Float` | They have no rational result. A whole power of a `Fraction` is exact |
| T9 | Decided | `Fraction` is shown as a fraction (`1/3`, `2 3/4`) by default, and as a decimal cut with the `·` mark under a format (owner, 2026-11-14); the exact value is the footer's | Reuses the existing "cut" mark |
| T10 | Proposed | A declared column refuses a value that has more places than its scale, per cell, naming the type that would hold it. It never rounds on assignment | As a declared `Float` refuses a `Complex` |
| T11 | Proposed | An undeclared column is inferred as the smallest scale (at most 18) that holds every cell, else `Fraction` | So an existing file with long decimals still loads exactly |
| T12 | Proposed | Parquet: `Decimal(p, s)` is read at its scale `s`; `Decimal(n)` is written `Decimal(38, n)`; `Decimal256` is refused | One column scale on both sides of the boundary |
| T13 | Proposed | Parquet: a `Fraction` column is a struct `{num: string, den: string}` with `omasheet.kind.<column> = Fraction`; `--decimal` writes the nearest `Decimal(38, s)` or `Float64` and reports what was cut | Strings need no fixed-width integer. Precedent: `Complex` as `{re, im}`, `Quantity` as `{value, sigma}` |
| T14 | Proposed | CSV and JSON write `n/d` for a `Fraction`; XLSX writes the rounded double and says so | |
| T15 | Proposed | Everything lowers except `Fraction` and a decimal power other than a small whole one (T19). Mixed scales are casts to the common scale (exact), then the operation, then an explicit round for `*` and `/` | See "Lowering" |
| T16 | Proposed | The per-row overflow guard stays. A row that might not fit sends the column to the engine, whose `Fraction` arithmetic gives the exact result, which is then rounded or is the T4 error | The two paths agree on the value, so the guard is a speed matter only |
| T17 | Proposed | `Dec` stays as the engine's carrier of one `Decimal(s)` value (an `i128` and a scale) with no point-moving | Keeps the arithmetic code; removes the 30-digit quotient and the guard digits |
| T19 | Proposed | `x ** n` of a `Decimal(s)` with a whole literal `n` of 2 or more and `n·s` at most 38 lowers: widen to `Decimal(38, n·s)`, multiply exactly, round once to `s`. A larger or negative `n` is the engine's; a fractional `n` is a `Float` | Rounding at each step would double-round and disagree with the engine, which rounds the exact power once. At `s = 10` this covers `**2` and `**3` |
| T18 | Proposed | `Percent` stays a dress over `Decimal(s)`; `Quantity` and `Scientific` stay `Float`-based | Unaffected |

## Lowering

With a column scale, `Decimal(a) + Decimal(b)` is `col_a.cast(Decimal(38, m)) + col_b.cast(Decimal(38, m))`, `m = max(a, b)`. Widening a scale is exact, so the two paths agree by construction. For `*` the operands are cast the same way and the product is rounded to `m`; for `/` the quotient is rounded to the left scale; the mean is a sum over a count by the same rule. Two things decide whether Polars' own multiply and divide can be used or an explicit round is needed, and the first task is to find out:

1. Whether Polars truncates or rounds when it scales a product back. If it truncates, an explicit round-half-away is wrapped round it.
2. Whether a quotient is truncated or rounded at its `max(x, y) + 4` digits. Rounding the digit after the cut is correct when it is truncated; if it is rounded there, a double rounding is possible for a digit run of `4999…` and the quotient is computed with an explicit guard digit instead.

The intermediate of a product is at twice the scale, so a product of two values passes `i128` once `|a·b|·10^(2m)` reaches `1.7·10³⁸`. That is the existing guard's job (D72-era), kept as T16.

## Migration

- A file written before this change whose `Decimal` column has a cell with more than 10 places loads as inferred (T11), not as bare `Decimal`, unless the column says `Decimal` outright. A column that says bare `Decimal` and has such a cell gets a diagnostic per cell naming `Decimal(n)`.
- Results that kept up to 30 digits now keep the column's. The examples are regenerated and the differences read, not just accepted.
- The app's column-type picker gains `Decimal(n)` and `Fraction`; a change of scale re-rounds nothing silently (T10).

## Open

- Settled by the owner (2026-11-14): `Integer / Integer` is a `Decimal(10)`, not a `Fraction`, accepting that `(1/3) * 3` is `0.9999999999`, and a `Fraction` is one `.Fraction` away; a `Fraction` is shown as a fraction (T9); the window functions lower over a `Decimal(s)`; the default scale is `Decimal(38, 10)` (T1).
- Window functions over a `Decimal(s)`: Polars' `cum_sum`, `diff` and `rank` over a decimal dtype are to be checked (task 0.2), and `rolling_*` of a sum or mean stay on the exact path as for floats unless the check shows a decimal slides exactly, which a fixed-point integer does.
- Settled by the owner (2026-11-14): the default scale is 10. One consequence for the lowering: if Polars multiplies at twice the scale, a product passes `i128` once `|a·b|` reaches about 10¹⁸, and the guard hands such a column to the `Fraction` engine path (T16). Task 0.2 measures how often that is.

## Risks

- `Fraction` denominators grow without a bound under repeated division. That is the `FatRat` choice, taken on purpose; the display cut is what keeps it readable.
- A type with a parameter in a name (`Decimal(2)`) is new to the checker's `S`, which is `Copy` today. A scale is a `u8` and `S` stays `Copy`.
- Anyone relying on `1/3` keeping thirty digits changes behaviour; the release note says so.
