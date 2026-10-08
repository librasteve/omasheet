# Design: Matrix multiplication (Phase 6)

## Context

Builds on `add-omasheet-core`; D/P/Q numbers continue from the earlier changes.
The core's `Value shapes` requirement already lists the matrix, and its design
notes record that no distinct matrix shape was built in Phase 1.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| P24 | Provisional | The matrix product is the infix operator `@`, binding like `*` and grouping to the left | `*` stays element-wise (core `Broadcasting`). Alternatives seen: a `dot` / `mmul` function only; `·`; making `*` the matrix product and adding `.*`, which would change Phase 1 |
| P25 | Provisional | A matrix literal is a vector of equal-length vectors: `[[1, 2], [3, 4]]` | `[1, 2; 3, 4]` reads well with "`;` separates dimensions" but `[1; 2]` already means an index into the current table |
| P26 | Provisional | A table whose selected columns are all numeric may stand where a matrix is wanted; the result is a matrix, never a table | Column names do not survive a product. `.matrix()` makes the conversion explicit |
| P27 | Provisional | With `@`, a vector on the right is a column and a vector on the left is a row; two vectors give their dot product | The convention of NumPy's `@` |
| D19 | Decided | The product is exact for `Int` and `Rational` elements, as every other operator is | Principle 3; follows D7 |

## Open Questions

**Q13 — The operator.** `@` is unused in OMX and familiar from Python, but it is
the least self-explanatory symbol in the language. Confirm it, or choose a
named form, before the lexer changes.

**Q14 — Matrices in cells.** A cell holds one value. Should a formula that
yields a 1 × 1 matrix be reduced to its element, as a vector of one is?

## Risks

- **Cost.** The straightforward product of two n × n `Rational` matrices is n³
  exact multiplications with growing denominators. Acceptable at spreadsheet
  sizes; no fast path is planned.
- **Two products.** `*` and `@` on the same operands give different answers.
  A shape error for `@` should say which operator the author may have meant.
