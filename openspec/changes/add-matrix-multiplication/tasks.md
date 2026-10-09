# Tasks: Matrix multiplication (Phase 6)

## 0. Settle the open questions
- [ ] 0.1 Confirm the operator (design Q13) and the 1 × 1 rule (design Q14)
- [ ] 0.2 Decide how a nearly singular `Num` matrix is treated (design Q15)

## 1. Matrix values (`omx-expressions`)
- [ ] 1.1 Matrix value in the engine and matrix shape in the checker
- [ ] 1.2 Nested vector literals, with a diagnostic for ragged rows
- [ ] 1.3 Indexing `m[r; c]`, `m[r]`, ranges and empty slots
- [ ] 1.4 A numeric table as a matrix, and `.matrix()`
- [ ] 1.5 Element-wise broadcasting over matrices
- [ ] 1.6 Display in `eval` and the read-only view

## 2. Product (`omx-expressions`, `evaluation`)
- [ ] 2.1 Lex and parse `@` at the precedence of `*`
- [ ] 2.2 Matrix–matrix, matrix–vector, vector–matrix and vector–vector products
- [ ] 2.3 Shape checking before execution where the shapes are known, at execution otherwise
- [ ] 2.4 `transpose()`
- [ ] 2.5 Verify exactness for `Int` and `Ratio`, and widening for `Num` and `Complex`

## 3. Inverse and determinant (`omx-expressions`, `evaluation`)
- [ ] 3.1 `det()` by fraction-free elimination, exact for `Int` and `Ratio`
- [ ] 3.2 `inverse()` by exact elimination, with an error for a singular matrix
- [ ] 3.3 A matrix that is not square rejected before execution where its shape is known
- [ ] 3.4 Verify `m @ m.inverse()` is exactly the identity for `Int` and `Ratio` matrices

## 4. Directory
- [ ] 4.1 List `transpose`, `inverse`, `det` and `matrix` in the function directory
