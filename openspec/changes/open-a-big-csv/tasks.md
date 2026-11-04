# Tasks: open a big CSV at once

Owner, 2026-11-01: `hold-less-of-a-big-file` items 2 and 3 are built first (step 1
here), before any work on CSV. The order matters: each step is measured against the file in `proposal.md`
before the next one starts, and steps 1 and 2 can be built and measured on
Parquet alone.

## 0. The figures this change is measured against

- [ ] 0.1 The test-file generator in the repository (seeded; 10 MB, 100 MB
      and 1.1 GB slices), and a release harness in `timings.rs` that takes
      wall time and peak memory for each target in `proposal.md`, with the
      page cache warm and with it dropped
- [ ] 0.2 The *before*: each phase of opening at 10 MB and 100 MB (read,
      `Grid`, `write_omx`, parse, evaluate, snapshot), with time and peak
      memory, so that the 29× is attributed to its phases rather than taken
      as a whole
- [ ] 0.3 `scan_csv → sink_parquet` on the 1.1 GB file, cold and warm: time,
      peak memory, size of the copy; and the effect of the `csv` feature (and
      the streaming engine, if it is a feature of its own) on build time and
      binary size. This is what decides whether D153's risk fallback is needed.
      Partly measured (design, *Measured*): on the 921 MB CSV, warm, 2.65 s
      and ~0.5 GB allocated, a 126 MB copy. Still to take: the 1.1 GB file,
      cold, and the build-time and binary-size cost

## 1. Prerequisites from `hold-less-of-a-big-file`

- [x] 1.1 Its item 2: `Value::Series` (D79), so that a column read stays a
      Polars column
- [x] 1.2 Its item 3: the snapshot formats only what the grid shows (D80).
      Recommended: the snapshot holds the frame and formats a cell when it
      is drawn, because the Qt model already asks only for visible cells
- [ ] 1.3 Measured on the NYC taxi Parquet file (41M rows): a `table` over it
      opens and scrolls in the app within the targets, which neither does
      today

## 2. Off the UI thread (D149, D150)

- [ ] 2.1 `rebuild` split into a synchronous compile and check, and an
      evaluation that runs on a worker, with generation numbers and results
      posted with `qt_thread().queue`
- [ ] 2.2 Elementwise columns computed for the visible rows only, and the
      whole column only when something asks for it
- [ ] 2.3 Pending and loading states in the grid and in the constants panel
- [ ] 2.4 The column cache (D150): owned by the `Document`, survives
      rebuilds, evicts LRU within a budget
- [x] 2.5 D155: an aggregate over a column that is not held runs its plan on
      `scan(path)` with `Engine::Streaming`, keeping D71a, D72 and the
      decimal guard; its answer is cached by file identity, column and
      function. Measured on the 611 MB Parquet file against today's path:
      time, and peak memory without the mapped file pages

## 2b. Found in the app, 2026-11-01 (D156, D157, D158, D159)

- [x] 2b.1 A selection's summary on a paged table reads the selected
      columns a block at a time, not every column as formatted cells
      (`08613f8`): 450,000 rows of the taxi file, 3 GB and killed → 284 ms
- [x] 2b.2 A table sort is a step to undo and redo (`2c1a795`)
- [x] 2b.3 D159: a column's filter lists at most 1,000 values and searches
      the rest in Rust (`2e65c8e`); the crash on a date-time column is gone
- [x] 2b.4 D157: the size a table takes held whole, estimated from its
      footer; held whole by a worker when under half of `MemAvailable`;
      otherwise a status-bar notice with the estimate. Measured: the
      estimate against the held size for both demo files (within 3%)
- [x] 2b.5 D158: sort, filter, the selection summary and Ctrl+Down as Polars
      operations over the column, with a differential test against today's
      answers (ties, nulls, NaN, decimals). Measured on short2: each ≤ 50 ms
      held, or off-thread with a pending state. Done 2026-11-02: sort is an
      `arg_sort` (`157eae5`), a filter a lowered mask
      (filter-by-expression), the summary added over the Polars arrays in
      the one-by-one order; each over 200,000 rows on a worker with the
      spinner (2b.6a). Ctrl+Down needed no Polars: its seconds were the
      summary of the selection it passed through, which now waits for the
      selection to settle (short2, release: 70–85 ms). Differential tests:
      `a_table_sorts_by_polars_in_the_order_it_sorts_by_values` (the edge
      file of NaN, ±0, nulls, ties, decimals),
      `a_paged_filter_shows_what_a_held_one_does`,
      `a_paged_summary_answers_as_a_held_one_reading_only_what_is_selected`
- [x] 2b.6 D156: the grid lays out one screen of rows from an integer top
      row, with its own scrollbar. Checked by eye at the last rows of the
      taxi file: gridlines straight, rows evenly spaced. Done 2026-11-02:
      `rowsView` is an `Item` with `contentY` as a number and a `Repeater`
      of one screen of rows placed within it; the wheel scrolls it and the
      scrollbar is bound to it both ways. At row 14,961,889 of the 15M-row
      CSV, rows and gridlines are even, which confirms the floats as the
      cause. The row-number gutter now widens with the last row's digits
- [x] 2b.6a Found 2026-11-02: Omarchy reported the app as not responding
      while the spinner turned, the main thread being held by a filter or
      a sort. Over 200,000 rows each now runs on a worker: the document
      makes a `FilterJob` (the frames and lowered expressions) or a
      `SortJob` (the column in the order shown), the worker runs it, and
      the result comes back by `filtered` / `sorted`; a filter overtaken
      by another is dropped, and a sort over a table changed meanwhile is
      done again. `Orders` holds `Arc`s so a job shares the order rather
      than copying it. Measured on short2, release: filter 95 ms, sort
      ~0.2 s, the main thread never more than 134 ms between ticks. Still
      on the main thread: the rebuild when a table arrives in memory or a
      copy is written (~40 ms on 15M rows), and a select-all summary
      (0.6 s on short2)
- [ ] 2b.7 Listing a column's distinct values in Polars (`unique`, then the
      first 1,000 in sort order), not a walk over every row
- [x] 2b.8 The probe in `Main.qml` taken out before this change is merged.
      Done 2026-11-02, in filter-by-expression; later probes were
      temporary and none is left

## 3. One funnel, first stage: big files (D147, D152, D153)

- [ ] 3.1 The reader interface: named `DataFrame`s, plus XLSX formulas.
      `parquet` is already one
- [ ] 3.2 `csv`: `sniff`, then `scan_csv` with that schema; OMX-only literal
      types converted per batch. First stage done (2026-11-02,
      `table::csv_schema`): Polars works the types out from the first
      10,000 rows, and a text column whose values there are all ISO dates
      or date-times is read as `Date` or `Datetime`; the sample and the copy
      share that schema. Polars' `try_parse_dates` was the first try and
      took 520 ms over the head alone. Still to do: `Datum::read`'s types
      (`20%`, `0x1F`, durations), and a cell that does not parse reported
- [ ] 3.3 `xlsx` and `json` building a `Series` per column directly, with no
      `Datum`; D154 for column-wide formulas
- [ ] 3.4 The conversion cache: hashed name, atomic write, `sink_parquet`,
      LRU within a disk budget. Done but the budget (2026-11-02): in
      `$OMASHEET_CACHE`, else `$XDG_CACHE_HOME/omasheet`, else
      `~/.cache/omasheet`; named by a hash of the canonical path, then the
      length and mtime; written to `.partial-<pid>` and renamed; a new copy
      takes away the CSV's earlier copies, its sample and any partial file
- [ ] 3.5 `dataframe["x.csv"]` (and `.xlsx`, `.json`) resolved by
      `table::read` to the cached copy; the engine reads Parquet alone.
      Done for CSV (2026-11-02): `table::located`, with `Copying::Wait`
      (the command line and tests convert first) or `Copying::Sample`
      (`Document::on_screen`, the app). Tested: sample, copy and a waited
      read give the same rows and types
- [ ] 3.6 The sniffed sample shown while the copy is written; loading rows
      beyond it. The sample done (2026-11-02): its first 1,000 rows, read
      from the file's head bytes and written as a small Parquet file, shown
      at once; `Document::to_convert` and `converted`; the app copies on a
      worker under the status-bar spinner and moves over when it is
      written. Not done: rows past the sample marked as loading (the table
      shows 1,000 rows until the copy arrives)

## 4. Opening (D146), and the funnel's second stage

- [ ] 4.1 The threshold, and the app opening any of the four formats above
      it as a table; the status bar says it is read-only and why. Done for
      CSV (2026-11-02): over 4 MB it opens as a table, with a note that
      says so and where the copy is kept. Measured, release, the 921 MB
      ride-hail CSV, cache empty: first screen 25 ms (sample 20 ms, open
      5 ms; was the inline import, which never finished), the copy 1.8 s
      (125 MB), reopening with the copy current 35 ms; in the app, the
      15M rows shown and held in memory (1.2 GB) within 4 s
- [ ] 4.2 "Copy into the document" for a table under the threshold
- [ ] 4.3 `omasheet import` above the threshold refuses and prints the
      `table` lines, ready to paste
- [ ] 4.4 The inline writer takes a frame, and `Grid`/`Datum` are deleted,
      only once every `examples/*.expected` and the interop tests are
      byte-identical. Where they are not (a column's scale, mixed-type
      columns, the text of a literal), the owner decides before this goes in

## 5. Measured against the targets

- [ ] 5.1 Open to first screen, 1.1 GB file, cold and warm: ≤ 50 ms
- [ ] 5.2 Once the copy is written, jump to the last row and scroll by a
      page: ≤ 16 ms a frame. Reopening with a current copy: ≤ 50 ms to a
      fully usable table
- [ ] 5.3 Edit a formula column: visible cells ≤ 50 ms; `Amount.sum()`
      first time off-thread, then ≤ 50 ms from the cache
- [ ] 5.4 Peak memory over a session that sums every numeric column:
      ≤ 150 MB + the columns' Arrow size, including during conversion
- [x] 5.5 By hand in the app: open, scroll, jump, edit, and confirm that the
      window never stops responding
      Done by hand, 2026-11-04 (owner).

## 6. Verification

- [ ] 6.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace`
- [ ] 6.2 The differential: for a corpus of CSV, XLSX and JSON files (quoted
      newlines, CRLF, a BOM, no final newline, empty cells, OMX-only
      literals), the inline sheet and the table over the converted copy
      answer the same for every column and every aggregate; and every
      aggregate gives the same answer on the streaming engine as on the
      in-memory one (D155), including NaN, null, empty and decimal columns
- [ ] 6.3 Every `examples/*.expected` byte-identical: a small file still
      opens inline as before
