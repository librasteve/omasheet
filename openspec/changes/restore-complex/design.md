# Design: Complex returns

## Context

Source: the conversation of 2026-11-04. Reverses D18 of `add-polars-dtypes`. D numbers continue from `type-lattice` (D194). The code to restore is in the commit before `4f85389`, "Complex numbers go".

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D195 | Decided | `Complex` is its own type, a pair of `Float`s; Parquet stores it as `Struct{re: Float64, im: Float64}` | Owner, 2026-11-04, taking the narrow route over a general `Struct` dtype. The storage is the only place a struct appears, so no `Struct` value, syntax or lattice rule is needed. D14 holds in the sense that matters: a Parquet file round-trips without a lossy conversion |
| D196 | Decided | `Complex` sits above `Float` in the lattice, and is the most infectious type | As before the removal. A `Complex` with an `Integer`, `Decimal` or `Float` is a `Complex`; with text it is `Any` (D181) |
| D197 | Decided | The arithmetic, functions, literals and display are those that `4f85389` removed | They were specified and tested then: `z ** w = exp(w ln z)`, the principal square root, `ln` with the principal argument, display as `3+4i` with `\i` after a non-finite imaginary part, as Raku does. No behaviour is redesigned |
| D198 | Decided | A `Complex` has no order and `<`, `>`, `min`, `max` and `sort` are errors on one; `=` and `!=` compare both parts | As before |
| D199 | Decided | Read from Parquet, only a struct of exactly `re` then `im`, both `Float64`, is a `Complex`; any other struct is dropped with a notice | Narrow on purpose: it must not claim a struct that happens to be called something similar. A `Float32` pair is not recognised; widening is later work |
| D200 | Decided | A `Complex` column is never lowered to Polars; expressions over one run on the engine. Nothing else is held back: a comparison of `Float` values lowers as it does today | Polars has no arithmetic for a struct. The pushdown rules of `add-dataframe-interop` already leave out anything they cannot lower. Lowering is the priority (owner, 2026-11-04), which is why D202 is as narrow as it is |
| D201 | Decided | Converting a `Complex` to a real type (`z.Float`, `z.Decimal`, `z.Integer`, `z.Fraction`, …) works when the imaginary part is within tolerance of zero and is otherwise a cell error: `Cannot convert 3+4i to Float: imaginary part not zero` | Owner, 2026-11-04, copying Raku, checked against it: `(3+4i).Num` fails, `(3+0i).Num` is `3e0`, `(3+1e-16i).Num` is `3e0` and `(3+1e-15i).Num` fails. The test is on the imaginary part alone and is absolute (`=~= 0`), not relative to the real part. The value is checked when the conversion is made, so the type checker allows the call |
| D202 | Decided | Tolerance is used in one place: the test that the imaginary part of a `Complex` is zero (D201). `Float` and `Complex` comparisons are exact, as `==` is in Raku: `0.1e0 + 0.2e0 = 0.3e0` stays false, and two `Complex` are equal when both parts are exactly | Owner, 2026-11-04, narrowing an earlier draft that gave every `Float` comparison Raku's `=~=` tolerance. That would have changed existing `Float` behaviour and kept every `Float` comparison out of Polars, which compares exactly. Raku's `==` is exact too, so this also matches the language. `Complex` is not lowered at all (D200), so it has no Polars to disagree with, but it is kept exact for the same reason: one rule for both |
| D203 | Decided | A cell in a declared column is accepted when its conversion to the column's type succeeds, and is then shown in that type's format; otherwise it is a cell error naming the conversion that failed. So `0b1` and `0x1F` in an `Integer` column are `1` and `31` (the file keeps them as entered, D193), `3+0i` in a `Float` column is `3`, and `3+4i` there is an error | Owner, 2026-11-04. One rule for all columns instead of a case for `Complex`. The `Integer` case is the one D193 already gives: `Binary` is an `Integer` shown another way, so it narrows by losing nothing. The rule also needs the existing treatment of a lossy cell (`1.5` in an `Integer` column) to be an error, which is to be confirmed in the code, not assumed |
| D204 | Decided | The lattice is `Integer` < `Decimal` < `Float` < `Complex`, `Complex` having no display family; the join of `Complex` with any number is `Complex` and with text is `Any` | Owner, 2026-11-04 |
| D205 | Decided | A `Complex` whose imaginary part is zero is shown with it: `3+0i`, not `3` | `4f85389^` showed it so, and a column keeps one shape from cell to cell. Built so, 2026-11-04 |
| D206 | Decided | A `Complex` cell is written to `.xlsx` as text, `3+4i`, and reported as a loss; a formula that uses one is written as its value, and reported | A workbook has no complex cell. Text is the form Excel's complex functions read. This is the existing text path of the writer, with no case of its own. Built so, 2026-11-04 |

## Holding

- Nothing about a `Struct` is said in the language. If a `Struct` dtype is added later, `Complex` stays as the named case, and D199 is the place its Parquet rule would widen.
- `Complex` is the first type whose Parquet dtype is not one of the names in the type table of `add-polars-dtypes`. That table gains one row: `Complex` → `Struct{re: Float64, im: Float64}`.

## Risks

- The restored code was written before the lattice, `Fraction` as a display, the 38-digit `Decimal` and the guard digits. A `Complex` made from a `Decimal` converts through a `Float` and loses the guard digits' exactness; this is the same as `x.Float`, and is stated in `docs/expressions.md`.
- `Float` comparison is untouched, so existing sheets and Polars lowering are unaffected. The tolerance applies only to a `Complex` converted to a real type, so a `z.Float` can succeed or fail on a value that `z.im = 0` says otherwise (`3+1e-16i`). The docs say so, and that `im(z) = 0` is the exact test.
- Two `.expected` files change in places that are not this change's own (`display`, `types`). They are regenerated, not edited, as in `4f85389`.
