# Proposal: Parquet, and tables backed by a dataframe

## Why

People who work in Polars, pandas or DuckDB exchange sheets as Parquet, which
carries column types where CSV carries only text. Bringing such a sheet into a
file, and handing a file's sheets back, should not lose those types.

This was deferred from `add-interop` (Phase 2) on 2026-10-09, because the
Parquet crates take the `omasheet` binary from about 1.7 MB to about 8-10 MB
and the shape of any Polars integration was undecided. Both questions are now
answered. `add-polars-dtypes` makes an omasheet type a name for a Polars dtype,
so a column maps to a Parquet type by being named rather than converted, and
the owner's decision of 2026-10-19 takes Polars as an unconditional dependency
and asks for three things from it: the file format, tables backed by Parquet
files, and elementwise column work.

## What Changes

- **parquet-interop** — ADDED: Parquet in both directions, with each column's
  type taken from the schema rather than inferred from the values
- **cli** — ADDED: `.parquet` in `omasheet import`, `omasheet export
  --parquet`, and `--meta` to carry what no format round-trips today
- **file-format** — ADDED: `table Sales := dataframe["data/*.parquet"]`, a
  sheet whose rows live in Parquet files, with its schema kept in the `.omx`
  text
- **omx-expressions** — ADDED: `.head(n)`, a selection of the first `n` rows
- **evaluation** — ADDED: row and column selections pushed down to Polars, and
  elementwise computed columns lowered to Polars expressions; MODIFIED: a
  row-recursive column stays on the engine

## Impact

- Depends on `add-interop` for the `omasheet-interop` crate and its `.omx`
  writer, and on `add-polars-dtypes` for the types
- `polars` as an unconditional workspace dependency: no Cargo feature, no
  `cfg` branches. Every build and every CI run pays the compile cost, the
  shipped binary grows from about 1.7 MB, and the `curl | bash` install line
  gets slower
- `Sheet` gains a source and `ColKind` a frame variant, so every match over
  them is touched, in the engine, in the interop crate and in the app
- The app's editing API is written against cells that have spans in the text.
  A dataframe-backed table has no such cells, so those calls must refuse or
  divert

## Open Questions

- What `--meta` carries exactly, and whether an importer that does not know it
  should see a usable file without it
- Whether a dataframe-backed table is editable at all, or read-only until a
  later change
- Which compression codecs to read. Snappy and Zstandard cover the defaults of
  pandas, Spark, Polars and DuckDB

## Out of Scope

- Arrow IPC and Feather; nested columns, which load with a notice
- Writing back to the Parquet files a table reads
- Lowering group, join and sort to Polars: those stay on the native kernel
  from Phase 3 until there is a reason to move them
