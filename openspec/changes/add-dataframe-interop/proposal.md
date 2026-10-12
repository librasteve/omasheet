# Proposal: Parquet and dataframe interop (later phase)

## Why

People who work in Polars, pandas or DuckDB exchange sheets as Parquet, which
carries column types where CSV carries only text. Bringing such a sheet into a
file, and handing a file's sheets back, should not lose those types.

Deferred from `add-interop` (Phase 2) by owner decision, 2026-10-09: the Parquet
crates take the `omasheet` binary from about 1.7 MB to about 8-10 MB, and the
shape of any Polars integration is not yet decided.

## What Changes

- **parquet-interop** — ADDED: Parquet for one sheet of typed columns in both
  directions, with terminating `Fraction` columns kept exact as decimals
- **cli** — ADDED: `.parquet` in `omasheet import`, and `omasheet export --parquet`

## Impact

- Depends on `add-interop`: it adds a format to the `omasheet-interop` crate and
  reuses its `.omx` writer, giving it column types from the Parquet schema
- `parquet` and `arrow` crates, as a Cargo feature so that a build without them
  stays small

## Open Questions

- Whether Polars is wanted at all beyond the file format, and for what: a
  library API that hands sheets to a Polars `DataFrame`, a Python binding, or
  nothing more than Parquet. Polars cannot hold `Int` or `Fraction` exactly (D11
  in `add-sheet-operations`), so any such bridge is lossy for those columns.
- Whether the Cargo feature is on by default.
- Which compression codecs to read. Snappy and Zstandard cover the defaults of
  pandas, Spark, Polars and DuckDB; all codecs add about 2 MB more.

## Out of Scope

- Arrow IPC, Feather; Parquet datasets of several files; nested columns
