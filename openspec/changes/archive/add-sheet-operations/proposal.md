# Proposal: Sheet operations (Phase 3)

> **Superseded**, and nothing of it was built. The decision of 2026-10-23
> puts the dataframe at the centre of evaluation, where this change had
> Phase 3 on a column kernel of omasheet's own and ruled a dataframe out
> (D11). What replaces it: `align-polars-vocabulary` for the names,
> `add-columnar-core` for the columns the operations will run over, and
> `add-sheet-operations-polars` for the operations themselves. Its two
> open questions go to the last of those — the `by <Column>` clause (P16)
> and multiple matches on a lookup (Q10) — and D11 is superseded by
> `add-polars-dtypes`.

## Why

The Phase 1 core evaluates row-wise formulas, filters, aggregates and lookups on
its own column kernel. Summaries by category and combining sheets — the work
pivot sheets and `VLOOKUP` do in a grid spreadsheet — need grouping and joining,
which Phase 1 leaves out to keep the first deliverable small.

## What Changes

- **evaluation** — ADDED: grouping, joining, sheet operations preserve
  exactness, incremental recalculation
- **omx-expressions** — ADDED: partitioned cursor offsets
  (`Sales[; *-1 by Region]`); MODIFIED: the pipe operator gains the `group` stage

## Impact

- Depends on `add-omasheet-core`
- `omasheet-engine` gains native sheet operations over its exact column
  kernel; no dataframe library is added (design D11)
- No change to the `.omx` file syntax

## Out of Scope

- Polars or any other dataframe backend (design D11)

- Pivot-sheet declarations, sort and window functions beyond the cursor
- Relationship-following references (`Sales[CustomerID.Name; *]`) — design Q9
