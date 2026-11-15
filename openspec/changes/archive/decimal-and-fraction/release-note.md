## Decimals and fractions

- `Decimal` is now a `Decimal(38, s)` with a scale you can choose, `Decimal(6)`; the default is 10 places. Every `+ - * /`, `mean`, window function and small whole power of a decimal runs in Polars, at the engine's own answer, to the digit.
- New `Fraction` type: exact, of any size (a googol, one over the number of atoms in the universe), never lowered. A number too big for a `Decimal` makes its undeclared column a `Fraction`; a declared `Decimal` column refuses it.
- A product or quotient is worked out exactly and rounded once, half away from zero, to the scale of its type.
- Parquet writes a `Fraction` as a struct of `num` and `den` strings, or as doubles with `--decimal`.
- The type picker has `Fraction` and takes `Decimal(n)`; the footer shows the exact value of a cut `Fraction` or `Decimal`.
- Behaviour change: results that used to be cut at 30 digits are now exact or rounded once to the column's scale.
