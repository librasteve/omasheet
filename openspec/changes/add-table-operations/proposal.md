# Proposal: Table operations (Phase 3)

## Why

The Phase 1 core evaluates row-wise formulas, filters, aggregates and lookups on
its own column kernel. Summaries by category and combining tables — the work
pivot tables and `VLOOKUP` do in a grid spreadsheet — need grouping and joining,
which Phase 1 leaves out to keep the first deliverable small.

## What Changes

- **evaluation** — ADDED: grouping, joining, table operations preserve
  exactness, incremental recalculation
- **omx-expressions** — ADDED: partitioned cursor offsets
  (`Sales[; *-1 by Region]`); MODIFIED: the pipe operator gains the `group` stage

## Impact

- Depends on `add-omasheet-core`
- `omasheet-engine` gains native table operations over its exact column
  kernel; no dataframe library is added (design D11)
- No change to the `.omx` file syntax

## Out of Scope

- Polars or any other dataframe backend (design D11)

- Pivot-table declarations, sort and window functions beyond the cursor
- Relationship-following references (`Sales[CustomerID.Name; *]`) — design Q9
