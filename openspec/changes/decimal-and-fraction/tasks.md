# Tasks: `Decimal(s)` and `Fraction`

Draft; nothing is built.

## 0. Settle
- [x] 0.1 Ratify T1-T18 and decide the Open list in `design.md`
- [x] 0.2 (measured; see `design.md`) A test of Polars 0.51's decimal `+ - * /`, `mean`, `cum_sum`, `diff`, `rank` and `rolling_*` near the limits (rounding or truncation of a product and a quotient; the scale of the result; the intermediate of a product at twice the scale); record the answer in T15 and T1
- [x] 0.3 Decide the dependency: `num-bigint` and `num-rational`, behind the `Fraction` type only

## 1. The type
- [x] 1.1 `S::Decimal` carries a scale (`u8`), `S::Fraction`; `S` stays `Copy`
- [x] 1.2 Parse and print `Decimal(n)`; bare `Decimal` is `Decimal(10)`
- [x] 1.3 The lattice and `join` (T7); assignability (T10)
- [x] 1.4 Inference of an undeclared column (T11)

## 2. The value
- [x] 2.1 `Dec` as one value at a scale; no point-moving; rounding half away from zero (T2, T3)
- [x] 2.2 The T4 error and its message (an undeclared result that does not fit is a `Fraction`; a declared `Decimal` refuses it)
- [x] 2.3 `Value::Fraction` over `BigRational`: arithmetic, comparison, display, the `·` cut (T6, T8, T9)
- [x] 2.4 Conversions among `Integer`, `Decimal(s)`, `Fraction` and `Float`

## 3. The checker
- [x] 3.1 Result types of `+ - * /`, `mean`, `sum`, `**` and the window functions (T5)
- [x] 3.2 A declared column refuses by cell (T10)
- [x] 3.3 Diagnostics name the type that would hold the value

## 4. Lowering
- [x] 4.1 Casts to the common scale; explicit round after `*` and `/` (T15)
- [x] 4.2 `/` and `mean` lowered
- [x] 4.3 The guard, with the `Fraction` engine path as its fallback (T16)
- [x] 4.4 Window functions over `Decimal(s)` (all but `pct_change` and the rolling windows, which Polars has no decimal version of)
- [x] 4.6 `x ** n` for a small whole literal `n` (T19) (the base multiplied by itself, `n` from 2 to 38 and the scale `n·s` at most 38, then one rounding; any other power is the engine's)
- [x] 4.5 Remove the guard digits and the lowering refusals the old design needed

## 5. Interop
- [x] 5.1 Parquet read and write of `Decimal(p, s)` (T12) (`Decimal(p, s)` read at its scale; one of more than eighteen places is declared in the `.omx` written)
- [x] 5.2 Parquet `Fraction` as `{num, den}` with the kind in metadata; `--decimal` (T13) (a fraction of two whole numbers of any length is a cell literal, `parse_big_fraction`)
- [x] 5.3 CSV, JSON and XLSX (T14) (`n/d` text in CSV and JSON, the nearest double in XLSX)

## 6. App
- [x] 6.1 Column-type picker: `Decimal(n)` typed in the box, `Fraction` in the list; `Set type` takes `Decimal(n)` and replaces it whole
- [x] 6.2 Footer shows the exact value of a cut `Fraction` or `Decimal`

## 7. Checks
- [x] 7.1 Table path and sheet path agree on every operation, scale pair and edge value (as `kernel.rs`)
- [x] 7.2 Regenerate the examples and read each `.expected` difference
- [x] 7.3 A 20-million-row decimal column: `+ - * /` and `mean` lowered, timed (`timings.rs`; at 5 million rows lowered is 10.7 times faster than the engine, 365 ms against 3.9 s, with a third of the memory; at 20 million lowered takes 1.6 s and 2.1 GiB; every answer is the same on both paths)
- [x] 7.4 Docs, skill, `project.md` principle 3, release note on the behaviour change (`expressions.md`, `format.md`, `interop.md`, the skill, `project.md`; the release note is drafted in `release-note.md` for the version that ships it)
- [x] 7.5 fmt, clippy -D warnings, workspace tests
