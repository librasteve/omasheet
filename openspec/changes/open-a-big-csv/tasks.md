# Tasks: open a big CSV at once

Owner, 2026-11-01: `hold-less-of-a-big-file` items 2 and 3 are built first (step 1
here), before any work on CSV. The order matters: each step is measured against the file in `proposal.md`
before the next one starts, and steps 1 and 2 can be built and measured on
Parquet alone.

## 0. The figures this change is measured against

- [x] 0.1 The test-file generator in the repository (seeded; 10 MB, 100 MB
      and 1.1 GB slices), and a release harness in `timings.rs` that takes
      wall time and peak memory for each target in `proposal.md`, with the
      page cache warm and with it dropped
      Built 2026-11-07: `omasheet-engine/examples/gen_big_csv.rs` and
      `demo_data/generate.sh` (211,000 / 2,070,000 / 20,800,000 rows; 10, 103
      and 1,060 MB; each a prefix of the next); the harness is
      `the_targets_on_a_synthetic_csv`. It takes open, copy, reopen and the
      first and second `sum`; scroll, jump and edit (5.2, 5.3) are frame
      times and stay for the app. Taken 2026-11-07, release, one run each,
      cold with `purge` first (1.1 GB, ms; peak MiB):

      | step | warm | cold | peak MiB |
      |---|---:|---:|---:|
      | open (first screen) | 27 | 57 | 3.2 |
      | copy | 1,999 | 2,955 | 383 (236 MB written) |
      | reopen, copy current | 14.7 | 30.6 | 158.7 |
      | `T.qty.sum()`, first | 110 | 156 | 476 |
      | `T.qty.sum()`, again | 89 | 99 | 317 |

      Against the targets: cold open is over 50 ms (57), and the second
      sum is 89–99 ms where the cache should answer in ≤ 50. The 100 MB
      figures are in the same run and scale with the file

- [x] 0.2 The *before*: each phase of opening at 10 MB and 100 MB (read,
      `Grid`, `write_omx`, parse, evaluate, snapshot), with time and peak
      memory, so that the 29× is attributed to its phases rather than taken
      as a whole

      Measured 2026-11-07, release, warm, one run each, by
      `where_opening_a_csv_spends_its_time_and_memory` in
      `omasheet-interop/tests/timings.rs` (`csv::import` taken apart, then
      what `Document` does). Peak is the counting allocator's above what
      the phase began with; held is what is held when it ends:

      | phase | 10 MB s | peak MiB | held MiB | 100 MB s | peak MiB | held MiB |
      |---|---:|---:|---:|---:|---:|---:|
      | read | 0.004 | 9.9 | 9.9 | 0.030 | 98.6 | 98.8 |
      | records, `Datum::read` | 0.343 | 59.5 | 69.4 | 3.249 | 575.0 | 673.8 |
      | `Grid::from_rows` | 0.000 | 0 | 69.4 | 0.007 | 0 | 673.8 |
      | `write_omx` | 0.599 | 131.4 | 34.6 | 6.024 | 1,238.9 | 298.8 |
      | parse (`compile`) | 0.464 | 219.5 | 162.4 | 4.727 | 2,138.5 | 1,550.1 |
      | evaluate (build, run) | 0.046 | 79.0 | 241.4 | 0.889 | 773.9 | 2,323.9 |
      | snapshot (every cell shown, kept) | 0.136 | 44.9 | 286.3 | 1.489 | 442.3 | 2,766.2 |
      | AST, second parse (`rebuild`) | 0.070 | 87.8 | 368.1 | 1.121 | 836.5 | 3,554.8 |
      | `Document::from_text`, on the source alone | 0.933 | 577.0 | 505.6 | 14.659 | 5,654.4 | 4,921.9 |

      The 29× is the parts path: 286 MiB of 10 MiB (29.0×) and 2,766 MiB of
      99 MiB (28.0×) held when the snapshot ends, and it is `Datum` rows,
      `write_omx` and the parse that make it: the three of them are 80–85% of
      the time to that point (the records and `Datum` 3.2 s, `write_omx`
      6.0 s and the parse 4.7 s at 100 MB). The text is 1.4× the file. Through `Document`
      it is worse, which the proposal's figures did not take: peak 36.5–38×
      and the time 1.3–1.8× the parts, because `rebuild` parses the text again
      for its AST and keeps a snapshot per cell (value, source, text), and
      what a `Document` holds once it is open is 506 MiB for 10 MB and
      4,922 MiB for 99 MB, about 50× the file. Where the 100 MB run spends
      its extra (14.7 s against 8.2 s) is not yet taken apart; 10× the rows
      costs 30× the time of the rest of `rebuild`, which is not linear
- [x] 0.3 `scan_csv → sink_parquet` on the 1.1 GB file, cold and warm: time,
      peak memory, size of the copy; and the effect of the `csv` feature (and
      the streaming engine, if it is a feature of its own) on build time and
      binary size. This is what decides whether D153's risk fallback is needed.

      Measured 2026-11-07, release, on the synthetic 1,060 MB file (20.8M
      rows, 7 columns), `table::convert`: the copy is 236 MB, in 2.0 s warm
      and 3.0 s cold (0.1's harness); in a process of its own
      (`examples/convert_csv.rs` under `/usr/bin/time -l`, warm, two runs)
      2.1–3.2 s, peak resident set 1.57 GB, of which the CSV's mapped pages
      are most, and peak memory footprint 0.49 GB, which is the allocator's
      383 MiB again. Cold in its own process was not taken (`purge` asks for
      a password); the cold time is the harness's

      Build cost, a scratch crate with this workspace's Polars features and
      release profile and the lockfile, clean builds one after the other,
      with and without `csv`, using a Parquet scan, a streaming aggregate and
      a Parquet sink, and with `csv` the scan feeding the sink:

      | | clean release build | binary |
      |---|---:|---:|
      | without `csv` | 297 s | 43.9 MB |
      | with `csv` | 279 s | 44.4 MB (+0.5 MB, +1.2%) |

      The streaming engine is not a feature of its own to pay for: Polars'
      `parquet` feature already turns on `new_streaming`, so `csv` adds only
      the reader. The difference in build time is within what one run each
      can tell apart (the one with `csv` was the faster), so the cost is
      none that can be seen and +0.5 MB of binary, against 52.7 MB for the
      release `omasheet` today. **D153's risk fallback is not needed**: the
      streaming conversion is 2 to 3 s and bounded by 0.5 GB, and the
      feature is cheap. This is the scratch crate's figure, not the
      workspace's: the workspace cannot be built without `csv` now, so the
      binary of `omasheet` itself was not taken both ways

## 1. Prerequisites from `hold-less-of-a-big-file`

- [x] 1.1 Its item 2: `Value::Series` (D79), so that a column read stays a
      Polars column
- [x] 1.2 Its item 3: the snapshot formats only what the grid shows (D80).
      Recommended: the snapshot holds the frame and formats a cell when it
      is drawn, because the Qt model already asks only for visible cells
- [ ] 1.3 Measured on the NYC taxi Parquet file (41M rows): a `table` over it
      opens and scrolls in the app within the targets, which neither does
      today

      Measured 2026-11-07, release, warm, one run each, by
      `a_parquet_table_against_the_targets` in `timings.rs` (the data's
      share of the frame: `Document::rows` of 40 rows, all columns; not the
      drawing). The 41M-row year is not in `demo_data`, so these are the two
      files that are: ride-hail March 2019 (611 MB, 23.9M rows × 24) and
      yellow taxi January 2026 (64 MB, 3.7M rows × 20):

      | | open | first screen | jump to last | scroll, median frame |
      |---|---:|---:|---:|---:|
      | ride-hail, paged (5.7 GB to hold, so never held) | 22 ms | 73 ms | 51 ms | 69 ms, 200 of 200 over 16 ms |
      | yellow, paged | 2.8 ms | 13 ms | 9 ms | 13.9 ms, none over 16 |
      | yellow, held whole (56 ms, 563 MiB to read) | | 0.2 ms | 0.1 ms | 0.1 ms |

      Open meets ≤ 50 ms for both. A frame meets ≤ 16 ms where the table is
      held, and paged only for the small file, by a margin of 2 ms. A table
      too big to hold, which is the case this task is about, was 51–73 ms a
      frame: every frame read its rows from the file again, three rows
      further on. The window of rows the engine keeps was exactly the rows
      last asked for, so each step walked off its end.

      Read-ahead, built 2026-11-07: a grid's read of a table's column
      (`Engine::rows_shown`) reads 8,192 rows past the screen on each side
      (`READ_AHEAD`, `read_range_ahead`); `head` and `tail` keep reading
      exactly their rows, so `rowgroups.rs` is as it was. Test:
      `a_scroll_reads_the_file_once_for_many_frames` (a thousand
      three-row steps read once, a jump reads once more, the last row is
      right). Measured again, warm, same harness:

      | | open | first screen | jump to last | scroll, median frame |
      |---|---:|---:|---:|---:|
      | ride-hail, paged | 22 ms | 73 ms | 51 ms | **0.2 ms**, 1 of 200 over 16 ms (the refill, 71 ms) |
      | yellow, paged | 2.8 ms | 13 ms | 9 ms | **0.1 ms**, none over 16 |

      Scrolling now meets ≤ 16 ms a frame, and open meets ≤ 50 ms. What was
      left was a jump (and the first screen), which read a window of every
      one of the 24 columns, one after another.

      Read side by side, built 2026-11-07: `Engine::prefetch_rows`, called
      by `Document::rows` for a paged sheet, works out which columns need a
      window and reads them on `available_parallelism` threads, then keeps
      each as `rows_shown` would have; fewer than two to read, and it leaves
      them to `rows_shown`. Test:
      `a_jump_reads_every_columns_window_and_shows_the_right_rows` (six
      columns, every cell is its own row and column, no more read on the
      next frame). Measured again, warm, three runs of the same harness:

      | | open | first screen | jump to last | jump to middle | scroll, median |
      |---|---:|---:|---:|---:|---:|
      | ride-hail, paged | 16–19 ms | 18.5–22 ms (was 73) | 12–15 ms (was 51) | 16–18 ms (was 70) | 0.2 ms (was 69) |
      | yellow, paged | 2.8 ms | 5 ms (was 13) | 5 ms (was 9) | 7 ms (was 17) | 0.1 ms (was 14) |

      Open to first screen is about 37 ms on the big file, under the 50 ms
      target; a frame is under 16 ms except the jump to the middle, which is
      16–18 ms on the 24-column file, at the line. Not yet taken: cold (it
      needs `sudo -v`; taken cleanly by the owner on 2026-11-07 with
      `--test-threads=1`: ride-hail first screen 45.6 ms, jump to last rows
      **170 ms**, to the middle 27 ms, scroll 0.2 ms median; yellow first
      screen 24 ms, jumps 11–17 ms). The 170 ms is that file's layout: it
      is a single row group of all 23.9M rows (610 MB compressed, 1.1 GB
      uncompressed; `examples/row_groups.rs` shows it), so a cold read near
      its end is a read of the far end of one huge column chunk, whatever the
      window. Our own copies are not laid out so: the 1.1 GB synthetic CSV's
      copy is 80 row groups of 262,144 rows, 3 MB each, and a window is one
      or two of them. Over that copy, warm (`OMASHEET_BIG=<copy>`): open 17.6
      ms, first screen 10.8, jumps 4, scroll 0.0 ms median with none of 200
      frames over 16 ms; held whole it is 144 ms and 1.35 GB. Cold over the
      copy, taken by the owner the same day: open 34.5 ms, first screen
      12.8, jumps 4–5, scroll 0.0 median with none of 200 over 16 ms; held
      whole 277 ms. Every target is met over our own copy, warm and cold.
      Not taken: the 41M-row year, which is not in `demo_data`
      (`OMASHEET_BIG=<path>`). So what is left is that file, and whether the
      single-row-group case (170 ms cold to the end) is the file's to answer
      for, which is the owner's to say

- [x] 2.1 `rebuild` split into a synchronous compile and check, and an
      evaluation that runs on a worker, with generation numbers and results
      posted with `qt_thread().queue`

      **Superseded, not built** (owner, 2026-11-07). It was written when an
      edit re-read the whole file on the UI thread. After 2.2 and 2.4 it
      does not: measured warm, release, an edit that adds a formula column to
      the 23.9M-row table is 7 ms on the UI thread, opening a table 17 ms,
      the swap when a table arrives held whole 15 ms (20.8M rows) and 4.5 ms
      (3.7M), a jump 4–18 ms. What is heavy already runs on a worker: the
      read of the table whole (127 ms), sort, filter and the selection
      summary (`file.rs`'s `spawn`s). Nothing found on a big table holds the
      thread for more than about 20 ms. Building it would mean an engine
      that can cross threads (it is `Rc` and `RefCell` throughout) or a
      thread of its own that every `rows` and `snapshot` talks to, for a
      problem the figures say is gone. A big inline sheet from XLSX or JSON,
      which does block, goes away with 3.3 and 4.1 making them tables
- [x] 2.2 Elementwise columns computed for the visible rows only, and the
      whole column only when something asks for it

      Built 2026-11-07. `Options::defer_rows` (the `Document` sets it to its
      paging threshold): for a `table`'s sheet of more rows than that, a
      computed column that lowers is not calculated when the engine runs.
      `Engine::run` tries the plan on the first window (`try_defer`), and if
      Polars takes it, leaves the column; `rows_shown` then calculates it
      for a window of the rows asked for plus 8,192 each side
      (`computed_window`), by the same plan over the same rows of each
      column it reads (`plan_with`, `window_column`), through the same
      checks as the whole column (`lowered_column`, shared with
      `run_lowered`: the unaccounted-null flag, the decimal cast). A formula
      over a formula is windowed through the first. Anything that wants the
      whole of the column (`pl_column`: an aggregate, a sort, a filter, a
      lowering over it; or a single `cell`) calculates it then
      (`materialise`), as `run` would have, or by the row loop if Polars
      will not. A column Polars refuses on the first window stays as it was:
      eager, whole.

      Tests: `a_formula_column_over_a_big_table_is_calculated_for_the_rows_shown`
      (a column over a column over a file's, 40,000 rows: nothing calculated
      whole at open or after screens at the start, middle and end and a
      scroll; every cell right; a `const` sum over the second is then the
      right sum and calculates it whole). The two paged-against-held
      differentials (`a_paged_table_shows_what_a_held_one_does` and the
      filter and summary ones) hold, with `Twice` deferred. The two cache
      tests of 2.4 had counted on opening reading every column whole, which
      it no longer does, and now read one with a sort.

      (The session of 5.3 found this cache never hit for a copy of a CSV, JSON
      or workbook, because a copy is touched at each read and its time was
      part of its identity; fixed there. The figures below are for plain
      Parquet files, which are not touched, and stand.)

      Measured, release, warm, `an_edit_over_a_big_table_reads_no_column_again`
      (add a formula column over `PULocationID`): ride-hail 23.9M rows
      **109 ms → 6.9 ms** (226 → 8.7 without the cache), yellow 20 → 2 ms.
      Then the new column drawn: first screen 7.9 ms, a jump to the middle
      3.6 ms, a scroll step under 0.1 ms; yellow 1.7, 1.3, under 0.1. An
      edit meets ≤ 50 ms (5.3) for the visible cells.

      What changes meaning, for the owner: a deferred column's per-row
      problems (an error in a row, as D67's row loop would file) are filed
      when the column is calculated whole, not at open, so the problems list
      for rows nobody has looked at is not there until something asks; and
      `attempts()` does not record a deferred column until it is
      materialised. A column that Polars will not do at all is unaffected,
      since it is not deferred
- [x] 2.3 Pending and loading states in the grid and in the constants panel

      **Superseded, not built** (owner, 2026-11-07): it exists to show the
      pending state of 2.1's worker evaluation, which is not built. The
      loading state of rows past a CSV's sample is 3.6's
- [x] 2.4 The column cache (D150): owned by the `Document`, survives
      rebuilds, evicts LRU within a budget

      Built 2026-11-07: `table::ColumnCache`, held by the `Document` and
      handed to each rebuild through `read_holding`, so that a
      `Held::Column` read whole takes the column from it and puts it there.
      Keyed by file and column name, with the file's identity (length and
      mtime) beside each entry: a file changed since is a miss and the old
      entry goes. Sized by the footer estimate (D157's), not Polars'
      figure, and dropped least recently used first past a budget of a
      quarter of physical memory (`Document::set_column_budget` for a
      test). A column over the budget alone is not kept, and a range read
      is never put there. Tests: four in `table.rs` (LRU order, over
      budget, changed file, replaced entry) and
      `a_column_read_whole_is_kept_across_an_edit` in `doc.rs`, which holds
      that an edit reads nothing from the file again and that a changed
      file is read again.

      Measured, release, warm, by `an_edit_over_a_big_table_reads_no_column_again`
      (a formula column added, which rebuilds the engine over a column
      already read by a sort): ride-hail 23.9M rows, the 184 MiB column,
      226 ms with no cache (read again, 2 misses) against 109 ms with it
      (1 hit); yellow 47 ms against 20 ms. That is the file read the cache
      saves, ~115 ms warm here and more cold; the rest of the 109 ms was
      the formula column calculated whole, which 2.2 has since taken to
      6.9 ms.

      D157's share, done the same day: the budget gives way to what is held
      whole. `Whole` carries its footer estimate, `rebuild` tells the cache
      the total (`ColumnCache::reserve`), so the cache may take a quarter of
      memory *less* what tables hold, and never below nothing; and holding
      a table drops that file's columns from the cache, which would be the
      same data twice. The two stores stay separate: a held table is one
      all-or-nothing read the grid moves over to. Tests: the budget giving
      way and a file's columns evicted, in `table.rs`, and
      `a_table_held_whole_is_not_also_kept_as_columns_and_the_cache_gives_way`
      in `doc.rs`.

      What this is not: the cache does not make a scrolled frame fast, since
      a frame reads a range and a range is never kept here. That was the
      read-ahead window, under 1.3

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
- [x] 2b.7 Listing a column's distinct values in Polars (`unique`, then the
      first 1,000 in sort order), not a walk over every row
      Superseded, 2026-11-08: nothing lists a column's distinct values any
      more. The popup that did (`values_matching`, D159) went out with
      `8d330b5`, when a filter became an expression written by the user, and
      `git grep` finds no caller or definition left. Distinct values are a
      filter expression now (`*.is_first_distinct()`, lowered to Polars and
      tested over a copy in `doc.rs`)
- [x] 2b.8 The probe in `Main.qml` taken out before this change is merged.
      Done 2026-11-02, in filter-by-expression; later probes were
      temporary and none is left

## 3. One funnel, first stage: big files (D147, D152, D153)

- [ ] 3.1 The reader interface: named `DataFrame`s, plus XLSX formulas.
      `parquet` is already one

      Two readers done, JSON then XLSX (2026-11-07), and the interface is
      what they share. `copyframe.rs` has the `RowSource` trait (rows of
      cells, in order, until told to stop), the types a column comes to
      (`Kind`), a cell read once (`Cell`), builders, schema inference from
      the head or from every row (`schema_of`), and the writer (`write`,
      `convert`, `sample`); `jsoncopy.rs` and `xlsxcopy.rs` are the two
      readers. `table::{Kind, kind, split_sheet, is_converted}` say what a
      source is and split its `file#Sheet`, `table::{sample, convert}`
      dispatch on it, and `omasheet_interop::table_source` gives the `view`
      lines for a file, a line for each sheet. CSV still goes through Polars'
      own reader and `parquet` is already a frame; folding CSV into
      `RowSource` is not worth it, since Polars does it faster
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

      JSON and XLSX done 2026-11-07 (below), and D154, the formulas of a
      workbook, for a workbook whose every formula is filled down a column;
      a workbook with any other formula is imported inline, as before (see
      below). JSON, in `omasheet-engine/src/jsoncopy.rs`, is read as a stream and never held: each value is read
      once as a `Cell` (no tree of values per row), and rows go to typed
      builders and out as a row group of 262,144 at a time. Types are worked
      out from the first 10,000 rows and the file written in one pass; where
      a later row does not fit (a number among whole ones, a key that is new,
      a string among numbers) the types are worked out again from every row
      and it is written again, as a CSV's are. Whole numbers are `Int64`,
      other numbers `Float64` (not the exact decimals `omasheet import`
      makes: the same as a CSV copy), `true`/`false` `Boolean`, a string that
      is an ISO date or date-time in every cell `Date` or `Datetime`, anything
      else text, with a list or an object as its JSON. A key a row lacks is a
      null. Shapes: a list of objects, a list of lists whose first row is the
      names, or an object of such lists, a sheet each by key.

      Tests (`jsoncopy.rs`, 11): types from what every row holds; mixed and
      nested values; list of lists; object of sheets and a source naming one;
      the sheets of a file; what is not rows; more rows than a batch (two row
      groups, all read back, the sample typed alike); a row past the head
      that does not fit; number forms (and serde_json's number token); dates
      among text; the quick date reading against the general one for every
      day from 1900 to 2200. And at the `Document` level
      `a_json_table_names_a_sheet_and_shows_its_sample_and_then_its_copy_alike`

      Measured, release, one run, `convert_csv` under `time -l` and the
      target harness with `OMASHEET_BENCH_EXT=json`; the files are the
      generator's seeded rows as JSON (`gen_big_csv --json`, or
      `generate.sh json`), 23 MB / 233 MB / 2.37 GB for 211,000 / 2.07M /
      20.8M rows:

      | | open (sample) | copy | peak RSS | copy size | reopen |
      |---|---:|---:|---:|---:|---:|
      | 23 MB | 26 ms | 0.19–0.26 s | 67 MB | 2 MB | 0.4 ms |
      | 233 MB | 13 ms | 1.8–2.1 s | 77 MB | 23 MB | 0.7 ms |
      | 2.37 GB | 18 ms | 18–19 s | 80 MB | 236 MB | 15 ms |

      Memory is bounded by a batch whatever the file's size. The first
      version built a `serde_json::Value` for every row and took 69 s for the
      2.37 GB file (34 MB/s, two passes); reading each value once as a cell
      and working the types out from the head took it to 19 s (125 MB/s).
      The copy is the same shape as a CSV's, so every figure after the copy
      (sums, jumps, scrolls) is the CSV's. What limits it now is serde_json's
      reader over a file, which is slower than over bytes in memory; splitting
      rows out of a buffer would be the next step if 19 s is too long. Cold
      not taken

      XLSX, `omasheet-engine/src/xlsxcopy.rs`: read with calamine's
      `worksheet_cells_reader`, which hands the cells of a sheet one at a
      time out of its XML, so the sheet is never held (only the workbook's
      shared strings are, as calamine holds them); the rows go to the same
      builders and out as row groups. It reads a sheet as `omasheet import`
      does: the first row of what is used is the names (the used part may
      start anywhere, and `dimensions()` says where, with a retry from the
      column of any cell found left of it); a blank cell, a whitespace-only
      string and an error cell are null; a whole-valued float is whole; a
      serial is a `Date`, a `Time`, a `Datetime` or a `Duration` by which parts
      are there, and a column with a time on some of its dates is a column of
      date-times (`Kind::join` now says so for JSON too); text stays text
      whatever it looks like; an empty row between rows is a row of nulls.
      Formulas are not read: a cell is the value the workbook last worked
      out. A bare `book.xlsx` is the first sheet with a cell in it, and
      `sheets` lists those that have.

      Tests, in `omasheet-interop/tests/interop.rs` since they need the
      importer to compare with: `a_workbook_sheet_as_a_table_shows_what_an_import_of_it_shows`
      (budget's two sheets and the 700-row financial sample: the same rows,
      columns and every cell as shown), and
      `a_workbook_sheet_with_offsets_gaps_and_every_kind_of_cell_reads_as_an_import_does`
      (a header at C3, an empty row, whitespace, a date among date-times, a
      time, a duration, a flag, a whole number as a float, an empty sheet
      first; table and import agree cell for cell, bare is the first sheet
      with anything). Differences from an import are the same as JSON's: a
      number that is not whole is a float, not an exact decimal, and names
      keep what the header says (`Discount Band`, not `DiscountBand`)

      D154, first half, 2026-11-07: a table is values only, so
      `xlsxcopy::has_formulas` reads a sheet for a formula (calamine's
      `next_cell_with_formula_metadata`, which tells a formula from a value
      without expanding a shared one, stopping at the first; the whole sheet
      is read when there is none, since a total at the foot is a formula as
      much as a cell at the head) and `omasheet_interop::table_source`
      refuses a workbook any sheet of which has one. The app and the CLI then
      keep to the import for it, which keeps the formulas, so no workbook
      changes what it does: a big workbook with values only opens as tables
      and one with a formula is imported as before. Tests:
      `table_source_names_each_sheet_of_a_workbook_that_has_something_in_it_and_no_formula`
      (an empty sheet first; a formula in another sheet; a formula only in the
      last of 5,000 rows; the examples' workbooks, which hold values only;
      what is not a workbook). The check costs 0.38 s on the 211,000-row
      workbook and 1.7 s on the 1,048,575-row one, once, before it opens (7.7 s
      to import it).

      D154, second half, 2026-11-07: a workbook with formulas is read as
      tables where every formula is one filled down a column
      (`omasheet-interop/src/xlsx/table.rs`, `plan`), and the formula is a
      `:=` line over the table, as `Family` is in `analysis.omx`. Each sheet is
      read once for its formulas, a cell at a time, keeping nothing of them but,
      for each column, how many cells have one, whether they are all the one
      formula (`formula::relative`: the formula with each cell it names as how
      far it is from its own row, so two rows of a column filled down come to
      the same text, and a `$` stays put), and the first. A column is filled
      when every data row has the formula and they are the same. The sheets are
      copied (the copies stay in the cache, so the document opens on them at
      once), the names are those an import gives, from the copies' columns, the
      first formula of each filled column is translated
      (`formula::translate`, whose `filled` form is the column's formula), and
      the document is written: a `view` for each sheet with its columns declared
      in the workbook's order, a type for each and a `from` where the name
      differs, and a `:=` line where the column is calculated. The cached
      values of the calculated columns stay in the copy, undeclared; a view
      shows only what it declares. It is then held to what the workbook has:
      the document is compiled and run over the copies, and each calculated
      column is calculated whole by Polars and compared with the values the
      workbook holds, every row, numbers to a relative 1e-9 and anything else
      as text. Anything in doubt is a reason to import instead, with that
      reason: a formula in any column but one filled down every row (a total
      at the foot, a column that changes its formula part way, a formula in
      the header row, a sheet of a single row), a formula with no translation
      or that gives something else each time it is worked out (`today()`), one
      Polars will not calculate whole (a whole number divided by another is an
      exact fraction; it would also be a row loop over every row each time the
      document is built), or one that does not give what the workbook holds in
      any row. Tests: `formula::tests::a_formula_filled_down_a_column_has_the_same_relative_form_on_every_row`,
      and in `interop.rs`
      `a_workbook_whose_formulas_are_filled_down_its_columns_is_tables_with_calculated_columns`
      (the exact text, the document's columns and rows) and
      `a_workbook_with_a_formula_that_is_not_filled_down_a_column_is_imported_not_read_as_tables`
      (a total at the foot, a formula that changes part way, one cached value
      that is not what its formula gives, no translation, an exact fraction;
      each with its reason).

      Measured, release, the workbooks of `gen_big_csv --xlsx --formulas` (the
      seeded rows with `amount` as `=qty*price` filled down, its value held):
      211,000 rows, 10 MB, 2.1 s and 86 MB for the whole plan (the read for
      formulas, the copy, the translation and the check of every row);
      1,048,575 rows, 50 MB, 4.2 s and 134 MB. The line it gives is
      `amount := qty * price` over `id`, `date`, `region`, `product`, `qty` and
      `price` as declared columns. Against 7.7 s and 1.56 GB for importing the
      same rows as text with no formula (the import of the one with formulas,
      which translates and checks each cell, was not timed).

      Not done: a workbook that has a sheet of formulas that differ from cell
      to cell next to a sheet of data (the usual shape of data and a summary)
      is imported whole, as before, where D154 has the summary stay inline and
      the data a table. That means building the inline sheets with the table
      sheets in the translator's view of the workbook, and writing a document
      of views and sheets, which the translator's `rows` and the checker of an
      import (`settle`) would both have to know. It is the next step if that
      shape matters. The pre-flight (read, copy, check) is on the thread that
      draws, 4.2 s for a million rows

      Measured, release, one run; the workbooks are the generator's seeded
      rows (`gen_big_csv --xlsx`, or `generate.sh xlsx`), 211,000 rows in 8 MB
      and, at the 1,048,575 rows a sheet can hold, 42 MB:

      | | open (sample) | copy | peak RSS | copy size | reopen | sum `qty` |
      |---|---:|---:|---:|---:|---:|---:|
      | 8 MB, 211,000 rows | 40 ms | 0.41 s | 65 MB | 2 MB | 0.4 ms | 5384690 |
      | 42 MB, 1,048,575 rows | 23 ms | 1.9 s | 74 MB | 11 MB | 0.5 ms | 26730673 |

      The 211,000 rows sum to what the CSV and the JSON of the same rows do
      (5384690). For contrast, `omasheet import` of the 42 MB workbook, which
      holds the sheet whole and writes it as text, takes 7.7 s and 1.56 GB.
      Cold not taken
- [x] 3.4 The conversion cache: hashed name, atomic write, `sink_parquet`,
      LRU within a disk budget. Checked 2026-11-07: the budget is there
      too, `table::cache_budget` (10 GB or a tenth of the free space, the
      less) and `table::prune` (least recently used first, the copy just
      written kept), called by `convert`; tests
      `the_budget_is_ten_gigabytes_or_a_tenth_of_the_space_free` and
      `the_cache_lets_the_least_lately_used_copies_go_first`. Done
      (2026-11-02): in
      `$OMASHEET_CACHE`, else `$XDG_CACHE_HOME/omasheet`, else
      `~/.cache/omasheet`; named by a hash of the canonical path, then the
      length and mtime; written to `.partial-<pid>` and renamed; a new copy
      takes away the CSV's earlier copies, its sample and any partial file
- [ ] 3.5 `dataframe["x.csv"]` (and `.xlsx`, `.json`) resolved by
      `table::read` to the cached copy; the engine reads Parquet alone.
      JSON and XLSX done 2026-11-07, with the sheet named by `file.json#Sheet`
      and `book.xlsx#Sheet` (D160): the copy's name is a hash of the file and the sheet, so each
      sheet of a book has its own, and a missing sheet is a diagnostic.
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
      it as a table; the status bar says it is read-only and why. JSON done
      2026-11-07 (not looked at in the window, which this session cannot
      do): over 4 MB `open_file` calls `open_table` for a `.json` as for a
      CSV, with a `view` line for each sheet from `table_source`, and the
      worker's `convert_tables` writes each sheet's copy. A JSON that cannot
      be read as rows falls back to the import, which says why. XLSX over 4 MB (the file's size,
      which is compressed: 8 MB is 211,000 rows of seven columns) is switched
      over for a workbook with no formula in it: `open_file` calls
      `open_table`, whose `table_source` lists its sheets, and a workbook with
      a formula is imported inline as before (D154). The pre-flight read for
      formulas is on the thread that draws, 1.7 s for a million rows Done for
      CSV (2026-11-02): over 4 MB it opens as a table, with a note that
      says so and where the copy is kept. Measured, release, the 921 MB
      ride-hail CSV, cache empty: first screen 25 ms (sample 20 ms, open
      5 ms; was the inline import, which never finished), the copy 1.8 s
      (125 MB), reopening with the copy current 35 ms; in the app, the
      15M rows shown and held in memory (1.2 GB) within 4 s
- [ ] 4.2 "Copy into the document" for a table under the threshold
- [ ] 4.3 `omasheet import` above the threshold refuses and prints the
      `table` lines, ready to paste
      Done for CSV, Parquet and, 2026-11-07, JSON (a line for each sheet);
      test `import_of_a_big_json_prints_a_table_for_each_sheet_and_they_read_through_copies`.
      XLSX too, for a workbook with no formula in it (test
      `import_of_a_big_workbook_of_values_prints_a_table_for_each_sheet`); one
      with a formula is imported, for the reason under 3.3. The import of a
      workbook over the size is not in a test, which takes a minute in a build
      without optimisation
- [ ] 4.4 The inline writer takes a frame, and `Grid`/`Datum` are deleted,
      only once every `examples/*.expected` and the interop tests are
      byte-identical. Where they are not (a column's scale, mixed-type
      columns, the text of a literal), the owner decides before this goes in

## 5. Measured against the targets

- [ ] 5.1 Open to first screen, 1.1 GB file, cold and warm: ≤ 50 ms

      Measured 2026-11-07, release, the seeded 1,060 MB CSV (20.8M rows),
      `the_targets_on_a_synthetic_csv` and `session_csv`: warm **11.6 ms**
      (12–41 ms across runs); cold, by the owner, **57 ms** (57.0 and 57.5),
      7 ms over. The same step is 52–57 ms cold for the 10 MB and 100 MB files
      as well, so it is not the size. A `purge` also evicts the program's own
      pages, which this run pays for and the app, already running, does not;
      that is not separated from the rest, so the cold figure is an upper
      bound. Not met cold as measured
- [x] 5.2 Once the copy is written, jump to the last row and scroll by a
      page: ≤ 16 ms a frame. Reopening with a current copy: ≤ 50 ms to a
      fully usable table

      Met, 2026-11-07, over the copy of the 1.1 GB CSV (80 row groups of
      262,144 rows): a jump to the last rows and to the middle **3.7–3.8 ms**
      warm and **4.2–4.7 ms** cold, a scroll **0.0–0.1 ms** a frame with none of
      200 over 16 ms, warm and cold (1.3 has the table); reopening with the
      copy current **13.6 ms** warm, **30.6 ms** cold, to a table that
      scrolls, sorts and sums
- [ ] 5.3 Edit a formula column: visible cells ≤ 50 ms; `Amount.sum()`
      first time off-thread, then ≤ 50 ms from the cache

      Measured 2026-11-07 by `session_csv` (release, the 1.1 GB CSV, a
      document with `const Sum… = T.<column>.sum()` for each of `id`, `qty`,
      `price` and `amount`, which is the session of 5.4 too). **Edit a formula
      column: 7.3 ms**, its visible cells 1.8 ms, a second edit 26.6 ms, with
      all four sums on the sheet. That is after two fixes this measurement
      found, below: before them the same edit was **1,500 ms**. The sums
      then come from the cache, as asked.

      Not met: the first time. The four sums take **1.4–1.6 s** when the copy
      arrives (the document is rebuilt on the thread that draws, which 2.1
      would have moved and which was superseded on the strength of edits that
      had no aggregate over a big column in the document; with one, the
      first rebuild holds the thread about 350 ms a column). The integer
      sums are Polars' (about 110 ms each, warm); a float sum is the engine's
      own fold over every value, which OMX holds to row order and Polars'
      pairwise sum does not give (D72), about 500 ms each. A file opened
      again in a new process pays them again, the answers being kept for a
      document's life and not on disk.

      **Two faults found and fixed, 2026-11-07.** (1) A copy in the cache
      is touched on every read, for the pruning, and its modification time
      was part of its identity, so every copy of a CSV, JSON or workbook was
      a different file at each rebuild: the column cache of 2.4 never hit for
      them (it was measured on plain Parquet, which is not touched), and a
      copy held whole (D157) failed its identity check and was dropped at the
      next rebuild. `table::identity` now gives a copy in the cache no time:
      its name says what it is a copy of. Test
      `a_copy_in_the_cache_keeps_its_identity_when_it_is_read`. (2) Task 2.5
      is ticked and the engine has no aggregate on `scan(path)` and no cache of
      answers (`Engine::Streaming` is in the writer of the copy and nowhere
      else): a sum was calculated again at every rebuild. The answer cache of
      D155 is built now: `ColumnCache` keeps the answer of an aggregate over a
      whole column of a file, by the file's path and identity, the column,
      the function and the column's declared type, and `Engine::call` gives
      it again (`Scalar`, a value without an `Rc` in it, for the numbers,
      dates, durations and text). Test
      `an_aggregate_over_a_copied_file_is_kept_across_edits_and_not_for_another_file`
      (kept across an edit, not given for a file written again, kept apart
      by the declared type). The scan with the streaming engine, which would
      add the column up without holding it, is still not built.

      **Typed sum of floats, 2026-11-07.** A sum or a mean of a column of
      floats is now a loop over the floats (`Engine::float_total`) and not a
      `Value` made of each of 20.8M cells, from `0.0`, a row at a time, which
      is what the walk's `arith` comes to (`0.0 + x`, then `x + y`), so the
      answer is the same to the last bit; where Polars' pairwise sum is not,
      D72. It applies where the sheet declares the column a `Float` and no
      cell of it has been worked out on its own, and leaves the rest to the
      walk. The walk itself no longer copies a column of many row groups
      into one for every aggregate asked of it (`Series::iter` wants one
      chunk, so the copy is made where it is walked, which the loop does not
      do). Tests, `omasheet-engine/tests/floatsum.rs`: the sum and the mean
      of seven columns (magnitudes from 1e-8 to 1e12 in both signs, empty
      cells, a `NaN`, both infinities, all `-0.0`, 1e16 + 1 - 1e16 and then
      small numbers, nothing at all), over one row group and over thirty,
      taken by the loop and by the walk, equal in every bit, and the loop
      is what answered (`float_totals()`); and that a column declared as
      anything but a float is left to the walk. Measured, `session_csv`: the
      four sums on the thread that draws when the copy arrives **1,550 ms →
      345–400 ms** (the two integer sums are Polars', over a column read
      whole, the two float sums the loop), with the same four answers to
      every digit; an edit with all four on the sheet 7–18 ms. The first
      time is therefore about 90 ms a column on the thread that draws, not
      off it, and with 2.1 superseded that is what stays
- [x] 5.4 Peak memory over a session that sums every numeric column:
      ≤ 150 MB + the columns' Arrow size, including during conversion

      Measured 2026-11-07 by `session_csv` (peak memory footprint, which is
      what the process asked of the machine and leaves out the file pages it
      mapped), the 1.1 GB CSV: open **11 MB**; the copy **499 MiB**; the sums of
      the four numeric columns **1,236 MiB**, with 644 MiB of the four columns
      held (166 MiB each); the edit that follows 1,240 MiB. The target for
      that session is 150 + 644 = 794 MB. Not met, in two places:

      The copy: Polars' streaming `scan_csv → sink_parquet` takes about 100
      MB a thread. At one thread 100 MB and 6.9 s, two 227 MB and 3.7 s,
      three 350 MB and 2.7 s, four 428 MB and 2.3 s, all eight 500 MB and
      3.1 s; the Parquet row group size (500 → 420 MB at 65,536 rows) and the
      CSV reader's chunk size (none) move it little. The thread count is the
      pool's, `POLARS_MAX_THREADS`, read once for the whole process, so it
      cannot be set for the copy alone without slowing every other Polars
      call. The copy of a JSON file takes 37 MiB and of a workbook 35 MiB,
      for their own readers are not Polars'. Left at 500 MB for 3 s, bounded
      however big the CSV is; the owner's to say whether that or a smaller
      one for longer (or the copy in a child process with fewer threads,
      which keeps it out of the app's own peak) is wanted.

      The sums: a sum adds up a column it has read whole and holds, 644 MiB
      for four, where the target has the sum not hold what it need not. That
      is D155's scan on the streaming engine, and the float sums, which are
      the engine's fold, would have to walk the file a row group at a time
      as well. Neither is built.

      Re-measured after the typed sum, 2026-11-07: the sums now peak at
      **1,080 MiB** (from 1,236; the walk no longer copies each column). The
      rest is not the sums: after the copy the process still holds **463 MiB**
      (451–473 across runs), which it does not give back. It is not free
      memory in the allocator: `malloc_zone_pressure_relief` released 0 bytes,
      and the figure was the same two seconds later; it is something Polars'
      streaming engine keeps (about 100 MB a thread, as above). So the
      session is that, 463, and the four columns held, 644, which is 1,080,
      and the copy's peak is not a peak that ends. The way to have it end is
      for the copy to be done in a process of its own, which gives its memory
      back to the machine when it exits: the app's footprint would then be
      the columns held and what it holds itself, about 680 MiB, under the 794
      of the target, and the copy's 500 MB (or 227 MB with two threads, 3.7 s)
      would be a child's.
      **Built, 2026-11-08 (owner's go-ahead), then reverted the same day: it does not bring the peak down (measured below).** `table::convert` runs the
      program itself again with `OMASHEET_COPY_CHILD=<source>` and
      `POLARS_MAX_THREADS=2`; the child calls `convert_here` and prints the
      copy's path. `omasheet_cli::main` (the app and the command line both go
      through it) answers as the child when the variable is set and otherwise
      calls `copy_in_children()`; tests and libraries never do, so they copy
      in-process as before. If the child cannot be started the copy is made
      in-process. The 100 MB CSV through the release CLI: the parent's peak
      footprint is 144 MB, the copy being the child's. The app's session
      peak is measured below.
      **Measured, 1.1 GB file, `session_csv` (child by default, `--here` for
      in-process), 2026-11-08:** copy 4.6 s in a child (2 threads) against
      2.3 s in-process; footprint after the copy 10 MiB against 500. But
      after the sums 1,078 MiB against 1,081, and the final peak 1,239
      against 1,242. **The child does not bring the session under 794 MiB,
      and my earlier reading was wrong**: the ~463 MiB "retained after the
      copy" was not the copy's, it is what the column reads leave (the
      cache holds 644 MiB for 11 columns, and the footprint over it is
      ~430 MiB more, which the reads grew to and the allocator keeps). The
      child is reverted: it took the copy's 500 MB off the app during the
      copy only, for twice the copy time. What is left to cut is in
      the reads: 11 columns held (644 MiB) for sums whose answers are
      cached, and the added formula column (~155 MiB)
      **Aggregate reads no longer kept, 2026-11-08.** The footprint was not
      the cache (making it hold 0 MiB changed nothing): the engine keeps a
      column it has read in its `held` slot for the whole run, so four sums
      held four columns until the run ended. `Engine::call` now lets go of
      a column read only to answer an aggregate (from `held`, and from the
      cache), keeping the answer. 1.1 GB, `session_csv`, copy in-process:
      after the sums 1,081 -> 699 MiB, session peak 1,242 -> **909 MiB**
      (target 794); sums 355 ms, edit with sums 7 ms, the answers
      identical. Still 115 MiB over; the peak is a transient inside the
      first sums (909 against 699 after), and the copy leaves ~450 MiB
      that the reads reuse. A later sum of the same column with another
      function reads it again (~90 ms)
      **Streaming scan built (D155), 2026-11-08.** An aggregate (sum,
      mean, min, max, count, len) over a file's own column of more than one
      row group, not held and not in the cache, is read a row group at a
      time, eight at once on threads: Polars' aggregate runs over each group
      and the partials are put together by the same aggregate (a sum of
      sums, the least of the least; `run_agg`'s guards and the decimal
      bound then run over the combined row, unchanged). A float sum or mean
      carries its running total from group to group in row order, so it is
      the walk's answer to the last bit. A file of one row group is read
      whole as before (a slice would decode the whole group every time).
      Test: `an_aggregate_over_many_row_groups_is_the_one_over_a_single_group_and_holds_no_column`
      (ints, floats with gaps and a NaN, dates, 14 aggregates, 30 groups
      against 1: identical displays, the cache never asked). 1.1 GB,
      `session_csv`, copy in-process: sums 355 -> 560-810 ms (the machine
      was noisy; read per group, not whole), after the sums 699 -> 613-650
      MiB, session peak 909 -> **772-809 MiB** against 794. What is left is
      the ~480 MiB the copy leaves in the process plus the added formula
      column (~155 MiB): with the copy in a child (reverted, in `1f82fb9`)
      the baseline would be ~10 MiB
      **Child copy put back, 2026-11-08** (owner's go-ahead; the code of the
      first attempt, `OMASHEET_COPY_CHILD`, `POLARS_MAX_THREADS=2`,
      `omasheet_cli::main`, `session_csv --here` for in-process).
      1.1 GB, `session_csv`: copy 3.7-4.5 s, 10 MiB after it; after the
      sums 217 MiB; session peak **380 MiB** (target 794), twice over;
      sums 400 ms, edit with sums 7 ms, answers identical. **5.4 met**
- [x] 5.5 By hand in the app: open, scroll, jump, edit, and confirm that the
      window never stops responding
      Done by hand, 2026-11-04 (owner).

## 6. Verification

- [ ] 6.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace`
- [x] 6.2 The differential: for a corpus of CSV, XLSX and JSON files (quoted
      newlines, CRLF, a BOM, no final newline, empty cells, OMX-only
      literals), the inline sheet and the table over the converted copy
      answer the same for every column and every aggregate; and every
      aggregate gives the same answer on the streaming engine as on the
      in-memory one (D155), including NaN, null, empty and decimal columns
      Built and ticked 2026-11-08, with fractions (`1/2`) in a CSV the one
      accepted difference (text in a table): `omasheet-interop/tests/differential.rs`
      (8 awkward CSVs: plain, CRLF, BOM, no final newline, quoted newlines,
      empty cells, percent/fraction literals, mixed; a 3,000-row CSV, a
      2,000-row JSON and a 2,000-row workbook sheet with gaps, dates and
      booleans: every cell shown and every aggregate of every column, inline
      against the table) and the streaming half in `doc.rs`
      (`an_aggregate_over_many_row_groups_...`: 29 aggregates over int,
      float with NaN and gaps, date, decimal, all-empty and all-NaN
      columns, 30 row groups against 1). Everything agrees but **two
      differences the differential found**, of which the first is fixed:
      (1) a column of numbers with a decimal point (`1.50`) was an exact
      `Decimal` inline and a `Float` in a table, so a sum of it differed in
      the last digits (400,615.19 inline, 400,615.1900000002 as a table).
      **Fixed, 2026-11-08:** a table's copy now holds such a column as
      `Decimal(38, s)`, `s` the most places in the head: a CSV column that
      Polars made a float of, whose head values are all plain numbers, is
      read as text and made a decimal by `str.to_decimal` (a value with more
      places than the head had is made not a number first, so the copy fails
      and is worked out again from every row, as a float, rather than the
      digits being dropped); a JSON number written with digits, a point and
      digits, and a workbook float that is a plain decimal to 15 digits, is
      a `Cell::Dec` and a `Kind::Decimal(places)` that widens to the most
      places (a float beside it makes the column a float). Whole numbers
      stay integers, and an exponent (`1e3`) or more than 15 digits stays a
      float. A document that declares such a column `Float` now gets the
      type error it gets for any exact column (`Decimal` is not assignable
      to `Float`): `session_csv` declares `price` and `amount` `Decimal`.
      1.1 GB, `session_csv`, child copy: copy 5.3 s (3.7-4.5 before), sums
      471 ms (400), peak 430 MiB (380), edit with sums 7 ms; and
      `SumPrice` is now `5221245285.26`, exact, where the float sum was
      `5221245285.26108`. (2) a CSV column of percentages or fractions
      (`20%`, `1/2`) is a number inline and text in a table (3.2's OMX
      literals), so it has no sum or mean there. **Percentages fixed,
      2026-11-08**: a CSV column whose head values are all `n%` is made the
      exact decimal each is (`7.5%` is `0.075`, two places more than the
      text), and the copy's file-level metadata names such columns
      (`omasheet.percent`, one to a line), which `fill` reads: a table over
      the copy types them `Percent` where its text declares nothing, so the
      app and the printed `view` lines show `20%`, and a declared `Decimal`
      or `Percent` takes them (exact types are assignable). A value further
      down that is not a percentage fails the copy, which is worked out
      again from every row, as text. Fractions (`1/2`) remain text in a
      table: a fraction is not a number a Parquet column holds exactly. The
      differential holds that as the only difference allowed, and fails if
      a second appears. Hex
      (`0x1F`) showed differently (`0x1f` inline, `0x1F` as a table);
      **fixed 2026-11-08 for a CSV**: a column whose head values are all
      `0x` and hex digits has each such value lowercased in the copy (a text
      further down is left as it is). Only a CSV cell is read as a literal
      by an import, so a JSON or workbook string `"0x1F"` is text in both
      and stays as written (held by a test)
- [x] 6.3 Every `examples/*.expected` byte-identical: a small file still
      opens inline as before
