# Proposal: a columnar core

## Why

Polars is an unconditional dependency and earns none of that cost: it reads
and writes files at the edge of `omasheet-interop` and touches no evaluation.
A table's rows are read as a `DataFrame` and then exploded into one `Cell` per
value, the frame dropped, before the engine starts; evaluation is a scalar row
loop with a memo slot per cell. So a sheet of a million rows pays for Arrow
twice and benefits from it nowhere.

The owner's decision of 2026-10-23 is to put the dataframe at the centre.
The engine keeps what Polars cannot do — row-recursive columns, the exact
decimal edges, a value with no dtype — and hands it the rest.

## What Changes

- **evaluation** — ADDED: a sheet's data is held as Polars columns; an
  elementwise computed column is lowered to one Polars expression; a row or
  column selection becomes a filter, a projection or a limit; an aggregate
  becomes a Polars aggregate. MODIFIED: a row-recursive column stays on the
  engine, and a lowered group that fails falls back to it wholly
- **numerics** — MODIFIED: `Time` is held in nanoseconds and `DateTime` in
  microseconds, as Polars holds them, so that no sub-second value is lost at
  the Parquet boundary. ADDED: neither `/` nor `mean` is lowered on an exact
  column, the two places Polars' decimal arithmetic and `Dec` disagree
A `table`'s columns arriving as Polars columns, and a zoned date-time being a
Polars dtype after all, need no delta of their own: the `evaluation`
requirements above are where both are said, under *A sheet's data is held as
columns* and *What is not lowered*.

## Impact

- **`omasheet-engine` takes the `polars` dependency, and the `ReadSources`
  inversion is dropped** (D62). `omasheet-interop/src/frames.rs` moves to
  `omasheet-engine/src/table.rs`; `omasheet-omx` stays dependency-free
- `lazy` joins the Polars features. It pulls `polars-lazy`, `polars-plan`,
  `polars-expr` and `polars-mem-engine`, none of which are compiled today, so
  the binary and the build both grow; measured per task 1.2
- `ColKind::Frame` loses its payload and `Engine` gains a frame store, so
  every match over them is touched — in the engine, the interop crate and the
  app
- `Value::Time` and `Value::DateTime` change unit. `+ 1` still means one
  second, which is what the goldens for `datetimes` and `zones` check
- The app's per-keystroke rebuild is the sharpest risk: a `collect()` is
  slower than the row loop for a ten-row sheet, which is omasheet's typical
  document. A row-count threshold (D73) and a column-wise snapshot (D75)
  are the mitigations, and the threshold is measured rather than guessed

## Out of Scope

- `group_by`, `agg`, `join` and `sort`: `add-sheet-operations-polars`, which
  needs this change
- A viewport-driven snapshot — formatting only the rows the grid shows. It is
  what actually makes a large table viewable, it is independent of this work,
  and it is probably worth more per unit of effort for the app
- Incremental recalculation, which `add-sheet-operations` carried and which
  has no consumer: with a columnar core the per-keystroke cost is dominated
  by the recompile and the snapshot
- Driving Polars' parallelism from inside evaluation. `Engine` is `RefCell`
  and `Rc` throughout and stays single-threaded; Polars parallelises inside
  `collect()`, which is enough

## Supersedes

- `add-dataframe-interop` sections 4 and 5 move here, and its task 1.1 — which
  claims the `lazy` feature is already on — is struck: `Cargo.toml` has no
  `lazy` and `polars-lazy` is not compiled
- `add-sheet-operations` is superseded wholly. Nothing of it was built, and
  its "on Omasheet's own engine" is now the reverse of the decision. Its open
  questions carry to `add-sheet-operations-polars`: the `by <Column>` clause
  (P16) and multiple matches on a lookup (Q10)
