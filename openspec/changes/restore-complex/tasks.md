# Tasks: Complex returns

## 0. Settle the open questions
- [x] 0.1 D205 (was P201): whether `3+0i` is shown as such
- [x] 0.2 D206 (was P202): how `.xlsx` carries a `Complex`

## 1. The type (`numerics`)
- [x] 1.1 `S::Complex` in `omasheet-omx/src/types.rs`, above `Float` in `wider` and `rank`, with the lattice tests extended
- [x] 1.2 `Value::Complex(f64, f64)` and `Lit::Complex`, with arithmetic (`+ - * /`, `**` as `exp(w ln z)`), equality, and no order, in `value.rs`
- [x] 1.3 The result type of arithmetic with a `Complex` side in `check.rs`
- [x] 1.4 Display: `3+4i`, `1.5-2i`, `0+Inf\i`, cut to the 23-character rule, localised and grouped in the app, exact under `--exact`

- [x] 1.5 `Complex` equality is exact on both parts, and `Float` comparison is not touched
- [x] 1.6 `lower.rs` declines a `Complex` column and any expression over one, and nothing else (D200); a test that a `Float` filter still lowers

## 2. The language (`omx-expressions`)
- [x] 2.1 Imaginary literals in the lexer: `4i`, `Inf\i`
- [x] 2.2 `re`, `im`, `conj`, `arg`; `abs`, `sqrt`, `exp`, `ln` accept a `Complex` and every other function refuses one, naming it
- [x] 2.2b `z.Float`, `z.Decimal`, `z.Integer`, `z.Fraction` of a `Complex` convert when the imaginary part is within `1e-15` of zero (absolute), and are otherwise a cell error (D201)
- [x] 2.3 `x.Complex` and `Complex(re, im)`; `Complex` in the function directory under its own category
- [x] 2.4 The function reference in `docs/expressions.md` and the skill

## 3. The file (`file-format`)
- [x] 3.1 `Complex` is a declarable type name; a real number in a `Complex` column is that number with no imaginary part
- [x] 3.2 An undeclared column of `3+4i` and `2.5` is `Complex` by the join (D191)
- [x] 3.3 A cell in a declared column is accepted when its conversion to the type succeeds and is shown in that type's format; otherwise it is an error naming the conversion (D203). Tests: `0b1` and `0x1F` in `Integer`, `3+0i` and `3+4i` in `Float`, `1.5` in `Integer`
- [x] 3.4 `docs/format.md`: the lattice tables with `Complex`, and the declared-column rule

## 4. Parquet and the other formats (`parquet-interop`)
- [x] 4.1 `dtype.rs`: `S::Complex` ↔ `Struct{re: Float64, im: Float64}`, in `type_of`, `cell_of` and the writer
- [x] 4.2 Any other struct is dropped with a notice, as now; a struct with a null field is `Invalid` with a notice
- [x] 4.3 Lowering leaves a `Complex` column to the engine (D200)
- [x] 4.4 `json.rs`: `{"re": …, "im": …}`, both ways
- [x] 4.5 `xlsx/write.rs`: a formula with a `Complex` is written as its value and reported; a `Complex` cell is written as text, `3+4i`, and reported (D206)
- [x] 4.6 `docs/interop.md`

## 5. Examples and checks
- [x] 5.1 `examples/display.omx`: the `Complexes` sheet back, from `4f85389^`, with `Num` written `Float`; `examples/types.omx`: `Wave`, `Size`, `Turned` and `AsComplex` back, and the header comment's list of types; `.expected` files regenerated
- [x] 5.2 `examples/showcase/undeclared.omx`, `CoreTypes` sheet: a `_3plus4i` column left undeclared and a `_3plus4is : Complex` column declared, beside the `Integer`, `Decimal` and `Float` pairs, with the same rows (`x+x`, `x-x`, `x*x`, `x/x`, the four with a constant `1`, and the `x/0`, `(x-x)/0`, `1/0` and `0/0` rows). The `.floor()` variants do not apply to a `Complex` and are written without. `x/0` is the case to look at: a `Complex` absorbs the infinity (D188 and the lattice rule), so it is `Inf` in each part, and it should show as such, not as an error
- [x] 5.3 `examples/showcase/undeclared.omx`, a `Joins` sheet: an undeclared column holding `1`, `1.5`, `1e0` and `3+4i`, whose heading shows `Complex`, and the same cells in a column declared `Float`, where `3+4i` is an error and `3+0i` is `3`; the same pair for `Integer` with `0b1` and `0x1F`, shown as `1` and `31` (D203). This is the lattice and the declared-column rule in one place
- [ ] 5.4 `examples/showcase/undeclared.omx` is run in the app, not only through the CLI: the heading of an undeclared column, and a `Complex` cell entered into an `Integer` column
- [ ] 5.5 `examples/functions.omx`: `re`, `im`, `conj`, `arg`, and `abs`, `sqrt`, `exp`, `ln` of a `Complex`; `examples/logic.omx` or `types.omx`: `3+4i = 3+4i`, and `1i < 2i` as the error it is
- [x] 5.6 A Parquet round trip of a `Complex` column, and a read of a `Struct{re, im}` file written by another tool; a `examples/` Parquet is not added, the test writes its own
- [x] 5.7 The tests removed by `4f85389` in `omasheet-engine/tests/spec.rs`, `interop.rs` and `value.rs`, restored and updated for the lattice
- [x] 5.8 `README.md`: the type list
- [ ] 5.9 The workspace passes, and every example matches its `.expected`
