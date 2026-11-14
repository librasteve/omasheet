# Proposal: `Decimal(s)` is a Polars decimal, and `Fraction` is the exact rational

**Draft for review.** Nothing here is built. It follows `polars-function-registry`, whose windowed columns it lets lower for `Decimal` too, and it reverses parts of `add-polars-dtypes` (D15, D17, D25) on purpose; see the Why.

## Why

`Decimal` is two things at once. In a dataframe it is `Decimal(38, s)`, with one scale `s` for the whole column. In the engine it is a `Dec`, a 38-digit mantissa with a scale **per value** that moves its point when a result needs more digits. The two disagree in ways that cost speed and a great deal of guarding code:

- `lower.rs` guards every exact `+`, `-` and `*` with a per-row test that the result fits, hands the whole column back to the engine when any row might not, and refuses division and the mean of exact numbers outright because a Polars quotient has `max(x, y) + 4` digits (capped at 21) where a `Dec` has up to 30.
- A column of money and a column of rates cannot say how many places they keep. The scale is whatever each cell happens to be.
- A Parquet `Decimal(p, s)` column has a scale; an omasheet `Decimal` does not, so the scale is chosen at the boundary.
- The exact rational (`Fraction`) was removed by `add-polars-dtypes` D17 because no dataframe dtype holds one. That cost the language its one type that is exact for `1/3`, and left `Fraction` as a display dress over a `Decimal`.

The owner's decision (2026-11-14) is to stop making one type do both jobs:

1. `Decimal` becomes a standard Polars `Decimal128`, `Decimal(38, s)`, with the scale **chosen per column** (`Decimal(2)`; bare `Decimal` is `Decimal(10)`). It is the type a dataframe holds, so everything over it **lowers**.
2. `Fraction` returns as the engine-only exact rational, arbitrary precision (Raku's `FatRat`): a googol at one end and 1 / the atoms in the universe at the other. `Decimal(10)` holds up to about 10²⁸, so a million trillion (10¹⁸) fits with room to spare. It never lowers, and it is the place for anything that needs more than 38 digits or an exact quotient.

> Fast and fixed by default; exact and unbounded by explicit choice.

## What Changes

- **numerics**: CHANGED. `Decimal` is `Decimal(38, s)`, the scale `s` a property of the column (0 to 38, default 10). The point no longer moves; a value has the column's scale. A result is rounded to a scale (half away from zero, as `round` is), and one that does not fit in 38 digits is an error cell naming `Fraction`.
- **numerics**: ADDED. `Fraction`: an exact rational of arbitrary-precision integers, always reduced, its denominator positive. Never lowered. `Integer` fits in `Decimal(s)`, `Decimal(s)` fits in `Fraction`; `Fraction` beside `Float` is `Float`.
- **numerics**: CHANGED. Mixed scales: `Decimal(a)` beside `Decimal(b)` is `Decimal(max(a, b))`, exact; `Decimal` beside `Fraction` is `Fraction`. Division, and the mean of a `Decimal`, give the left operand's scale, rounded.
- **numerics**: CHANGED. A number past 38 digits is no longer a `Decimal` with a moved point. `2 ** 64` is `18446744073709551616` still (it fits, 20 digits); `10 ** 40` is an error cell offering `Fraction`, or is a `Fraction` where a `Fraction` is asked for.
- **file-format**: ADDED. `Decimal(n)` as a column type, and the inferred type of an undeclared column: the smallest scale that holds every cell (up to 18), else `Fraction`.
- **file-format**: CHANGED. A declared column that is given more places than it keeps is refused by cell, naming `Decimal(n)` or `Fraction` that would hold it.
- **parquet-interop**: CHANGED. A `Decimal(p, s)` column is read with its own scale `s` and written as `Decimal(38, s)`. A `Fraction` column is written losslessly as a struct of two strings, `num` and `den`, with its kind in the file's metadata (as `Mass` is); `--decimal` writes the nearest `Decimal(38, s)` or `Float64` instead and reports what was cut.
- **csv-interop, json-interop, xlsx-interop**: CHANGED. A `Fraction` is written as `n/d` text in CSV and JSON, and as its rounded double in XLSX with a note.
- **evaluation**: CHANGED. **Every operation over `Integer`, `Decimal(s)`, `Percent`, `Float`, text and the temporal types lowers**, including `/` and `mean`. Only `Fraction` and a decimal power other than a small whole one stays on the engine. Mixed scales are lowered as casts to the common scale, with an explicit rounding after `*` and `/`. The per-row overflow guard stays; its fallback is the `Fraction` engine path, which gives the true answer or the same error.
- **evaluation**: CHANGED. The window functions of `polars-function-registry` lower for `Decimal(s)` as they do for `Integer`.
- **Dependencies**: `num-bigint` and `num-rational` return (D14 removed them), for `Fraction` only.
- **Behaviour change to review**: a result that used to keep 30 digits (`1/3`) now keeps the column's scale (0.3333333333 at the default). Anyone who wants the exact value writes `Fraction`. Examples whose `.expected` shows more than 10 places change.

## Impact

- `omasheet-omx`: `types.rs` (`S::Decimal` carries a scale, `S::Fraction`), `decimal.rs` (`Dec` becomes the carrier of one `Decimal(s)` value), `check.rs` (the result-type and assignability rules, inference), `colfmt.rs`, the lexer and parser for `Decimal(n)`, `funcs.rs`
- `omasheet-engine`: `value.rs` (arithmetic and rounding rules, `Value::Fraction`), `lower.rs` (the guard, mixed scales, division, mean), `dtype.rs`, `eval.rs`, `doc.rs`
- `omasheet-interop`: `parquet.rs`, `csv.rs`, `json.rs`, `xlsx/`
- `omasheet-ui`: the column-type picker and the schema editor gain `Decimal(n)` and `Fraction`
- `Cargo.toml`: `num-bigint`, `num-rational`
- `docs/expressions.md`, `docs/format.md`, `docs/interop.md`, `skill/omasheet/SKILL.md`, `openspec/project.md` (principle 3), examples and their `.expected`

## Out of Scope

- A faster `Fraction` for small values (an inline `i128 / i128` arm); the type is `BigRational` first and measured
- `Decimal256` (Polars' wider decimal) and a precision other than 38
- Changing `Float`, `Scientific`, `Quantity` or `Complex`
- A display scale separate from the storage scale (formats already do that)
