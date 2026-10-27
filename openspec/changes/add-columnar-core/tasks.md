# Tasks: a columnar core

The second of three on the `polars-core` branch, after
`align-polars-vocabulary` and before `add-sheet-operations-polars`. It absorbs
sections 4 and 5 of `add-dataframe-interop`.

Section 3 is a releasable state on its own: the frame store with no lowering
at all, which is where most of the gain per unit of risk is, because it is
what stops a table's rows being exploded into cells.

## 0. The gate

- [x] 0.1 The spike: what polars 0.51 does with `Decimal(38, s)` arithmetic
      and with a `NaN`. Recorded in `design.md` under *What the spike found*.
      Add, subtract and multiply agree with `Dec`; divide and `mean` do not;
      equality and sort order agree on a `NaN`; `max` over one does not

## 1. The dependency

- [x] 1.1 `lazy` in the Polars features, and `propagate_nans` with it
- [x] 1.2 Measured, on this machine, `cargo build --release` from a warm
      tree so that the figures are the feature's marginal cost rather than a
      cold build's:

      | | Before | After |
      |---|---|---|
      | `target/release/omasheet` | 44,553,400 bytes | 50,379,032 bytes |
      | Polars crates compiled | 14 | 19 |
      | That build | 35 s (the omasheet crates alone) | 12 m 07 s |

      So `lazy` costs **5.8 MB, about 13%**, and the five crates are
      `polars-lazy`, `polars-plan`, `polars-expr`, `polars-mem-engine` and
      `polars-stream`. For the record the earlier figures stand: 1.7 MB
      before Polars at all, and 20 MB from the 44 MB with the PKGBUILD
      levers, which this change has not re-measured.
- [ ] 1.3 Whether 13% is worth enumerating features to avoid. It is the
      owner's call and the figure is now known; nothing here depends on it
- [ ] 1.4 The PKGBUILD levers re-measured once the lowering lands, since fat
      LTO folding the Polars generics is where the 44 MB became 20 MB and the
      lazy stack is more of the same generics

## 2. The layering (D61, D62)

- [x] 2.1 `polars` in `omasheet-engine`; `omasheet-omx` unchanged and still
      without dependencies
- [x] 2.2 `omasheet-interop/src/frames.rs` → `omasheet-engine/src/table.rs`
- [x] 2.3 `omasheet-engine/src/dtype.rs`: `type_of(&DataType)` and
      `cell_of(AnyValue, S)`, with the units a date-time is written in.
      `parquet.rs`'s duplicate `type_of` is deleted and imports this one.
      Named `cell_of` and not `value_of` because it makes a `Cell`; the
      `Value`-making twin, and `dtype(S)` and `column_from_cells`, arrive
      with section 3, where there is something to make them from and a
      scale to give a decimal
- [x] 2.4 `ReadSources`, `Document::set_sources` and `view_sourced` deleted,
      with their call sites in `omasheet-cli` and `omasheet-ui`
- [x] 2.5 `Options { base: Option<PathBuf> }` and a `Document` field for it,
      so a source is still read beside the `.omx` and not from the cwd. This
      passes every test and breaks for every real user if it is lost
- [x] 2.6 `add-dataframe-interop` task 3.4 — `nrows` from the Parquet footer,
      a metadata-only read — which this makes natural. Done by 6.2, which
      needed it: with the file scanned lazily there is no frame to take a
      height from, so `table::read` asks `table::rows` for it, and the tool
      written in section 3 and called by nothing now has its caller
- [x] 2.7 `omasheet eval file.omx` reads a `table`'s rows, which it never
      did: the inversion reached only `view_sourced` and `Document`, so the
      eval path saw a table with no rows. Putting the read inside the engine
      made it fall out
- [ ] 2.8 The three other places that compile a program and build an engine
      without reading sources, so a `table` can be neither exported nor
      rendered into a document: `omasheet-interop/src/lib.rs` (the shared
      export path), `omasheet-interop/src/xlsx.rs` (which does not even take
      the caller's options) and `omasheet-md/src/lib.rs`. The gap predates
      this change; closing it is a line each plus the options to carry

## 3. The frame store (D63, D64, D65)

- [x] 3.1 `ColKind::Frame` loses its `Vec<Cell>` and becomes a marker
- [x] 3.2 `Tables` and `Held`, in `table.rs` beside the reader that fills
      them. Two departures from the sketch, both deliberate:

      It holds a `table`'s columns only, **not every sheet's**. Building
      Polars columns from authored cells would add a conversion pass at
      construction and then serve those cells by per-element Arrow access,
      which D31 itself calls pathologically slow — so it would make an
      ordinary file slower for no gain until section 6 gives the columns a
      column-wise reader. An authored column stays `Vec<Cell>` and is served
      exactly as before.

      `Tables` and not `Frames`, because `eval.rs` already has a `Frames`:
      the cursor's stack of rows in scope.
- [x] 3.3 A table's columns kept as the file holds them, not read value by
      value. `Held::Absent` is the third state the store needs: a column the
      file lacks was a column of `Cell::Invalid`, so its cells show as
      errors, and with only "column or nothing" that would have quietly
      become empty cells in the app
- [x] 3.4 `Engine::cell`'s frame path, which makes a transient `Cell` of the
      element and sends it through the same reading as a cell of the text.
      One conversion rather than two, so they cannot come to disagree —
      which matters when the goldens are what stands between a subtle
      difference and a wrong answer. `cell_shown`, the xlsx writer and
      `summary` are untouched
- [x] 3.5 The memo grid narrowed to `Vec<Vec<Option<Vec<Slot>>>>`, allocated
      per column on first use. Contained: the field, its construction, and
      three accesses, all inside `Engine::cell`.

      It does not mean a frame column never gets slots: `Engine::cell`
      memoises one like any other, so showing a table still makes a slot
      vector per column shown. What goes is the allocation up front from
      `nrows × ncols`, and `run()` walks only computed columns and
      constants — so `eval 'T.len()'` over a million rows now makes none,
      where it used to make the whole grid before evaluating anything.

      That last clause was not true when it was written: `run()` walked
      every column, a frame column included, so it made a slot vector per
      column and read every column of the file. Task 6.2 made it true,
      needing it to be.

      Measured: a `Cell` is 96 bytes and a slot holds a `Value` (48 bytes)
      and its tag. A million rows of ten columns was therefore about 960 MB
      of cells, from the loop section 3 deleted, and about 560 MB of slots,
      from the allocation this one deletes.
- [x] 3.6 **Checkpoint: no lowering, everything correct, and opening a large
      table no longer materialises it.**

      What this section does **not** do, so that the gain is not overstated:
      the file is still read whole, eagerly, by `ParquetReader::finish()`.
      What goes is the second copy — reading every value into a `Cell`
      before evaluation begins, which cost the memory twice and the
      conversion once for values nothing asks for. Reading only the rows and
      columns a selection asks for is section 6, and `table::rows` — the
      row count from the footer alone, with no row group read — is the tool
      written for it and still called by nothing (task 2.6).

      Reached: 287 tests, every `examples/*.expected` byte-identical, and
      the whole workspace clean under `fmt` and `clippy`. The goldens were
      confirmed by the owner running `cargo test -p omasheet-cli`, the run
      here having been stopped for system memory rather than for failing.

## 4. The units (D68)

- [x] 4.1 `Value::Time` in nanoseconds of day, `i64`
- [x] 4.2 `Value::DateTime` in microseconds, and `Zoned`'s wall and instant
      with it. `date.rs` is the one module that knows the factors: `DAY`,
      `NANOS`, `MICROS`, `NANOS_PER_MICRO`, `DAY_NANOS`, `DAY_MICROS` and an
      `hms` that splits a time into its clock parts. The copies `dtype.rs`
      and the interop modules kept are gone
- [x] 4.3 `+ 1` still one second, and every golden byte-identical — which
      was the point of doing the rescale on its own, with no literal able to
      hold a fraction of a second yet: a golden that moved would be a bug
      rather than an expectation.

      It found four things the compiler could not. `hour`, `minute` and
      `second` divided nanoseconds by 3600 and 60. A sequence of times took
      its step from the raw difference of its terms, which an `Int` step
      then read as seconds, so `09:00, 09:30 ... Inf` stepped to 17:30.
      `succ` of a time advanced one nanosecond. And the CLI's `--now` had
      two units in one expression — `parse_datetime` in microseconds beside
      a fallback in seconds — which overflowed and silently fell back to the
      real clock, so `--now` was ignored.

      `Options::now` is therefore microseconds now, as a date-time is, and
      the only places that still speak seconds are the two a person reads:
      `base_utc_offset`, and a whole number written beside a date or a time.

      The Parquet export's scaling is gone outright: a `Time` and a
      `Datetime` pass through unscaled, being already in the units Polars
      holds them in
- [ ] 4.4 The sub-second values Parquet holds, which the old division
      truncated, kept. **Still truncated, deliberately**: no literal can
      write a fraction of a second, so a value holding one would show as
      something the file could not write back. It arrives with the syntax,
      below
- [ ] 4.5 Fractional seconds in a literal and in what is shown, and then
      `millisecond`, `microsecond` and `nanosecond`.

      The difference of two times is not a question here: `add-polars-dtypes`
      D22 decided it is a `Duration`, inferred from the difference of two
      dates, times or date-times and written as an ISO 8601 literal
      (`PT1H30M`), which reverses
      `Date - Date -> Int`. So the subtraction never truncates once that
      lands, and nothing here needs it to be a `Decimal`. It is that
      change's task 5.2, and the whole of its section 5 is outstanding;
      `dtype.rs`'s `type_of` returns `None` for `DataType::Duration` until
      it does, so a Parquet `Duration` column cannot be read at all.

      The literal is ISO 8601, as the Duration literal is (owner,
      2026-10-26): a `.` and the fractional digits after the seconds, so
      `09:30:00.123`, `09:30:00.123456`, `09:30:00.123456789`, and
      `2025-01-31T09:30:00.123456`. The fractional part is only written
      where the seconds are, since `09:30.5` would say which of minutes or
      seconds the fraction belongs to only by convention.

      A time takes up to nine fractional digits and a date-time six, which
      is not an inconsistency but the two units D68 settled, and D68 stands
      (owner, 2026-10-26). Polars' `Time` has no time unit at all -- it is
      always nanoseconds of day, and a day of nanoseconds sits far inside
      an `i64`. `DataType::Datetime` is parameterised
      `Nanoseconds | Microseconds | Milliseconds`, so microseconds is
      omasheet's own choice, made to keep the calendar: nanoseconds would
      reach only 1677-09-21 to 2262-04-11, and a spreadsheet holding a date
      outside that is ordinary.

      A date-time literal with a seventh fractional digit is therefore a
      diagnostic, saying that a date-time holds microseconds -- not a
      silent truncation, which is the bug 4.4 is holding the line against.
      `Value::DateTime` stays a bare `i64` of microseconds, as D68 has it,
      with no `TimeUnit` to carry.

      The lexer must take the fraction inside the time branch
      (`omasheet-omx/src/lexer.rs:309,347`). Today `09:30:00.5` lexes as
      `Tok::Time` followed by `.5`, because `.` is not a word character and
      so does not stop the time token.

      Then `millisecond`, `microsecond` and `nanosecond` as the accessors,
      beside `hour`, `minute` and `second`

## 5. The two gates (D66, D67, D74)

- [x] 5.1 `Column::elementwise`, computed in `Checker::order` where
      `col_deps` is still alive and carried through `finish`. A read of a
      constant does not count against it: a constant is one value for every
      row, so it says nothing about which row a formula reaches. The flag is
      false for a column that is not a formula at all — cells of its own, or
      a file's rows — so that it cannot be read as a promise about something
      there is nothing of to lower
- [x] 5.2 `omasheet-engine/src/lower.rs`: `expr(&Node) -> Option<Expr>`,
      total, `None` for anything it does not handle. It takes a `Cx` beside
      the node, which the signature in this line did not allow for: a
      constant's value and the columns in scope come from outside the
      expression, and the `Cx` is also what remembers which columns were
      read, so that the frame built for the plan holds exactly those. It
      answers an `Out`, the expression and its dtype, because the dtype is
      what decides whether the operation above it may be lowered in turn.

      What it handles: a literal and a constant, a read of its own row, `+`,
      `-` and `*` on exact numbers and all four of those and `/` on
      floating-point ones, the six comparisons, `//`, `if`, and `-` and
      `not`. Everything else is declined, which is 44 of the corpus's 54
      elementwise columns (task 8.3) — a function call above all, since no
      call of any kind is lowered yet
- [x] 5.3 `Engine::plan` and `Engine::run_lowered`, with the borrows scoped
      so that none is held across a `collect()`: the plan is built and the
      columns owned before the collect, and `Cx::taken` consumes the context
      rather than lending it out. `Engine::pl_column` is the store's reader —
      a `table`'s own column, a column already lowered, or one built out of
      calculated values by `dtype::column_of`, which is task 2.3's
      `column_from_cells` arriving where there is something to make it from.

      A group of more than one column is not lowered, and does not need to
      be: a group is an SCC, so more than one column in it is columns that
      read each other, which for same-row reads is a circular reference and
      for any other kind is not elementwise
- [x] 5.4 The whole-or-nothing fallback: a failed `collect()` discards the
      lowered result, files nothing, and re-runs the group row-wise. A
      column that comes back the wrong length, and a re-stamp that will not
      cast, go the same way
- [x] 5.5 `Options { lowering: Auto | Never | Always }`, and `Always`
      reporting per column whether lowering was attempted and refused:
      `Engine::attempts`, as `Lowered`, `Refused` or `Failed`. A column the
      row count left alone is not in it, having never been tried.
      `Lowering::Auto` is above `ROW_THRESHOLD`, which is 1024 until D73 is
      measured. **Not exposed on the command line**: D74 wants it as the way
      out for a user whose file miscalculates in the field, and that is a
      flag and its help text, which is the owner's to word
- [x] 5.6 D72: `/` and `mean` not lowered on an exact column; `*` lowered
      with its scale checked; D72a: the precision re-stamped to 38. The
      scale checked is the one the engine would give — the wider of the two
      for `+` and `-`, the sum of them for `*` — and a scale past the 38
      digits a decimal holds is where `Dec` gives up digits after the point,
      so it is refused. Both sides widen to a `Decimal(38, s)`, an `Int64`
      included, because a `Dec` holds 38 digits where an `i64` holds 19 and
      a column of whole numbers that overflows Polars' addition still has an
      answer on the engine. `mean` is not reachable from here at all, an
      aggregate reading a whole column rather than its own row; it is
      restated with the aggregates in 6.3
- [x] 5.7 D71a: `max` lowered to Polars' `nan_max` and `min` to its default
      `min`, with a Float column of nothing but `NaN`s left on the engine for
      `min` — the one case where the two disagree. Landed with 6.3, which is
      what made it reachable. The `NaN`-only column is detected by asking
      `is_not_nan().any()` in the same plan as the `min`, so the one case
      that must go back to the engine costs one more expression rather than
      a pass of its own
- [ ] 5.8 D70: a zoned column lowered, one zone per column, and a column of
      mixed zones left on the row path. Declined for now, and `zones.omx` is
      ten elementwise columns refused: a `Zoned` is a value with its own
      zone here, where Polars carries one zone on the column's dtype, so
      lowering one is a column type this store has no entry for rather than
      an expression `lower::expr` is missing

## 6. Selections and aggregates (the old 4.1, 5.2)

- [x] 6.1 `Ir::Index` intercepted: `RowSel::Pred` → `filter`, `ColSel` → a
      projection, `head` → a limit. Only the first of the three is work on
      the eager path, and it is done: `Engine::run_filter`, intercepting in
      `Engine::rows` where the condition would otherwise be evaluated once
      per candidate row. The condition goes through `lower::expr` like any
      other expression, and what comes back from Polars is the row index
      column of the filtered frame — positions, which is what the engine's
      own selection answers with, so a view is still made of row numbers and
      no cell is read.

      `ColSel` and `head` need no interception here, and saying they do
      would overstate it: a column selection is already indices on the view
      and reads nothing, and `head` resolves to positions by arithmetic over
      the row count. A projection and a limit are things to push at the
      **file**, which is 6.2, and until the file is read lazily there is
      nothing for either to save.

      One condition it does not take: one over rows already selected. The
      rows would have to be gathered into a frame of their own first, which
      is a copy, and it is left until there is a reason to pay for it — such
      a condition is not offered to the gate at all.

      It used to refuse a second kind, one written with `and` or `or`, so
      that `Survived == 1 and Pclass == 1` was the engine's however ordinary
      it looked. That is now taken (D76). The reason it was refused was that
      Polars has Kleene's logic and omasheet had half of it: the
      short-circuit in `eval` made `false and null` false and
      `null and false` null, so `and` read differently each way round. The
      two were brought into line rather than the condition left behind,
      which is also the one widening with a measured case for it — a
      condition is where `and` and `or` are written most
- [ ] 6.2 `.head(5)` over a large file reads only the first row group.
      **Half of it is done — the projection, not the limit.**

      Done: the source is scanned rather than read.
      `LazyFrame::scan_parquet` in place of `ParquetReader::finish`, the
      schema from `collect_schema` and the row count from the footer (2.6),
      so opening a file of a million rows and twenty columns reads neither
      the rows nor the columns. Each of a table's columns is a plan that
      reads that column alone, collected the first time a cell of it is
      asked for and kept from then on, and `Engine::reads` says which were
      read. One column summed out of the eight in `titanic.parquet` reads
      one.

      `Engine::run` had to stop walking frame columns for any of that to
      bite: it calculated every cell of every column, a column whose values
      are in a file included, so every column was read before anything was
      asked. A column with no formula has nothing to calculate, so the group
      is skipped and the column is read when a cell of it is shown or a
      calculation wants it. A column of cells stays, a cell of it being able
      to hold a formula of its own.

      Not done: the limit. A column is still collected whole the first time
      it is touched, so `head(5)` over a file of many row groups reads every
      row group of the columns it shows. Pushing the limit down needs the
      row range to reach the scan, and the engine asks for cells one at a
      time by position — so it means either a sliced materialisation
      (`pl_column` taking the rows wanted, which the snapshot could give it
      once D75's column-wise pass exists) or the selection staying lazy
      until it is read. 10.3 is what would show it working, and that is
      where the row groups read are counted
- [x] 6.3 An aggregate over a frame column taken by Polars, with `fit_agg`
      re-applying the `Percent` or `Fraction` wrapper from `Column::ty`
      (D69) — `examples/budget.expected` is what catches a slip.
      `Engine::run_agg` intercepts in `call` before the argument is
      evaluated, and `lower::aggregate` is the gate. `Engine::aggregates`
      reports what became of each, beside `attempts` for columns.

      Taken: `sum`, `mean`, `min` and `max` of exact numbers, `min` and
      `max` of floating-point ones and of temporal ones, `count` and `len`
      of anything held. A sum or an extreme of exact numbers widens to a
      `Decimal(38, s)` first, an `Int64` included, for the reason `arith`
      widens.

      **A sum is bounded before it is believed (D72d).** This said that a
      column whose total overflows Polars' own addition "still has an
      answer on the engine", which was true of the engine and not of the
      gate: Polars adds decimals in an `i128` and wraps without saying so,
      and nothing here was looking. A hundred thousand quotients held at
      `Decimal(38, 34)` summed to `6481.74` where the row path folds
      `40509.98` — a wrong number, shown, with no diagnostic. So the plan
      takes the two extremes as well, `|sum|` being at most the rows times
      the wider of them, and where that bound will not fit in 38 digits the
      sum is handed back. It is a bound rather than the sum, so it refuses a
      shade early; `Dec`'s 38 digits rather than an `i128`'s 39 is the same
      caution. The bug was there before this task's `mean` and was found by
      measuring it.

      `mean` is there because of what 7.3's fifth table priced it at, and
      it is lowered in halves (D72b): the `sum` and the `count` are asked
      of Polars in the one plan and the single division between them is the
      engine's `Dec`, which is the whole of what D72 was refusing. The row
      path divides the sum it has already dressed and puts a percentage
      back on afterwards, a division being one of the things that takes it
      off, so this does the same; a fraction needs no such thing, anything
      done with one giving one. A column of nothing has no mean, and since
      Polars sums such a column to zero rather than to null it is the count
      that says so.

      `min` and `max` of a `Date`, a `Time` or a `Datetime` are there for
      the same reason (D72c), and were refused by omission rather than for
      one: an extreme of such a column is the extreme of the whole number
      it is held as, which is the order `value::compare` puts two of them
      in. A `Datetime` of another unit or in a zone is not one a sheet
      holds and is still left alone.

      Not taken, each for a reason:

      - `mean` of a floating-point column, the `sum` half of it being the
        one addition order matters to.
      - **`sum` and `product` of floating-point numbers**, which the design
        did not call out and should have: addition of them is not
        associative, so a sum collected in parallel and a sum folded from
        the left are not the same number. `min` and `max` do not care in
        which order they look, so a float is no trouble to them.
      - `median`, `std`, `var`, `quantile`, `first` and `last`, which are
        not refused for a reason so much as not yet written.
      - An aggregate over anything but a whole column: a selection of rows,
        a vector, a block, or several arguments together. That is 6.1's
        work, and `Passengers[; Survived == 1].Age.max()` is the test that
        says so.

      `budget.expected` catches nothing here after all, and `analysis2` is
      what does: `Survived : Percent` over `titanic.parquet` is the test,
      and its sum is `34200%` either way.

      It was the only test for a while, because `column_of` declined a
      `Percent` or a `Fraction` outright — "there is no one dtype to hold it
      as" — so the wrapper D69 asks for was only ever put back on a
      **table's** column. That was wrong, and the fifth table of 7.3 is what
      caught it: the dress is display and `cell_of` puts it back on from
      `Column::ty`, which is exactly what a table's column declared as one
      relies on, so a sheet's own is held as the `Decimal(38, s)` its values
      already are and only the matching dress is read into it (D69a). An
      aggregate over such a column is now taken, and — the larger half —
      the column is one any lowered expression can read, where before it
      was a wall

## 7. The snapshot (D75)

- [x] 7.1 `Engine::column_shown(t, c) -> Option<Vec<Value>>`, one pass per
      column. `Vec<Value>` rather than a `Vec<Shown>`: nothing defined a
      `Shown`, and a `Value` is what all three of the snapshot's readers
      want — `doc.rs` formats one and keeps it, `render_view` and
      `omasheet-md` format one and throw it away.

      `Some` is a column the dataframe is the authority for: one lowered, or
      a `table`'s own. It is walked in order, which needs the column in one
      chunk, so it is rechunked first — a lowered column usually is one
      already and is handed straight back; one read from a file of several
      row groups is a chunk per group, and joining them once is what asking
      for a row at a time pays per row.

      **`None` is every other column, and that is the measurement, not a
      shortcut.** Collecting a calculated column into a vector before
      reading it is *slower* than reading it a cell at a time — those values
      are in the memoisation slots already and `cell_shown` is one clone
      from them, so the vector is a million-element allocation that buys
      nothing. Taken at once first, before this was understood, it cost 16%
      of `budget.omx`'s snapshot at 10⁶ rows and 8% of the wide sheet's. So
      the method says which columns are worth taking at once and the caller
      reads the rest cell by cell.

      It does not fill the memoisation slots either, for the same reason the
      slots are made per column and not per sheet: a million-row column the
      dataframe holds needs none of them, the dataframe being the authority
      and giving the same answer twice. It does *read* a slot already
      filled, so a cell read before the walk is given back as it was read.
- [x] 7.2 `Document::rebuild` looping columns then rows: every column of a
      sheet taken first, then the rows formatted out of what is in hand, and
      the value moved rather than copied so the snapshot costs what it did
      before. A column `column_shown` declines is read cell by cell in the
      row loop as it always was.

      The harness of 7.3 grew a third table for this: the snapshot alone,
      each style on an engine of its own so neither reads what the other
      left behind, over two shapes — `budget.omx`, where one computed column
      of eight is lowered, and the wide sheet with rows, where every column
      is one the dataframe holds.

      | rows | sheet | mode | per cell | per column |
      |---|---|---|---|---|
      | 10³ | budget | Never | 1.6 | 1.6 |
      | 10³ | wide | Always | 0.9 | 0.8 |
      | 10⁵ | budget | Always | 158.4 | 158.1 |
      | 10⁵ | wide | Always | **93.8** | **79.5** |
      | 10⁶ | budget | Never | 1580.8 | 1591.1 |
      | 10⁶ | budget | Always | 1607.0 | 1658.3 |
      | 10⁶ | wide | Never | 821.7 | 853.3 |
      | 10⁶ | wide | Always | **995.7** | **821.1** |

      **Where every column is one the dataframe holds it is 15% of the
      snapshot at 10⁵ rows and 18% at 10⁶**, which is D75's claim and the
      only row of the table that shows anything. Everywhere else the two
      styles are now the same code reading the same slots, so the figures
      are a machine with other things to do and should be read as level —
      the 3% and 4% against are the noise this harness has shown all along,
      not a cost.

      Which also says what the gain is worth: 18% of a snapshot is 5% of a
      rebuild, because `compile` is still the half of it that nothing here
      touches. 7.3's own table is unchanged within noise by the switch, so
      it stands as measured.
- [x] 7.3 `compile()`, `Engine::new`, `run()` and the snapshot timed
      separately on `examples/budget.omx`, before any threshold is chosen.
      `omasheet-engine/tests/timings.rs`, ignored by default since it is a
      measurement and not an assertion:

      ```text
      cargo test -p omasheet-engine --test timings --release -- --ignored --nocapture
      ```

      It grows `budget.omx`'s `Sales` sheet to each row count, keeping its
      columns, its formulas and its constant, and leaves `Summary` out: every
      line of that is an aggregate or a lookup over the whole of `Sales`, none
      of it lowered until section 6, so it would time a row-wise scan of a
      million rows and hide the phases this is asking about. Each figure is
      the quickest of three runs — two single runs of the same work came out
      20% apart.

      Milliseconds, release, this machine, with the thread pool warm:

      | rows | mode | compile | build | run | snapshot | total |
      |---|---|---|---|---|---|---|
      | 10 | Never | 0.0 | 0.0 | 0.0 | 0.0 | **0.1** |
      | 10 | Always | 0.0 | 0.0 | 0.1 | 0.0 | **0.2** |
      | 10³ | Never | 1.2 | 0.0 | 1.0 | 1.6 | **3.7** |
      | 10³ | Always | 1.1 | 0.0 | 1.0 | 1.5 | **3.7** |
      | 10⁵ | Never | 133.2 | 0.0 | 98.0 | 158.3 | **389.5** |
      | 10⁵ | Always | 132.9 | 0.0 | 89.3 | 158.3 | **380.6** |
      | 10⁶ | Never | 1504.6 | 0.2 | 1029.0 | 1604.8 | **4138.6** |
      | 10⁶ | Always | 1364.0 | 0.2 | 932.2 | 1589.3 | **3885.7** |

      Taken twice, the two runs of the whole table came out about 20% apart
      in their absolute figures and the same in their proportions — a
      measurement of a machine that has other things to do. The figures
      below 10⁵ are a millisecond or two either way and should be read as
      "nothing in it" rather than as a difference.

      Polars starts a thread pool the first time anything collects, which
      costs **6.6 ms once** and is measured on a file of its own so that it
      is not charged to the first row of the table.

      Four things it says, and the first is the one that matters:

      1. **`compile` and the snapshot are the rebuild, and `run` is a
         quarter of it.** At a million rows they are 1.50 s and 1.60 s
         against `run`'s 1.03 s: 76% of a rebuild is work lowering cannot
         touch.
         This is the question the design's risk paragraph asked, and the
         answer it said would follow — D75 and a viewport-driven snapshot
         matter more to the app than lowering does, and lowering is
         justified by large tables rather than by the keystroke.
      2. **Lowering's gain here is 9% of `run` and 6% of the rebuild**, and
         that is this gate's gain rather than lowering's ceiling: one of
         `budget.omx`'s four computed columns lowers. `Tax` reads a
         percentage, `Margin` divides exact numbers, `Growth` reads the row
         before it. A sheet whose columns all lower would move this figure
         and the threshold with it.
      3. **It costs a fraction of a millisecond to be wrong at the small
         end**: a tenth of a millisecond at ten rows and nothing measurable
         at a thousand — but per lowered column, so a wide small sheet pays
         it per column.
      4. **`Engine::new` is free**, 0.2 ms at a million rows, which is task
         3.5's narrowed memo grid showing up in a measurement.

      The harness measures a second shape, the one the threshold is really
      for: ten rows, and every column lowerable.

      | columns | mode | run | total |
      |---|---|---|---|
      | 5 | Never | 0.0 | **0.0** |
      | 5 | Always | 0.4 | **0.5** |
      | 20 | Never | 0.0 | **0.1** |
      | 20 | Always | 1.8 | **1.9** |
      | 50 | Never | 0.1 | **0.2** |
      | 50 | Always | 4.3 | **4.4** |

      A plan costs about **0.085 ms per column**, paid per column and not per
      sheet, and nothing in it scales down with the rows. So a fifty-column
      sheet of ten rows is 4.4 ms against 0.2 ms — twenty-two times slower —
      and that is a keystroke, not a batch.

      **A fourth table, and the one that answers what lowering is worth.**
      The two above understate it badly, and it took asking whether it was
      worth doing at all to see why: `budget.omx` lowers one computed column
      of four, and one of the three it refuses is
      `Growth := Revenue / Sales[Revenue; *-1] - 1`, which reads the row
      before it and so is a sequential row walk that nothing can lower. Its
      `run` is the engine's row loop either way, with a columnar garnish.
      The same five-column sheet as above, every column of it lowerable, at
      the four row counts instead of ten rows:

      | rows | mode | compile | run | snapshot | total |
      |---|---|---|---|---|---|
      | 10³ | Never | 0.7 | 0.8 | 0.8 | **2.3** |
      | 10³ | Always | 0.7 | 0.6 | 0.7 | **2.0** |
      | 10⁵ | Never | 82.1 | 82.4 | 77.3 | **241.8** |
      | 10⁵ | Always | 82.8 | **10.4** | 77.8 | **171.0** |
      | 10⁶ | Never | 951.6 | 833.1 | 807.6 | **2592.4** |
      | 10⁶ | Always | 912.9 | **103.4** | 819.5 | **1836.0** |

      **`run` is eight times quicker lowered** — 833.1 ms to 103.4 at a
      million rows, and 82.4 to 10.4 at a hundred thousand, the same factor
      twice. The rebuild as a whole comes down 29%, which is the one
      difference in any of these tables that a person would notice rather
      than measure.

      So the gate works and **coverage is what is left**: 8.3's figure is
      that of 54 elementwise columns in the corpus only 10 lower, and the 44
      refused are nearly all a function call, which `lower::expr` does not
      handle at all. Each one of those is a column moved from the 833 to the
      103. That is worth more than anything left in section 7, and it is
      what the risk paragraph's conclusion should now read against: the
      snapshot and `compile` dominate a rebuild *of a sheet whose columns do
      not lower*, which today is nearly all of them.

      **A fifth table: the aggregates and the filters**, which the four
      above leave out by leaving `Summary` out. `budget.omx`'s `Sales` with
      a `Summary` beside it of six lines — `Revenue.sum()`, `Profit.sum()`,
      `Margin.mean()`, `Closed.max()`, and a sum and a `len()` over
      `Sales[; Profit > 10000]`. The example's own `Best month` line is left
      out, not for being awkward but for being quadratic row-wise: an
      aggregate nested in a condition is evaluated once per candidate row
      and nothing memoises it, so at a million rows it does not finish.

      | rows | mode | compile | run | snapshot | total | aggregates | selections |
      |---|---|---|---|---|---|---|---|
      | 10 | Never | 0.0 | 0.0 | 0.0 | **0.1** | — | — |
      | 10 | Always | 0.0 | 0.6 | 0.0 | **0.6** | 4 of 4 | 2 of 2 |
      | 10³ | Never | 1.2 | 1.8 | 1.5 | **4.5** | — | — |
      | 10³ | Always | 1.1 | 1.6 | 1.6 | **4.3** | 4 of 4 | 2 of 2 |
      | 10⁵ | Never | 147.6 | 187.4 | 162.0 | **497.0** | — | — |
      | 10⁵ | Always | 141.4 | **122.9** | 164.0 | **428.3** | 3 of 4 | 2 of 2 |
      | 10⁶ | Never | 1682.1 | 2171.3 | 1634.4 | **5488.1** | — | — |
      | 10⁶ | Always | 1569.2 | **1394.9** | 1644.3 | **4608.7** | 3 of 4 | 2 of 2 |

      **`run` comes down 34% at 10⁵** — 187.4 ms to 122.9 — and 36% at 10⁶,
      and the rebuild as a whole 14% and 16%.

      The `Summary` sheet's own cost is the difference from the `Sales`-only
      shape, taken from the same run of the harness so that the two are of
      the same machine: its `run` was 101.5 ms `Never` and 90.4 `Always` at
      10⁵, and 1022.0 / 952.0 at 10⁶. So the `Summary` work alone is
      **85.9 → 32.5 ms at 10⁵ and 1149.3 → 442.9 at 10⁶, which is 2.6
      times quicker at both** — two row counts a factor of ten apart
      agreeing on the figure, which an earlier pair of these did not.

      **The aggregate taken at ten rows and handed back at a hundred
      thousand is the interesting one.** All four are taken up to 10³; from
      10⁵ `Margin.mean()` is not, and it is D72d that hands it back rather
      than any of D69a, D72b or the dress: `Margin := Profit / Revenue`
      holds its quotients at `Decimal(38, 34)`, and a hundred thousand of
      those have nowhere to put the total's integer part. Before that bound
      existed the same measurement showed four of four taken and `run` at
      101.9 ms, which was quicker only because the number was wrong.

      So what is left of the `Summary`'s cost is still that one line, and
      what holds it there is no longer the dress but **one scale for the
      whole column meeting a division's 38 digits**. A quotient keeps every
      digit it can, which is the most expensive scale a column can be held
      at and far more than anyone reads; the useful question for a later
      change is whether an exact column has to offer Polars every digit it
      holds, or whether a sum may be taken at a scale wide enough for the
      answer and no wider.

      Both selections' conditions are taken (6.1), but their sums stay on
      the engine, which is 6.3's last bullet.
- [ ] 7.4 D73: the threshold, from a committed timing harness over
      `budget.omx` at 10, 10³, 10⁵ and 10⁶ rows. The harness is 7.3 and the
      figures are above; the number is the owner's. What they support is
      less than was expected of them: lowering is clearly ahead at 10⁵ and
      10⁶, level at 10³, and a tenth of a millisecond behind at 10, so the
      measurement does not single out a crossover — there is no row count
      below which lowering visibly costs anything. **The 1024 in
      `ROW_THRESHOLD` is as defensible as any number between 10² and 10⁴,
      and nothing measured here argues for moving it.**

      What the threshold is for is in 7.3's second table, and it is not the
      long sheet: it is the wide short one. At 0.085 ms a column, paid per
      column and not shrinking with the rows, fifty lowerable columns over
      ten rows cost 4.4 ms a keystroke against the row path's 0.2 ms. That
      is what `Auto` exists to refuse, and it is a stronger case for the
      threshold than the long-sheet figures make

## 8. The tests

- [ ] 8.1 `omasheet-engine/tests/kernel.rs`: for every `(S, S, BinOp)`, over
      a fixed edge table, `scalar_op` against the same operation on a one-row
      frame. Table-driven plus a seeded LCG, the seed printed on failure.
      No new dev-dependency: the workspace has none
- [ ] 8.2 The corpus differential: for each `examples/*.omx`, `Never` and
      `Always` byte-identical to each other and to the committed `.expected`.
      Half of it is in `omasheet-engine/tests/spec.rs` as
      `every_example_calculates_the_same_either_way`: all twelve examples
      give the same output and the same diagnostics either way. What is left
      is the `.expected` side, which only the CLI's own golden test reads,
      and which it reads under `Auto` alone
- [ ] 8.3 The coverage number that falls out of 8.2 — how much of the corpus
      actually lowers. Measured once, by hand, at the end of section 5: of
      100 computed columns in the twelve examples, 54 are elementwise, and
      with lowering forced 10 are lowered, 44 refused and none failed. The
      44 are nearly all a function call — a cast, a date part, a zone, a
      rounding — none of which `lower::expr` handles yet. The harness that
      prints this rather than a test that was read once is what is left

## 9. The documents

- [ ] 9.1 `openspec/project.md`: the Tech Stack bullet that says group, join
      and sort are native, the Phase 3 row, and the Roadmap paragraph
- [ ] 9.2 `add-dataframe-interop`: sections 4 and 5 marked moved here, task
      1.1's tick struck
- [ ] 9.3 `add-sheet-operations` superseded, and the archive convention
      settled — it is the first change to be superseded
- [ ] 9.4 `docs/interop.md` on what a `table` now reads

## 10. Verification

- [ ] 10.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] 10.2 Every `examples/*.expected` byte-identical, under both `Never`
      and `Always`
- [ ] 10.3 A real multi-row-group Parquet opened, and the row groups read
      counted

## 11. If the measurement says so

- [ ] 11.1 `Value::Series(Rc<Column>)`, with a `materialised()` that turns it
      into a `Vector` at the top of the generic paths. Only if 7.3 says
      `Ir::Column` escaping into generic code is actually costing anything
