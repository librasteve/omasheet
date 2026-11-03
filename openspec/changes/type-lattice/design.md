# Design: the type lattice

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D181 | Decided | `Integer` < `Decimal` < `Float`; a number with text is `Any`, and text with text is `String` | Owner, 2026-11-03, correcting a first table that made it `String` |
| D182 | Decided | `Fraction` and `Percent` are `Decimal`s shown another way, not wider than one; `Hex`, `Octal` and `Binary` are `Integer`s the same way | Owner, 2026-11-03 |
| D183 | Decided | Two of one type give that type; two different members of a family, or one with its parent, give the parent | Owner, 2026-11-03 |
| D184 | Decided | Arithmetic returns the join of its sides, with no case for `Fraction` or `Percent`; a quotient of whole numbers is a `Decimal`, as it was | Owner, 2026-11-03 |
| D185 | Decided | A power with a whole exponent is exact; any other is a `Float` | Owner, 2026-11-03 |
| D186 | Decided | A cell does not remember the form it was written in. The file stores the cell text as entered, and the column's type decides how it is shown | Owner, 2026-11-03, dropping "each cell remembers its literal" |
| D187 | Decided | A quotient of whole numbers written out that comes to a whole number, `4/2`, is an `Integer`; any other quotient is a `Decimal`, and where a whole number is wanted (a row, a range end, a place count, a bitwise operand) a `Decimal` made from a quotient is accepted and its value is checked when it is used | Owner, 2026-11-03 |
| D188 | Decided | A division by zero is a `Float` (`Inf`, `-Inf`, `NaN`), and infects: a column left to its cells that holds one is a `Float` column and is exported as one, with every value kept; a declared exact column refuses it with an error saying how to fix it. It is not turned into a null, and an exact column does not hold it | Owner, 2026-11-03, over letting an exact column hold it |
| D189 | Decided | A fraction is written in a formula in angle brackets, `<1/10>`, `<-3/4>`, `<2 3/4>`, and is the `Fraction` of that number; a bare `1/10` in a formula stays a quotient | Owner, 2026-11-03 |

## Holding

`rank` has one level for every exact type, so `wider` of two different exact
types is decided by family and not by rank. A `Percent` divided by a
`Percent` is a `Percent`, by D184.
