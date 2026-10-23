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

- [ ] 1.1 `lazy` in the Polars features, and `propagate_nans` with it
- [ ] 1.2 The release binary size and the `cargo build` time before and
      after, recorded here. The figures to beat: 1.7 MB before Polars, 44 MB
      plain release, 20 MB with the PKGBUILD levers, about eight minutes of
      link time at `lto=fat`
- [ ] 1.3 If the figures are unacceptable, enumerate the features rather than
      taking `lazy`'s defaults, and add this change's and the next one's
      separately

## 2. The layering (D61, D62)

- [ ] 2.1 `polars` in `omasheet-engine`; `omasheet-omx` unchanged and still
      without dependencies
- [ ] 2.2 `omasheet-interop/src/frames.rs` → `omasheet-engine/src/table.rs`
- [ ] 2.3 `omasheet-engine/src/dtype.rs`: `dtype(S)`, `type_of(&DataType)`,
      `column_from_cells(..)` and `value_of(AnyValue, S)`. The two
      near-duplicate `cell` functions in `frames.rs` and `parquet.rs`
      collapse into `value_of`
- [ ] 2.4 `ReadSources`, `Document::set_sources` and `view_sourced` deleted,
      with their call sites in `omasheet-cli` and `omasheet-ui`
- [ ] 2.5 `Options { base: Option<PathBuf> }` and a `Document` field for it,
      so a source is still read beside the `.omx` and not from the cwd. This
      passes every test and breaks for every real user if it is lost
- [ ] 2.6 `add-dataframe-interop` task 3.4 — `nrows` from the Parquet footer,
      a metadata-only read — which this makes natural

## 3. The frame store (D63, D64, D65)

- [ ] 3.1 `ColKind::Frame` loses its `Vec<Cell>` and becomes a marker
- [ ] 3.2 `omasheet-engine/src/frame.rs`: `Frames`, `SheetFrame`,
      `Frames::new(&Program)` from the authored cells, `get`, `set`, `lazy`
- [ ] 3.3 A table's rows read as whole columns, not value by value
- [ ] 3.4 `Engine::cell`'s frame fast path, so every existing caller —
      `cell_shown`, the xlsx writer, `summary` — works unchanged
- [ ] 3.5 The memo grid narrowed to `Vec<Vec<Option<Vec<Slot>>>>`, allocated
      per column on first use
- [ ] 3.6 **Checkpoint: no lowering, everything correct, and opening a large
      table no longer materialises it.** Release here

## 4. The units (D68)

- [ ] 4.1 `Value::Time` in nanoseconds of day, `i64`
- [ ] 4.2 `Value::DateTime` in microseconds
- [ ] 4.3 `+ 1` still one second, and `examples/datetimes.expected` and
      `zones.expected` read before and after rather than only diffed
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
- [ ] 5.7 D71a: `min` and `max` not lowered over a Float column holding a
      `NaN`
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
