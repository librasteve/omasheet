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
- **omx-expressions**: A type Polars cannot hold (`Complex`, `Scientific`) is refused with an error naming the function, not passed to Polars.
- The Cargo features each family needs are switched on one family at a time.

## Impact

- `omasheet-omx`: `funcs.rs`, `check.rs` (the registry replaces per-function arms)
- `omasheet-engine`: `lower.rs`, `eval.rs`, `dtype.rs`/`copyframe.rs` (the temporary frame)
- `Cargo.toml`: Polars features
- `docs/expressions.md`, `skill/omasheet/SKILL.md`, examples

## Out of Scope

- Every Polars function by reflection; the registry is curated
- Migrating the existing `is_*` and text functions (a later step, optional)
- Keyword arguments in the language (D57)
