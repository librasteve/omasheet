# Tasks: Parquet, and tables backed by a dataframe

The second of two stages, on the `polars-parquet` branch, built on
`add-polars-dtypes` (the `polars-dtypes` branch). The type mapping below is a
naming, not a conversion, only once an omasheet type *is* a Polars dtype:
sections 1 to 4 of that change made it so for the types there are now, and
section 2 here uses them. The types still to come there — `Binary`,
`Duration`, a `DateTime` with a zone, and unsigned widths — reach Parquet as
2.9 once they land.

## 0. Settle the open questions

- [ ] 0.1 P34: what `--meta` carries, and what a reader that ignores it sees
- [ ] 0.2 Whether a dataframe-backed table is editable, or read-only for now

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

- [ ] 3.1 A `table` arm in `parse_file`, beside `sheet`, for
      `table Sales := dataframe["data/*.parquet"]`, plus a `FileAst` field and
      the catch-all keyword message
- [ ] 3.2 Recognise `Index(Name("dataframe"), [Lit::String])` in the checker
      and lower it to a source rather than a selection
- [ ] 3.3 `Sheet.source` and `ColKind::Frame`, and every match over `ColKind`
      in the engine, the interop crate and the app
- [ ] 3.4 `nrows` from the Parquet footer, a metadata-only read
- [ ] 3.5 The schema written into the `.omx` text and checked against the
      footer on load, with a diagnostic on a mismatch (D29)
- [ ] 3.6 A glob matching several files, read as one frame

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

- [ ] 6.1 The editing API refuses or diverts on a dataframe-backed table,
      which has a declaration line but no cell spans
- [ ] 6.2 The table's source and row count shown, so it is plain the rows are
      not in the file

## 7. The documents

- [x] 7.1 `docs/interop.md`: Parquet, `--meta`, and the type table
- [ ] 7.2 `docs/format.md`: the `table` statement
- [ ] 7.3 `docs/expressions.md`: `.head(n)`
- [x] 7.4 `.claude/skills/omasheet/SKILL.md`: the statement, the command and
      the types
- [ ] 7.5 `openspec/project.md`: the Roadmap line and the Tech Stack

## 8. Verification

- [ ] 8.1 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `cargo test --workspace --no-fail-fast`
- [ ] 8.2 Open a real multi-file Parquet, and an unknown one, and confirm the
      schema drives the displayed types
