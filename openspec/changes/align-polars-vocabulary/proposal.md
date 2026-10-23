# Proposal: Polars names for OMX

## Why

Polars is an unconditional dependency (D26) and `add-dataframe-interop` is
bringing it into evaluation. Someone who works in Polars, pandas or DuckDB
should be able to read an OMX pipeline without translating it, and the names
should be settled before `group_by`, `join` and `sort` are written on top of
them rather than renamed afterwards.

The owner's decision of 2026-10-23 takes Polars' vocabulary as OMX's: where
the two disagree about a name, the Polars name wins. It is a cutover, not a
migration — an old name is gone, and a file that uses one gets the ordinary
unknown-function diagnostic.

Three of the renames are not cosmetic. `count` is split, because Polars asks
two questions where OMX asks one with a single name. `round` keeps its
spelling and takes Polars' meaning, because `round(x, 2)` is legal in both
languages today and means different things in each — the one name a Polars
reader would get wrong silently rather than loudly. And `NaN` stops being
unordered, because its being unordered is what would keep a Float column off
the dataframe for the whole of `add-columnar-core`.

## What Changes

- **omx-expressions** — MODIFIED: `avg` is `mean`; `asin`, `acos` and `atan`
  are `arcsin`, `arccos` and `arctan`; `eom` is `month_end`; `to_zone` is
  `convert_time_zone`; `offset` is `base_utc_offset`; `zone` is `time_zone`.
  `count` counts values and the new `len` counts rows. `round(x, n)` is to
  `n` decimal places, and the nearest-multiple form is withdrawn. ADDED:
  `len`, `median`, `std`, `var`, `quantile`, `product`, `first`, `last`,
  `head`, `tail`, `shift`, `is_null`, `is_not_null`, `fill_null`, `is_in`,
  `cast`, `month_start`, `quarter`, `week` and `ordinal_day`
- **numerics** — MODIFIED: a `NaN` is ordered greatest and equal to itself, as
  Polars has it, rather than unordered as Raku has it. `Inf` and `-Inf` are
  unchanged
- **file-format** — MODIFIED: the `DateTime` type is written `Datetime`

The Excel bridge and the app follow the names without a spec delta: neither
capability's requirements name a function, so what changes there is code —
`AVERAGE` maps to `mean`, `ROUND(x, 2)` imports as `round(x, 2)` rather than
`round(x, 0.01)`, the function directory lists the new names, and the app's
column type menu is brought back into agreement with the types there are.

## Impact

- **Every `.omx` using a renamed function stops working**, with an
  unknown-function diagnostic. The 16 example files and the four documents
  under `docs/` are updated in this change; a user's own files are not, and
  there is no migration path offered
- `round`'s change of meaning is the one silent break: a file with
  `round(x, 100)` keeps compiling and changes answer. A two-argument `round`
  whose second argument is not a non-negative integer literal gets a
  diagnostic, which catches `round(x, 0.01)` but not `round(x, 100)`
- The NaN rule changes one cell of one golden (`examples/types.expected`, the
  `Same` column where `NaN == NaN` is now true) and two assertions in
  `omasheet-engine/tests/spec.rs`
- `Func::Avg` becomes `Func::Mean`, `Func::RoundTo` is deleted and
  `Func::RoundDigits` added, so the xlsx writer's formula mapping is touched
- No parser change: `name(a)`, `recv.name(a)` and `recv |> name(a)` already
  lower to one call, and no name is a lexer keyword

## Out of Scope

- `group_by`, `agg`, `join`, `sort`, `unique`, `n_unique`, `drop_nulls`,
  `reverse` and `over`: named in `add-sheet-operations-polars`, which
  implements them. A name enters the function directory in the change that
  implements it, so that every listed name is callable
- The `.str` namespace. OMX has no string functions at all, which is a
  question of scope rather than of naming
- The other type names. `Int` → `Int64` and `Float` → `Float64` belong with
  the other widths, in `add-polars-dtypes` section 5, where the width first
  tells the reader something. `Decimal`, `String`, `Boolean`, `Date` and
  `Time` are already the Polars spellings, and `Fraction` and `Percent` are
  display wrappers over `Dec` with no dtype to be named after
- Any lowering to Polars: `add-columnar-core`
