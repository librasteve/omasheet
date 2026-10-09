# Proposal: Matrix multiplication (Phase 6)

## Why

The core names four value shapes — scalar, vector, matrix and table — but builds
only three: there is no matrix literal and no matrix value. Weighted totals,
input–output and transition models, rotations and linear maps are all one matrix
product, which in a grid spreadsheet is `MMULT` over hand-counted ranges and in
Phase 1 OMX cannot be written at all. `*` is already element-wise, so the
product needs its own operator. Undoing a product needs the inverse, and telling
whether it can be undone needs the determinant — `MINVERSE` and `MDETERM` in a
grid spreadsheet, there in floating point and here exact.

## What Changes

- **omx-expressions** — ADDED: matrix values and literals, a table as a matrix,
  the matrix product `@`, `transpose`, `inverse`, `det`; MODIFIED:
  broadcasting covers matrices
- **evaluation** — ADDED: matrix shapes are checked before execution; the
  product, inverse and determinant preserve exactness

## Impact

- Depends on `add-omasheet-core` only
- `omasheet-omx` gains the `@` operator, nested vector literals and a matrix
  shape in the checker; `omasheet-engine` gains a matrix value over the same
  exact numbers
- No change to the `.omx` file syntax outside expressions; a cell still holds
  one value, so a matrix lives in a constant or is reduced before it reaches a
  cell

## Out of Scope

- A dedicated solver for linear systems (`inverse(A) @ b` serves),
  decompositions, matrix powers, pseudo-inverses
- Arrays of more than two dimensions
- Sparse or lazily evaluated matrices
- Units and uncertainty through a product: deltas for `add-units` and
  `add-uncertainty` once this phase and those have landed
