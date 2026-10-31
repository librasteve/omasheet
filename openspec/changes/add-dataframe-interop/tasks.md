# Tasks: Parquet, and tables backed by a dataframe

The second of two stages, built on `add-polars-dtypes`; both are on the
`polars` branch. The type mapping below is a naming, not a conversion, only
once an omasheet type *is* a Polars dtype: sections 1 to 4 of that change made
it so for the types there are now, and section 2 here uses them. The types
still to come there — `Binary`, `Duration`, a `DateTime` with a zone, and
unsigned widths — reach Parquet as 2.9 once they land.

## 0. Settle the open questions

- [x] 0.1 P34: what `--meta` carries, and what a reader that ignores it sees.
      Settled as written (owner, 2026-10-30): the zone, the type lines, the
      formulas, the constants and the functions, as `.omx` source under the
      key `omasheet`; a reader that ignores it sees the columns alone
- [x] 0.2 D49: a dataframe-backed table is a read-only view. The app shows
      its rows and takes no edit of them; to edit, import the Parquet

## 1. The dependency

- [x] 1.1 `polars` in `[workspace.dependencies]`, with the features the work
      needs and no more: lazy frames, Parquet, and the dtypes.

      **The tick is struck.** Parquet and the dtypes were on; lazy frames
      were not — `Cargo.toml` named no `lazy` feature and `polars-lazy` was
      never compiled, so the claim was read off the intent rather than off
      the file. `add-columnar-core` task 1.1 turns the feature on and 1.2
      measures what it costs

      Ticked again, 2026-10-30, read off the file this time: `Cargo.toml`
      names `lazy`, `parquet` and the five `dtype-` features, and
      `table.rs` scans with `LazyFrame::scan_parquet`
- 1.2 Record the release binary size and the `cargo build` time before and
      after, per the risk in `design.md`. **Moved to
      `hold-less-of-a-big-file`**, the snagging change, as its task 5.1
      (owner, 2026-10-30)

## 2. Parquet at the boundary (`parquet-interop`, `cli`)

- [x] 2.1 `omasheet-interop/src/parquet.rs`, with the two free functions the
      other formats expose — `import(name, bytes)` and `export(name, text,
      sheet, options)` — reusing `Grid`, `Datum`, `write_omx` and `with_file`
      so diagnostics and `--sheet` behave as they do elsewhere
- [x] 2.2 Each column's type from the schema, never inferred (D33)
- [x] 2.3 Nested columns dropped with a notice
- [x] 2.4 `--parquet` on `omasheet export`, with the existing mutual-exclusion
      pattern, and `.parquet` in `omasheet import`
- [x] 2.5 The four hard-coded format lists: the CLI's import allow-list and
      export match, and the app's import match and export suggestion
- [x] 2.6 The two CLI tests that assert the format list in a message
- [x] 2.7 `--meta`, once 0.1 is settled: `parquet::export_meta`, and
      `import` restoring the source it keeps with the file's rows under its
      header. Checked from the CLI over `budget.omx`, and read by Python
      Polars 2.1.0 as an ordinary file
- [ ] 2.9 Once `add-polars-dtypes` section 5 lands: `Binary` and `Duration`
      columns read and written rather than left out, a `DateTime`'s zone kept,
      and an integer column written back at the width and signedness its
      schema had rather than as `int64`
- [x] 2.8 A round-trip over the `examples/` corpus, asserting a second trip is
      a fixed point — the shape of the CSV round-trip test

## 3. The table statement (`file-format`)

- [x] 3.1 A `table` arm in `parse_file`, beside `sheet`, for
      `table Sales := dataframe["data/sales.parquet"]`, plus a
      `SheetDecl.source`
      field (a table is a sheet with a source, per D28, so it keeps its place
      among the sheets) and the catch-all keyword message
- [x] 3.2 Recognise `Index(Name("dataframe"), [Lit::String])` in the checker
      and lower it to a source rather than a selection
- [x] 3.3 `Sheet.source` and `ColKind::Frame`, and every match over `ColKind`
      in the engine and the interop crate. The app's one use is a `matches!`
      for a computed column and needed no arm; the app does not yet read a
      source, which is section 6 and waits on 0.2
- [x] 3.4 `nrows` from the Parquet footer, a metadata-only read.
      `frames::rows` does exactly this and is there for a caller that wants
      only the count, but `frames::read` sets `nrows` from the frame it has
      just read, because without the pushdown of section 4 the values are
      read anyway. The spec scenario "a large table opens without reading its
      rows" is therefore not met yet

      Met since `add-columnar-core` replaced `frames` with `table.rs`
      (2026-10-30): `table::read` takes the height from
      `ParquetReader::num_rows`, the footer alone, and holds each column as
      a plan that reads that column and is collected only when something
      asks for it, so opening a table reads none of its rows
- [x] 3.5 The schema checked against the file on load, with a diagnostic on a
      mismatch (D29): a column the file types differently, a column the file
      has and the schema does not, and a column the schema has and the file
      does not. Writing the schema into the `.omx` text on a first load is
      not done; a table with no schema lines takes its columns from the file
      instead
- [x] 3.6 ~~A glob matching several files~~: not wanted. The `*` in the
      example stood for a file name, not a pattern (owner, 2026-10-21), so a
      source names one Parquet file and a missing one is reported as missing
- [x] 3.7 D50: a `from` clause on a schema line,
      `SiblingsSpousesAboard : Int from "Siblings/Spouses Aboard"`, so a
      Parquet column whose name is not an omasheet name can be read at all.
      The quoted name is what the column is found and checked by; a `from`
      on a sheet of its own rows is reported, and the diagnostic for a
      column no schema line names offers a name formed from the file's own.
      `examples/analysis.omx` reads `titanic.parquet` through two of them

## 4. Pushdown and lowering (`evaluation`) — moved

**Moved to `add-columnar-core`.** Lowering a column needs the sheet's data
to be held as columns in the first place, which this change does not do: it
reads a table's rows as a `DataFrame` and then explodes them into cells.
The decision of 2026-10-23 puts the frame at the centre, and the lowering
goes with it. Where each task went:

- 4.1 Intercept `Ir::Index` → section 6, which is named for it
- 4.2 Lower an elementwise computed column → section 5, the two gates:
      `Column::elementwise` is the first and `lower::expr` the second
- 4.3 Leave row-recursive columns on the engine → section 5 with it, where
      it is what the second gate refuses
- 4.4 Property-test the scalar path against a one-row series → task 8.1,
      the kernel differential, which asks it of a column each way rather
      than of `scalar_op` directly

## 5. `.head(n)` (`omx-expressions`) — moved

**Moved, and the two tasks went different ways.** The desugaring to
`[; ^n]` was wanted sooner than this change, because the name had to be
settled with the rest of the vocabulary: it is `align-polars-vocabulary`
task 5.6, done. The row groups a `.head(5)` reads is `add-columnar-core`
task 6.2, which is where it belongs — it is a question about the plan, and
there is no plan here.

## 6. The app (`app`)

- [x] 6.1 The editing API refuses on a dataframe-backed table, which has a
      declaration line but no cell spans: `Document` reads a table's rows
      through `set_sources`, and every call that would write one back
      answers "table `Sales` is a read-only view of `sales.parquet`:
      import that file to make edits"
- [x] 6.2 The table's source and row count shown, so it is plain the rows are
      not in the file: its tab is marked `▤`, and says
      `titanic.parquet, 887 rows, read-only` when the pointer rests on it
      (owner's choice, 2026-10-30)

## 7. The documents

- [x] 7.1 `docs/interop.md`: Parquet, `--meta`, and the type table
- [x] 7.2 `docs/format.md`: the `table` statement
- [x] 7.3 `docs/expressions.md`: `.head(n)`, among the stages of a sheet
      with `tail(n)`
- [x] 7.4 `.claude/skills/omasheet/SKILL.md`: the statement, the command and
      the types
- [x] 7.5 `openspec/project.md`: the Roadmap line and the Tech Stack, both
      of which name this change and what it brings in

## 8. Verification

- 8.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace --no-fail-fast`. **Moved to
      `hold-less-of-a-big-file`**, the snagging change, as its task 5.2
      (owner, 2026-10-30)
- 8.2 Open a real multi-file Parquet, and an unknown one, and confirm the
      schema drives the displayed types. **Moved to
      `hold-less-of-a-big-file`**, the snagging change, as its task 5.3
      (owner, 2026-10-30)
