# Proposal: open a big CSV at once

## Why

Measured 2026-11-01, release CLI, on this machine (8 cores, 10 GB RAM): a
1.1 GB CSV of 20.8 million rows and seven columns cannot be opened. Peak
memory runs at about 29× the size of the file, at every size measured:

| CSV | import | lint | print | peak |
|---|---:|---:|---:|---:|
| 10 MB, 211k rows | 1.4 s | 1.0 s | 1.4 s | 0.37 GB |
| 100 MB, 2.07M rows | 21.8 s | 9.6 s | 16.6 s | 3.4 GB |
| 1.1 GB, 20.8M rows | killed by the 7 GB cap after 69 s | — | — | — |

The cost comes from how the file is handled, not from the code doing it, so
tuning that code will not fix it. Opening a CSV in the app is
`csv::import`, then `document()` on the result, and every row passes through
four forms in turn: the bytes, then a `Vec<Vec<Datum>>` with a `String` for
every cell, then `.omx` source about 1.4× the size of the CSV, then the
parsed program, and finally a `Value` for every cell (about 150 bytes each,
by `hold-less-of-a-big-file` 0.1), with the snapshot formatting every cell
on top of that. Opening a `.parquet` file in the app takes the same path
(`parquet::import` writes the rows out as text), so a big Parquet file fails
the same way, even though a `table` over that same file opens in 18 ms.

The owner also asks for interactive responses well under 100 ms. Every
rebuild today runs on the UI thread and reads the whole file again, so a
file of this size would freeze the window even if it fitted in memory.

## What Changes

- **interop** — MODIFIED: every importer (CSV, XLSX, JSON, Parquet) reads
  into Polars `DataFrame`s, and two shared writers take it from there: an
  inline `.omx` sheet for a small file, or a Parquet copy and a `table` over
  it for a big one (D152)
- **app** — MODIFIED: opening any of the four formats above a size threshold
  makes a new, unsaved document holding a `table`, rather than writing the
  file's rows into the document as text. Below the threshold, opening works
  as it does today (D146)
- **format** — MODIFIED: `dataframe["…"]` may name a CSV, XLSX or JSON file.
  The engine reads it from a Parquet copy in the cache, written by a
  streaming conversion, so the engine still reads Parquet alone (D153)
- **app** — MODIFIED: nothing that reads the whole file runs on the UI
  thread. A cell whose value needs the whole file shows as pending and is
  filled in when the answer arrives (D149). A column read whole is cached
  across rebuilds (D150)
- **engine** — MODIFIED: an aggregate over a table column that is not held
  runs in the Parquet scan on Polars' streaming engine, and its answer is
  cached, rather than reading the whole column first (D155)
- **app** — MODIFIED: the grid formats only the rows it shows. This is
  `hold-less-of-a-big-file` item 3 (D80), which this change depends on
- **app** — MODIFIED: a table that fits in memory is held whole, read in
  the background once it has opened, and one that does not says so (D157).
  Sort, filter, the selection summary and Ctrl+Down run as Polars
  operations over a column (D158). The grid lays out only the rows on
  screen, so a table of millions of rows draws straight to its last row
  (D156). A column's filter lists at most 1,000 values (D159, done)

## Targets

Each target is measured on the 1.1 GB file from *Why*, with the page cache
cold and warm:

| | target |
|---|---|
| Open, from the file chosen to the first screen drawn | ≤ 50 ms |
| Scroll or jump to any row, once the copy is written | ≤ 16 ms a frame |
| While the copy is being written: rows past the sample | shown as loading, never blocking |
| Reopening, with a current copy in the cache | ≤ 50 ms to a fully usable table |
| The copy written in the background | measured by task 0.3; seconds, bounded memory |
| Edit a cell or a formula, visible cells redrawn | ≤ 50 ms |
| An aggregate over a whole column, first time | one column decoded from the copy; off the UI thread |
| The same aggregate again, or after any edit | ≤ 50 ms, answered from the cache |
| Peak memory | ≤ 150 MB + the columns held whole, each about its Arrow size (8 B/row for a number) |

No design can answer the first aggregate over 20.8 million rows read from
CSV in under 100 ms: parsing 1 GB of text takes about a second at best. What
this change guarantees is that the window never waits for that parse.

## Impact

- `omasheet-interop`: a reader interface (named frames, plus XLSX
  formulas); `csv::sniff`; the two writers; the conversion cache. In the
  second stage `Grid` and `Datum` go, and `names` and the notices are
  shared by every format rather than called from four places
- `omasheet-engine`: `table::read` resolves a non-Parquet source to its copy;
  a column cache owned by the `Document`; `doc.rs` (an asynchronous rebuild,
  and a snapshot that formats on demand). No CSV code in the engine
- `omasheet-ui`: `import_file` chooses between a table and an inline sheet;
  the grid gets pending and loading states
- `omasheet-engine`: `eval.rs`'s aggregate builds its plan on the scan when
  the column is not held, and an answer cache beside the column cache (D155)
- `Cargo.toml`: Polars' `csv` and `new_streaming` features. Their effect on
  build time and binary size is measured (task 0.3)
- No change to what a file means or to any number it answers. The inline
  sheet and the table over the copy give the same answers, and a
  differential test holds them to that

## Out of Scope

- Editing a table's rows. A table is read-only (D49), so a big CSV opened
  this way is read-only too. The app offers "copy into the document"
  for one that fits
- Writing a big CSV back out, and `omasheet print` of 20 million rows
  (print can stream batches later, as a change of its own)
- Gzipped CSV, and a CSV that is changing while it is open (it is re-read
  when its mtime changes)
- Predicate pushdown and row-group statistics, as in `hold-less-of-a-big-file`

## Depends on

`hold-less-of-a-big-file` items 2 (`Value::Series`, D79, decided and not yet
built) and 3 (the snapshot, D80, open). Without item 2, a `mean` over 20.8M
rows is 3 GB of values. Without item 3, the snapshot alone is 145M formatted
cells. Both can be built and measured on Parquet today, before any CSV work
starts.
