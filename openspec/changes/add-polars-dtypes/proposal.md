# Proposal: The Polars dtypes become the representation

## Why

Omasheet holds exact numbers as `BigInt` and `BigRational`, and D11 in
`add-sheet-operations` rejected Polars for exactly that reason: no dataframe
library has an arbitrary-precision rational dtype. That decision keeps the
arithmetic exact and keeps Parquet, Polars and every dataframe tool at arm's
length.

The owner's decision, 2026-10-19, reverses it from the other end. Rather than
bridging two numeric worlds, **an omasheet type becomes a name for a Polars
dtype**: `Int` *is* `Int64`, `Decimal` *is* `Decimal(38, s)`, `Float` *is*
`Float64`. `num-bigint` and `num-rational` leave the tree. Exactness narrows
from rational to decimal, and in exchange omasheet can open any Polars
dataframe, read and write Parquet without a lossy conversion at the boundary,
and hand elementwise column work to Polars.

This is a breaking change to the language, not an addition. It wants a version
bump and a note in the README.

## What Changes

- **numerics** — MODIFIED: `Int` is `i64` and `Decimal` is a 38-digit
  fixed-point `i128`, both the dtypes Polars holds; arithmetic is exact where
  the result fits and says so where it does not; the last 8 of the 38 digits
  are guard digits, held but neither compared nor shown (D37); `Fraction`
  stays, no longer a representation but a display of a `Decimal`, the
  fraction recovered from the number (D38); REMOVED: arbitrary-precision
  `Int`, the `Complex` type
- **omx-expressions** — REMOVED: imaginary literals (`4i`, `Inf\i`) and the
  `re`, `im`, `conj` and `arg` functions; `x.Complex`;
  MODIFIED: `x.Bool` is `x.Boolean`, `x.Text` is `x.String`, `x.Num` is
  `x.Float`; ADDED: a `Duration` type, written as an
  ISO 8601 literal and inferred from the difference of two dates or times
- **file-format** — MODIFIED: the declarable type names are the Polars ones,
  and a blank cell is `Null` rather than a value called empty; a fraction of
  whole numbers and a whole number with a part are read as exact `Decimal`s,
  and a `Fraction` column shows what it holds as one

## Impact

- Every `examples/*.omx` that declares a type, and every `.expected` beside one
- `docs/format.md`, `docs/expressions.md`, `docs/interop.md`, `README.md` and
  `.claude/skills/omasheet/SKILL.md`
- Supersedes **D11** in `add-sheet-operations` and the Tech Stack and Design
  Principle 3 of `openspec/project.md`
- Unblocks `add-dataframe-interop`, which can then map a column to a Parquet
  type by naming it rather than converting it

## Open Questions

Both of the questions this proposal opened are settled, as D25 and D35: a
whole number past an `i64` widens to a `Decimal` and is only reported past
38 digits, and `--exact` writes every digit a value holds.

What remains open is narrower:

- A ratio of thirds is no longer inferred for a sequence, because squaring a
  rounded third does not give a ninth back. `examples/sequences.omx` is given
  its rule instead. Whether to infer a ratio within some tolerance is left
  open; it would be the first place exactness was traded for convenience.

## Out of Scope

- `Vector`/`List`, `Array`, `Struct`, `Categorical` and `Enum` dtypes, and the
  return of `Complex` as a `Struct{re, im}`: deferred, though a column list
  remains a required concept for the Polars operations that take several
  columns
- Parquet itself, dataframe-backed tables and lowering work to Polars: those
  are `add-dataframe-interop`
