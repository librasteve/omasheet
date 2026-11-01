# Design: open a big CSV at once

## Context

Source: the conversation of 2026-11-01, on the `speedup` branch, after
`fc21f34` (a row range reaches the Parquet reader). D numbers continue from
D145. The measurements are in `proposal.md`. The test file is synthetic: 20.8M
rows of `id,date,region,product,qty,price,amount`, made by a seeded script,
which task 0.1 adds to the repository so the figures can be taken again.

Revised the same day at the owner's suggestion: every importer goes through
the frame Parquet already uses, so that the engine reads only one source
format (D152). An earlier draft gave the engine a CSV source of its own, with
a CSV schema reader and a row index. Both are gone, and D148 and D151 are
replaced by D153.

## The shape of it

```
csv  ─ scan_csv, streaming, schema from sniff ─┐
xlsx ─ calamine → a Series per column ─────────┤             ┌─ small: frame → inline .omx (editable)
json ─ → a Series per column ──────────────────┼─ DataFrame ─┤
parquet ─ already a frame ─────────────────────┘             └─ big: frame → Parquet in the cache → table
                                                                    (a big .parquet: table over the file itself)
```

The engine never reads a CSV. `table Big := dataframe["big.csv"]` names the
CSV, so the `.omx` stays portable, and `table::read` resolves the name to
the cached Parquet copy, converting the file first if there is no copy. From
then on it is the existing path: footer row count, one column at a time, a
range read by row group (D78).

## Decisions

| | | |
|---|---|---|
| D146 | Decided | **When opening a file makes a table rather than writing its rows inline.** Owner decision, 2026-11-01: above 4 MB, or more than 100k cells, opening makes a table; below that, rows are written inline as today, so a small CSV stays editable. The threshold is one constant, and the status bar says which happened ("read-only: rows are read from big.csv"). The same rule applies to `.parquet`, `.xlsx` and `.json`. The CLI's `import` is unchanged (it exists to write `.omx` text), except that above the threshold it refuses and prints the `table` lines instead |
| D147 | Recommended | **A CSV's types are sniffed once, when it is first converted.** `csv::sniff` reads the header and runs `Datum::read` over a 64 KB sample, so a column gets the type `import` would give it today. That schema is given to `scan_csv` (`with_schema`) and written as the table's schema lines. Polars parses Int, Float, String and ISO dates itself. An OMX-only literal (`20%`, `0x1F`, a duration) is read as String and converted by the engine's own parser as each batch streams through. A cell that does not parse is null in the copy, and is reported once per column |
| D149 | Recommended | **Evaluation runs off the UI thread, and the grid computes only the rows it shows.** `rebuild` is split in two. Compiling and checking the `.omx` text stays synchronous (≤ 10 ms). Everything that reads a source runs on a worker, and its results are posted back with `qt_thread().queue`, as the theme watcher already does. A cell of an elementwise column is computed for the visible rows alone. A value that needs the whole column shows as pending. Each rebuild has a generation number, and a late answer from an older one is dropped |
| D150 | Recommended | **A column read whole is cached across rebuilds, within a memory budget.** The cache is keyed by (Parquet path, size, mtime, column, dtype), belongs to the `Document`, and holds the Polars column (needs D79). Entries are evicted least recently used beyond a quarter of physical RAM, and an evicted column is read again from the Parquet file, which is now cheap (one column, decoded) rather than a parse of the whole CSV. A range read is never cached as if it were the whole column (D78's rule) |
| D152 | Recommended | **One funnel: every reader produces a frame, and two writers take it from there.** `csv`, `xlsx`, `json` and `parquet` each turn their input into named `DataFrame`s, plus, for XLSX only, the formulas found. Two writers share everything after that: `frame → inline .omx` and `frame → Parquet → table`. Column naming (`names`, `name_for`), the notices and the threshold live in one place, not four. Done in two stages: first, big files go through the frame alone; then the inline writer moves to the frame and `Grid`/`Datum` are deleted, but only once `examples/*.expected` is byte-identical (task 4.4), because the frame drops what `Datum` keeps (the text a literal was written as, and a column whose cells have several types) |
| D153 | Recommended | **A converted copy lives in `$XDG_CACHE_HOME/omasheet/`.** Named by a hash of the canonical path, size and mtime. It is written by Polars' streaming `sink_parquet`, so memory stays bounded whatever the file's size, and atomically (a temporary file, then renamed into place), so a half-written copy is never read. While it is being written, the grid shows the sniffed sample rows and marks the rest as loading. A reopened file whose copy is still current opens as fast as a Parquet table (18 ms measured). Copies are evicted least recently used beyond a disk budget (recommended: 10 GB or 10% of free space, whichever is smaller). A `.parquet` file is never copied |
| D154 | Recommended | **XLSX formulas over a big sheet.** A formula that is the same down a whole column (`column_formula` already finds these) becomes a `:=` line on the table, as `Family` is in `analysis.omx`, and its values are left out of the copy. A sheet with formulas that differ from cell to cell stays inline at any size, since a table cannot hold them. XLSX's 1,048,576-row limit keeps such a sheet within reach of the inline path |

## Why not make the inline path leaner

Halving the `Datum` and streaming `write_omx` would still leave 145M cells
of `.omx` text to parse and 145M `Value`s to hold. At about 150 B a value
that is 21 GB, so any version of writing the rows inline is ruled out above
a few million cells. What omasheet already does well is a `table` over
Parquet, so every big file is made into one.

## Risks

- **The first jump to the end waits for the conversion.** The copy cannot be
  read until its footer is written. Task 0.3 measures the streaming
  conversion. If it misses the targets, the fallback is to write the copy as
  numbered part files and read each one as soon as it is finished. That
  breaks "a source names one file" (`add-dataframe-interop` 3.6), so it is
  the fallback rather than the plan
- **A sample that misses the type of later rows.** A column of integers for
  64 KB that then has `3.5`. The copy holds a null and a notice is shown,
  and the status bar offers to widen the type, which converts the file
  again. This differs from `import`, which reads every row, so the
  differential test compares only files whose types the sample catches
- **A column has one type in a frame.** A CSV column that mixes numbers and
  text, which an inline sheet keeps cell by cell, is stored as String in the
  copy, with a notice. That is why the inline writer moves to the frame
  only once the examples are identical (D152)
- **A stale copy.** Keyed by size and mtime, which a tool can preserve while
  rewriting a file. A cheap check (hash of the first and last 64 KB) is
  added to the key
- **Threads and Qt.** The `Document` moves to the worker or is shared as an
  `Arc`, and results come back through the Qt thread queue. Nothing from Qt
  is touched off the UI thread
