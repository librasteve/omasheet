# Tasks: `Decimal(s)` and `Fraction`

Draft; nothing is built.

## 0. Settle
- [ ] 0.1 Ratify T1-T18 and decide the Open list in `design.md`
- [ ] 0.2 A test of Polars 0.51's decimal `+ - * /`, `mean`, `cum_sum`, `diff`, `rank` and `rolling_*` near the limits (rounding or truncation of a product and a quotient; the scale of the result; the intermediate of a product at twice the scale); record the answer in T15 and T1
- [ ] 0.3 Decide the dependency: `num-bigint` and `num-rational`, behind the `Fraction` type only

## 1. The type
- [ ] 1.1 `S::Decimal` carries a scale (`u8`), `S::Fraction`; `S` stays `Copy`
- [ ] 1.2 Parse and print `Decimal(n)`; bare `Decimal` is `Decimal(10)`
- [ ] 1.3 The lattice and `join` (T7); assignability (T10)
- [ ] 1.4 Inference of an undeclared column (T11)

## 2. The value
- [ ] 2.1 `Dec` as one value at a scale; no point-moving; rounding half away from zero (T2, T3)
- [ ] 2.2 The T4 error and its message
- [ ] 2.3 `Value::Fraction` over `BigRational`: arithmetic, comparison, display, the `·` cut (T6, T8, T9)
- [ ] 2.4 Conversions among `Integer`, `Decimal(s)`, `Fraction` and `Float`

## 3. The checker
- [ ] 3.1 Result types of `+ - * /`, `mean`, `sum`, `**` and the window functions (T5)
- [ ] 3.2 A declared column refuses by cell (T10)
- [ ] 3.3 Diagnostics name the type that would hold the value

## 4. Lowering
- [ ] 4.1 Casts to the common scale; explicit round after `*` and `/` (T15)
- [ ] 4.2 `/` and `mean` lowered
- [ ] 4.3 The guard, with the `Fraction` engine path as its fallback (T16)
- [ ] 4.4 Window functions over `Decimal(s)`
- [ ] 4.6 `x ** n` for a small whole literal `n` (T19)
- [ ] 4.5 Remove the guard digits and the lowering refusals the old design needed

## 5. Interop
- [ ] 5.1 Parquet read and write of `Decimal(p, s)` (T12)
- [ ] 5.2 Parquet `Fraction` as `{num, den}` with the kind in metadata; `--decimal` (T13)
- [ ] 5.3 CSV, JSON and XLSX (T14)

## 6. App
- [ ] 6.1 Column-type picker and schema editor: `Decimal(n)`, `Fraction`
- [ ] 6.2 Footer shows the exact value of a cut `Fraction` or `Decimal`

## 7. Checks
- [ ] 7.1 Table path and sheet path agree on every operation, scale pair and edge value (as `kernel.rs`)
- [ ] 7.2 Regenerate the examples and read each `.expected` difference
- [ ] 7.3 A 20-million-row decimal column: `+ - * /` and `mean` lowered, timed
- [ ] 7.4 Docs, skill, `project.md` principle 3, release note on the behaviour change
- [ ] 7.5 fmt, clippy -D warnings, workspace tests
