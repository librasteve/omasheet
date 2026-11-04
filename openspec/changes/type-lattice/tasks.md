# Tasks: the type lattice

## 1. The lattice (`numerics`)
- [x] 1.1 `S::wider` and `rank` follow D181 to D183, with a test of the tables
- [x] 1.2 The result type of arithmetic is the join (D184)
- [x] 1.3 `arith` gives a `Fraction` or a `Percent` only where both sides are
- [x] 1.4 A power with a non-whole exponent is a `Float` (D185)
- [x] 1.5 A whole quotient of literals is an `Integer`, and a quotient is accepted where a whole number is wanted (D187)
- [x] 1.6 A column left to its cells that holds `Inf`, `-Inf` or `NaN` is a `Float` column, in its heading and in a Parquet export; a declared exact column's error says how to fix it (D188)
- [x] 1.7 `<1/10>` and `<2 3/4>` are `Fraction` literals in a formula (D189)

## 2. Documents (`file-format`)
- [x] 2.1 `docs/format.md`: the three tables, and a cell is stored as entered
- [x] 2.2 `docs/expressions.md` and the skill
- [x] 2.3 `examples/types.omx` with `.expected`: `Joined` and `Lattice` sheets

## 2b. Undeclared columns (D191 to D194, to build)
- [ ] 2b.1 A cell entered into an undeclared column never writes a type line: `based_declared` and the app's inference of `Hex`, `Octal` and `Binary` go
- [ ] 2b.2 An undeclared column shows each cell in the format of its own contents, whatever is entered beside it
- [ ] 2b.3 The heading shows the inferred type as the join of the current cells, `Binary` for one `0b11`, widening to `Integer` with `255`
- [ ] 2b.4 Setting a type shows every compatible cell in that type's format, and an entered value is shown in it too: `15` in a `Binary` column is `0b1111`
- [ ] 2b.5 The same for `Fraction` and `Percent` under `Decimal`, with tests for both families

## 3. Checks
- [x] 3.1 The workspace passes but for two `numfmt` tests (`%+f` of NaN, `%b` of 0), which test C's formatting and do not touch the lattice
