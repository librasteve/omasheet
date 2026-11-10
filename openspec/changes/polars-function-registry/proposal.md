# Proposal: A Polars function registry

**Deferred until `add-units` is archived** (owner, 2026-11-10). Nothing here is built before then. The naming rules (N1–N9 in `design.md`) are the prelude, settled first so `add-units` can avoid colliding with them.

## Why

Each function that has a Polars version is written five times: `funcs.rs` (name and directory entry), `check.rs` (arguments and result type), `eval.rs` (the scalar path), `lower.rs` (the Polars path) and `xlsx/write.rs`. The two evaluation paths must agree, and they can only be made to by checking them against each other. The functions a ledger or time series needs most (`cum_sum`, `diff`, `rolling_mean`, `rank`, `forward_fill`) are missing, and each would cost five edits and a disagreement risk.

The owner's rule (2026-11-10): these functions are implemented as their Polars versions.

## What Changes

- **omx-expressions**: ADDED. A registry of Polars-backed functions: one entry per function with its OMX name, its Polars method, its argument kinds, its result-type rule and the dtypes it refuses. Adding a function is adding an entry.
- **omx-expressions**: ADDED. Where it runs: on a table, in the lazy plan; on a plain sheet, on a temporary one-column frame built from the column and read back. The Polars version is the only implementation, so the two paths agree by construction.
- **omx-expressions**: ADDED. An order key and a partition for the functions that have a window: `Amount.cum_sum(Date by Account)`, lowering to `.over(partition_by=Account, order_by=Date)`.
- **omx-expressions**: ADDED, as the first users: `cum_sum cum_min cum_max cum_prod cum_count`, `diff`, `pct_change`, `rank`, `forward_fill`, `backward_fill`, `rolling_sum rolling_mean rolling_min rolling_max rolling_std`, then `n_unique`, `clip`, `is_between`, `any`, `all`.
- **omx-expressions**: ADDED. The registry covers **sheet stages** as well as column functions, and the first stages in it are the join family that `join(Other, keys)` lacks:
  - `join_asof(Other, key)`: each row takes the row of `Other` with the nearest key at or below it (`backward`, the default), at or above it (`forward`) or the closest (`nearest`); `by` partitions both sheets; a tolerance is an optional last argument. This is the commission-rate lookup (`Sales |> join_asof(Commission_Rates, Sales_Amount, Sales_Target)`), done in one pass and without a filter per row
  - `join_where(Other, condition)`: the rows of the pair where an inequality condition is true (Polars' inequality join), for ranges the as-of join does not cover
  - `join(Other, keys, how)`: `how` widened to Polars' `semi`, `anti`, `full` and `cross`, next to the `left` that exists
- **omx-expressions**: Later candidates, one line each in the design's list, not part of the first build: `group_by_dynamic` and `rolling` (time windows), `upsample`, `unique`, `top_k`/`bottom_k`, `explode`, `pivot`/`unpivot`, `with_row_index`.
- **omx-expressions**: A type Polars cannot hold (`Complex`, `Scientific`) is refused with an error naming the function, not passed to Polars.
- The Cargo features each family needs are switched on one family at a time.

## Impact

- `omasheet-omx`: `funcs.rs`, `check.rs` (the registry replaces per-function arms)
- `omasheet-engine`: `lower.rs`, `eval.rs`, `dtype.rs`/`copyframe.rs` (the temporary frame)
- `Cargo.toml`: Polars features (`join_asof` is behind the `asof_join` feature; `join_where` behind `iejoin`, to be checked against the pinned version)
- `docs/expressions.md`, `skill/omasheet/SKILL.md`, examples

## Out of Scope

- Every Polars function by reflection; the registry is curated
- Migrating the existing `is_*` and text functions (a later step, optional)
- Keyword arguments in the language (D57)
