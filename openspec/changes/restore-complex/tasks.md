# Tasks: Complex returns

## 0. Settle the open questions
- [ ] 0.1 P201: whether `3+0i` is shown as such
- [ ] 0.2 P202: how `.xlsx` carries a `Complex`

## 1. The type (`numerics`)
- [ ] 1.1 `S::Complex` in `omasheet-omx/src/types.rs`, above `Float` in `wider` and `rank`, with the lattice tests extended
- [ ] 1.2 `Value::Complex(f64, f64)` and `Lit::Complex`, with arithmetic (`+ - * /`, `**` as `exp(w ln z)`), equality, and no order, in `value.rs`
- [ ] 1.3 The result type of arithmetic with a `Complex` side in `check.rs`
- [ ] 1.4 Display: `3+4i`, `1.5-2i`, `0+Inf\i`, cut to the 23-character rule, localised and grouped in the app, exact under `--exact`

- [ ] 1.5 `Complex` equality is exact on both parts, and `Float` comparison is not touched
- [ ] 1.6 `lower.rs` declines a `Complex` column and any expression over one, and nothing else (D200); a test that a `Float` filter still lowers

## 2. The language (`omx-expressions`)
- [ ] 2.1 Imaginary literals in the lexer: `4i`, `Inf\i`
- [ ] 2.2 `re`, `im`, `conj`, `arg`; `abs`, `sqrt`, `exp`, `ln` accept a `Complex` and every other function refuses one, naming it
- [ ] 2.2b `z.Float`, `z.Decimal`, `z.Integer`, `z.Fraction` of a `Complex` convert when the imaginary part is within `1e-15` of zero (absolute), and are otherwise a cell error (D201)
- [ ] 2.3 `x.Complex` and `Complex(re, im)`; `Complex` in the function directory under its own category
- [ ] 2.4 The function reference in `docs/expressions.md` and the skill

## 3. The file (`file-format`)
- [ ] 3.1 `Complex` is a declarable type name; a real number in a `Complex` column is that number with no imaginary part
- [ ] 3.2 An undeclared column of `3+4i` and `2.5` is `Complex` by the join (D191)
- [ ] 3.3 A cell in a declared column is accepted when its conversion to the type succeeds and is shown in that type's format; otherwise it is an error naming the conversion (D203). Tests: `0b1` and `0x1F` in `Integer`, `3+0i` and `3+4i` in `Float`, `1.5` in `Integer`
- [ ] 3.4 `docs/format.md`: the lattice tables with `Complex`, and the declared-column rule

## 4. Parquet and the other formats (`parquet-interop`)
- [ ] 4.1 `dtype.rs`: `S::Complex` ↔ `Struct{re: Float64, im: Float64}`, in `type_of`, `cell_of` and the writer
- [ ] 4.2 Any other struct is dropped with a notice, as now; a struct with a null field is `Invalid` with a notice
- [ ] 4.3 Lowering leaves a `Complex` column to the engine (D200)
- [ ] 4.4 `json.rs`: `{"re": …, "im": …}`, both ways
- [ ] 4.5 `xlsx/write.rs` per P202
- [ ] 4.6 `docs/interop.md`

## 5. Examples and checks
- [ ] 5.1 `examples/display.omx`: the `Complexes` sheet back; `examples/types.omx`: `Wave`, `Size`, `Turned` and `AsComplex` back; `.expected` files regenerated
- [ ] 5.2 The tests removed by `4f85389` in `omasheet-engine/tests/spec.rs`, `interop.rs` and `value.rs`, restored and updated for the lattice
- [ ] 5.3 A Parquet round trip of a `Complex` column, and a read of a `Struct{re, im}` file written by another tool
- [ ] 5.4 `README.md`: the type list
- [ ] 5.5 The workspace passes
