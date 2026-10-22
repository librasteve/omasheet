# Tasks: Parquet, and tables backed by a dataframe

The second of two stages, built on `add-polars-dtypes`; both are on the
`polars` branch. The type mapping below is a naming, not a conversion, only
once an omasheet type *is* a Polars dtype: sections 1 to 4 of that change made
it so for the types there are now, and section 2 here uses them. The types
still to come there — `Binary`, `Duration`, a `DateTime` with a zone, and
unsigned widths — reach Parquet as 2.9 once they land.

## 0. Settle the open questions

- [ ] 0.1 P34: what `--meta` carries, and what a reader that ignores it sees
- [x] 0.2 D49: a dataframe-backed table is a read-only view. The app shows
      its rows and takes no edit of them; to edit, import the Parquet

## 1. The dependency

- [x] 1.1 `polars` in `[workspace.dependencies]`, with the features the work
      needs and no more: lazy frames, Parquet, and the dtypes
- [ ] 1.2 Record the release binary size and the `cargo build` time before and
      after, per the risk in `design.md`

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
- [ ] 2.7 `--meta`, once 0.1 is settled
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
- [ ] 3.4 `nrows` from the Parquet footer, a metadata-only read.
      `frames::rows` does exactly this and is there for a caller that wants
      only the count, but `frames::read` sets `nrows` from the frame it has
      just read, because without the pushdown of section 4 the values are
      read anyway. The spec scenario "a large table opens without reading its
      rows" is therefore not met yet
- [x] 3.5 The schema checked against the file on load, with a diagnostic on a
      mismatch (D29): a column the file types differently, a column the file
      has and the schema does not, and a column the schema has and the file
      does not. Writing the schema into the `.omx` text on a first load is
      not done; a table with no schema lines takes its columns from the file
      instead
- [ ] 3.6 ~~A glob matching several files~~: not wanted. The `*` in the
      example stood for a file name, not a pattern (owner, 2026-10-21), so a
      source names one Parquet file and a missing one is reported as missing
- [x] 3.7 D50: a `from` clause on a schema line,
      `SiblingsSpousesAboard : Int from "Siblings/Spouses Aboard"`, so a
      Parquet column whose name is not an omasheet name can be read at all.
      The quoted name is what the column is found and checked by; a `from`
      on a sheet of its own rows is reported, and the diagnostic for a
      column no schema line names offers a name formed from the file's own.
      `examples/analysis.omx` reads `titanic.parquet` through two of them

## 4. Pushdown and lowering (`evaluation`)

- [ ] 4.1 Intercept `Ir::Index` so `RowSel` and `ColSel` become Polars
      `limit`, `filter` and projection (D31)
- [ ] 4.2 Lower an elementwise computed column to a Polars expression, using
      the existing `DepKind`/`reverse` analysis to pick the columns that
      qualify (D30)
- [ ] 4.3 Leave row-recursive columns on the engine, and test a running
      balance beside a lowered column in one sheet
- [ ] 4.4 Property-test the scalar path in `value.rs` against a one-row Polars
      series for each dtype pair

## 5. `.head(n)` (`omx-expressions`)

- [ ] 5.1 A `lower_call` arm desugaring `.head(n)` to `[; ^n]`, following
      `filter` and `select`, plus an entry in the function directory
- [ ] 5.2 Check that `.head(5)` over a large file reads only the first row
      group

## 6. The app (`app`)

- [x] 6.1 The editing API refuses on a dataframe-backed table, which has a
      declaration line but no cell spans: `Document` reads a table's rows
      through `set_sources`, and every call that would write one back
      answers "table `Sales` is a read-only view of `sales.parquet`:
      import that file to make edits"
- [ ] 6.2 The table's source and row count shown, so it is plain the rows are
      not in the file

## 7. The documents

- [x] 7.1 `docs/interop.md`: Parquet, `--meta`, and the type table
- [x] 7.2 `docs/format.md`: the `table` statement
- [ ] 7.3 `docs/expressions.md`: `.head(n)`
- [x] 7.4 `.claude/skills/omasheet/SKILL.md`: the statement, the command and
      the types
- [ ] 7.5 `openspec/project.md`: the Roadmap line and the Tech Stack

## 8. Verification

- [ ] 8.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace --no-fail-fast`
- [ ] 8.2 Open a real multi-file Parquet, and an unknown one, and confirm the
      schema drives the displayed types
