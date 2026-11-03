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

## 3. Checks
- [x] 3.1 The workspace passes but for two `numfmt` tests (`%+f` of NaN, `%b` of 0), which test C's formatting and do not touch the lattice
