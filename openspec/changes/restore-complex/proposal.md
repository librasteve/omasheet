# Proposal: Complex returns

## Why

`add-polars-dtypes` removed `Complex` (D18) because Polars has no complex dtype, and noted that it "may return as a `Struct{re, im}`". The owner has asked for it back (2026-11-04). Complex numbers are a first-class part of Raku, which omasheet follows, and the removal took out the only way to work with roots of negative numbers, rotations and waves in a sheet.

The first design was a general `Struct` dtype with `Complex` as a named case of it. That is a change in itself (a `Struct` value, lattice rules for it, JSON and xlsx handling, file syntax), and `Struct` is deferred along with `List`, `Array`, `Categorical` and `Enum`. This change is the narrow route: `Complex` comes back as its own type with its own arithmetic, and only its **storage** in Parquet is a `Struct{re: Float64, im: Float64}`.

## What Changes

- **numerics**: ADDED. A `Complex` type, a pair of `Float`s. It is the most infectious type: arithmetic with a `Complex` on either side gives a `Complex`. `+ - * /` and `**` follow the complex rules, and a `Complex` has no order (`<` is an error) but does have equality. It is shown as `3+4i`, each part as a `Float`, cut to fit the 23-character rule of the display spec.
- **omx-expressions**: ADDED. Imaginary literals (`4i`, `Inf\i`, so `3+4i` is a sum and a `Complex`); `re`, `im`, `conj` and `arg`; `abs`, `sqrt`, `exp` and `ln` accept a `Complex`; `x.Complex` and `Complex(re, im)` convert. Every other function is refused for a `Complex` with an error naming it.
- **file-format**: ADDED. `Complex` is a declarable type name. A cell holding `3+4i` in a column declared `Complex` is that number, and a real number in such a column is that number with no imaginary part. Undeclared, the type is inferred as the join of the cells (D191), so `3+4i` beside `2.5` is `Complex`.
- **numerics**: ADDED. A `Complex` converts to a real type when its imaginary part is within Raku's tolerance, 1e-15, of zero, and otherwise is an error. This is the only use of tolerance: `Float` and `Complex` comparison stays exact, so nothing is held back from Polars.
- **file-format**: ADDED. A cell in a declared column is accepted when its conversion succeeds and is shown in the column's format: `0b1` in an `Integer` column is `1`, `3+0i` in a `Float` column is `3`.
- **parquet-interop**: MODIFIED. A `Complex` column is written as a Parquet struct of two `Float64` fields named `re` and `im`. A struct column that has exactly those two fields, in that order, both `Float64`, is read as `Complex`; any other struct is dropped with a notice, as before. A null struct is `Null`; a struct with one null field is `Invalid` with a notice.
- **lattice**: `Float` < `Complex`. A `Complex` with text is `Any`, as every number with text is.

## Impact

- `omasheet-omx`: `S::Complex` in `types.rs` and its place in `wider` and `rank`; `Lit::Complex` in `ast.rs`; imaginary literals in `lexer.rs`; `MathFn::{Re, Im, Conj, Arg}` and `of_complex` in `funcs.rs`; the result types in `check.rs`; `convert.rs`
- `omasheet-engine`: `Value::Complex` and its arithmetic, comparison, equality and display in `value.rs`; `dtype.rs` maps `S::Complex` to `Struct{re, im}` both ways; `eval.rs`, `doc.rs`
- `omasheet-interop`: `json.rs` (a `Complex` is `{"re": …, "im": …}`), `xlsx/write.rs` (a `Complex` is written as its text, which Excel's `IMSUM` and friends read), `parquet.rs`
- `docs/format.md`, `docs/expressions.md`, `docs/interop.md`, `README.md`, `.claude/skills/omasheet/SKILL.md`
- `examples/display.omx` (the `Complexes` sheet), `examples/types.omx` (the `Wave`, `Size`, `Turned` and `AsComplex` columns), `examples/showcase/undeclared.omx` (a `Complex` pair of columns in `CoreTypes`, and a `Joins` sheet for the lattice and declared columns), and `examples/functions.omx`, with `.expected` files regenerated
- Most of this is a restoration of `4f85389^`, rebased onto the type lattice (`Integer`, `Decimal`, `Float`) and the `Float` name for `Num`. The 4f85389 diff is the starting point, not a patch to apply
- Supersedes D18 of `add-polars-dtypes`, and the `Complex` entry of its Out of Scope

## Out of Scope

- A general `Struct` dtype, and a named-field value in the language: deferred as in `add-polars-dtypes`. Reading a Parquet struct of any other shape stays a notice and a dropped column
- Complex columns lowered to Polars operations. A `Complex` column is computed by the scalar engine path; Polars has no arithmetic for the struct, so pushdown (`add-dataframe-interop`) leaves such a column and any expression over it to the engine
- Exact complex numbers. Both parts are `Float`s, as before. A `Complex` of `Decimal` parts would be a different type
- Polar literals and a polar display
- A tolerance that can be changed: `$*TOLERANCE` is fixed at 1e-15
