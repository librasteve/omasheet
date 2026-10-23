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
- [ ] 2.6 `add-dataframe-interop` task 3.4 — `nrows` from the Parquet footer,
      a metadata-only read — which this makes natural
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
      truncated, kept
- [ ] 4.5 `millisecond`, `microsecond` and `nanosecond`, which this makes
      meaningful

## 5. The two gates (D66, D67, D74)

- [ ] 5.1 `Column::elementwise`, computed in `Checker::order` where
      `col_deps` is still alive and carried through `finish`
- [ ] 5.2 `omasheet-engine/src/lower.rs`: `expr(&Node) -> Option<Expr>`,
      total, `None` for anything it does not handle
- [ ] 5.3 `Engine::plan` and `Engine::run_lowered`, with the borrows scoped
      so that none is held across a `collect()`
- [ ] 5.4 The whole-or-nothing fallback: a failed `collect()` discards the
      lowered result, files nothing, and re-runs the group row-wise
- [ ] 5.5 `Options { lowering: Auto | Never | Always }`, and `Always`
      reporting per column whether lowering was attempted and refused
- [ ] 5.6 D72: `/` and `mean` not lowered on an exact column; `*` lowered
      with its scale checked; D72a: the precision re-stamped to 38
- [ ] 5.7 D71a: `max` lowered to Polars' `nan_max` and `min` to its default
      `min`, with a Float column of nothing but `NaN`s left on the engine for
      `min` — the one case where the two disagree
- [ ] 5.8 D70: a zoned column lowered, one zone per column, and a column of
      mixed zones left on the row path

## 6. Selections and aggregates (the old 4.1, 5.2)

- [ ] 6.1 `Ir::Index` intercepted: `RowSel::Pred` → `filter`, `ColSel` → a
      projection, `head` → a limit
- [ ] 6.2 `.head(5)` over a large file reads only the first row group
- [ ] 6.3 An aggregate over a frame column taken by Polars, with `fit_agg`
      re-applying the `Percent` or `Fraction` wrapper from `Column::ty`
      (D69) — `examples/budget.expected` is what catches a slip

## 7. The snapshot (D75)

- [ ] 7.1 `Engine::column_shown(t, c) -> Vec<Shown>`, one pass per column
- [ ] 7.2 `Document::rebuild` looping columns then rows
- [ ] 7.3 `compile()`, `Engine::new`, `run()` and the snapshot timed
      separately on `examples/budget.omx`, before any threshold is chosen
- [ ] 7.4 D73: the threshold, from a committed timing harness over
      `budget.omx` at 10, 10³, 10⁵ and 10⁶ rows

## 8. The tests

- [ ] 8.1 `omasheet-engine/tests/kernel.rs`: for every `(S, S, BinOp)`, over
      a fixed edge table, `scalar_op` against the same operation on a one-row
      frame. Table-driven plus a seeded LCG, the seed printed on failure.
      No new dev-dependency: the workspace has none
- [ ] 8.2 The corpus differential: for each `examples/*.omx`, `Never` and
      `Always` byte-identical to each other and to the committed `.expected`
- [ ] 8.3 The coverage number that falls out of 8.2 — how much of the corpus
      actually lowers

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
