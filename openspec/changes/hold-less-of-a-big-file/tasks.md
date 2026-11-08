# Tasks: hold less of a big file

> **Under way again** (owner, 2026-11-01): items 2 and 3 come before
> `open-a-big-csv`, which depends on them. This replaces the deferral below.
>
> **Deferred** until every pipe stage of `add-sheet-operations-polars` is done
> (owner, 2026-10-28; D89 there). It is the snagging change that measures what a big
> Parquet costs where a part of it would do, and it cannot measure the cost
> through a `group_by` or a join that has not been written.

The snagging change after `add-columnar-core` and `add-sheet-operations-polars`.
Three items, each one left by a change that was about speed rather than
size, and each with its *before* already measured and pinned by a test.

## 0. The measurement that decides one of them

- [x] 0.1 What a whole-column read costs as `Value::Vector` against the
      Polars column it was built from: a `len()` and a `sum()` over a
      column at 10⁵ and 10⁶ rows, timed and with peak memory taken.
      `omasheet-engine/tests/timings.rs` is the harness and is already
      shaped for the row counts.

      This is the measurement `add-columnar-core` task 11.1 was made
      conditional on and that its task 7.3 never took — 7.3 timed the
      phases of a rebuild, not the width of a value. D79 is open until
      this says something

      Measured 2026-10-29, release, `a_column_as_values_against_the_column`
      in `timings.rs`, which gains a counting allocator for the peak. At a
      million rows, a value a cell against Polars on the column:

      | | values | Polars |
      |---|---:|---:|
      | `Whole.len()` | 50 ms, 145 MiB | 1.4 ms, 10 MiB |
      | `Whole.sum()` | 173 ms, 145 MiB | 7.4 ms, 23 MiB |
      | `Real.len()` | 57 ms, 145 MiB | 1.2 ms, 10 MiB |
      | `Real.sum()` | 56 ms, 145 MiB | 56 ms, 145 MiB — refused, D72 |

      About 150 bytes a cell as values, against 8 a row in the column, and
      linear: a tenth of each at 10⁵ rows. Over the NYC taxi year — one
      Parquet of 41,169,720 rows, 869 MiB — the release CLI opens the file
      in 18 ms (6 MiB), and a lowered `len`, `sum` or `max` comes back in
      0.4 to 1.0 s at 0.7 to 1.3 GiB; a float `mean`, a column of values,
      passed an 8 GiB cap in 3.7 s and was killed, and so was `print`,
      every cell formatted as the snapshot formats it. **D79 has its
      answer: a column as values is what a big file cannot afford**

## 1. The limit reaches the scan

- [x] 1.1 D78: a sliced materialisation, or a selection that stays lazy
      until it is read. The first is the smaller change and keeps the
      column cache; the second is what a row-group filter would want later

      Sliced, by Polars' eager `ParquetReader` with `with_slice` (owner,
      2026-11-01). `Held::Column` is the file's path and the column's name
      in it rather than a plan, and is read whole or for a range
- [x] 1.2 The row range reaches `scan_parquet`, so `head(5)` over a file of
      many row groups reads one

      It reaches the reader instead, which skips a row group none of the
      slice is in. `Engine::column` reads the span of the rows it is given
      where that is less than the sheet and the column is not held whole,
      and `cell` answers a `table`'s row out of that range before reading
      the column
- [x] 1.3 `omasheet-engine/tests/rowgroups.rs` is the before and the after:
      it asserts 5,000 values and 5 row groups today, and says in its
      message that the day the limit lands it fails with the new figure.
      The new figure is 5 values and 1 row group

      And it is: `the_first_five_rows_are_read_out_of_the_first_row_group`,
      with `tail(5)` out of the last as a second
- [x] 1.4 A column read for a range and then wanted whole: read again,
      filled in, or kept as two reads. Whatever it is, the corpus
      differential and the kernel differential say the answers did not move

      Read again: the range is dropped once the column is held whole.
      `a_range_then_the_whole_column_reads_the_column_again` reads 5 and
      then 5,000. The differentials are in `cargo test --workspace`

## 2. A column need not become a value per row

- [x] 2.1 Only if 0.1 says so. `Value::Series(Rc<PlColumn>)`, with a
      `materialised()` that turns it into a `Vector` at the top of each
      generic path, so that a column read hands back what the frame store
      already holds

      Built 2026-11-01 as a type private to the aggregate path rather than a
      variant of `Value`: `Items` in `eval.rs`, with `Engine::whole_column`.
      An aggregate's argument that is a whole column the dataframe holds (a
      `table`'s own, or one lowered) is walked in place, in one pass, with
      the memoised cells honoured; anything else is its values, shared
      rather than copied. A series therefore never reaches a generic path,
      so no `materialised()` is needed and no `match` on `Value` changed. It
      becomes a variant when 2.3 needs a series to flow further. Where a
      walk could fail where values would not (a row short, a cell in error
      or busy), the column is left to the values.

      Held to the values by
      `an_aggregate_walks_a_whole_column_as_it_would_its_values` in
      `spec.rs`: ten aggregates, and `max` of two, over seven columns of
      `titanic.parquet`, under each lowering, against the same rows taken by
      a selection, failures and diagnostics included. Measured A/B, release,
      at a million rows under `Never` (0.1's harness): `len` 92 → 29 ms,
      `Whole.sum()` 235 → 205 ms, `Real.sum()` (D72's refusal) 100 → 44 ms,
      145 → 15 MiB each. Over a month of NYC taxi (4.09M rows), release CLI:
      a float `mean` 1.48 s, 634 MB → 0.22 s, 102 MB; `median` 2.33 s,
      634 MB → 1.60 s, 319 MB (it sorts the values, so it keeps them)
- [x] 2.2 `Passengers[; Pclass == 1].Name.len()` counts without reading
      `Name`'s values, which `spec.rs`'s `unselected_columns_are_not_read`
      records as reading them today
      Done 2026-11-08: `len` of a file's column over a selection returns the
      rows the selection keeps, without asking for the column
      (`Engine::field`, factored out of `Ir::Field`, and the `Len` arm of
      `call_uncalculated`). Only a file's own column: a calculated one is
      still read, since its cells can fail and `len` would have said so.
      `unselected_columns_are_not_read` now expects `[(0, 1)]` where it
      expected `[(0, 1), (0, 2)]`; `len_of_a_selected_column_is_the_rows_kept`
      checks the counts (216, 487, 0, 26) and against `count` of a column
      read. Workspace tests, clippy and fmt clean
- [x] 2.3 A sum over a selection taken by Polars — `add-columnar-core`
      task 6.3's last bullet, which left it on the engine because a
      selection answers with a vector
      Done 2026-11-08: an aggregate of a file's column over a selection is
      taken as a whole column's is. The column is read, the kept rows are
      gathered from it in the selection's order, and the plan is the one a
      whole column has (`Engine::run_agg_selected` over `run_agg_over`); the
      column is not held afterwards. Same rules as for a whole column, so a
      float `sum` and `mean` stay on the engine, and a `sum` is bounded by
      the rows kept. A selection within a quarter of the file, which a
      `head` or `tail` is, keeps the engine and its range read (D78):
      `a_range_then_the_whole_column_reads_the_column_again` caught the
      first version reading the whole column for five rows.
      `an_aggregate_over_a_selection_is_taken_by_polars` (was `..._stays_on_
      the_engine`) holds ten expressions to the engine's answer, Never
      against Always. Measured, release CLI, the month of taxi trips (3.7M
      rows), `payment_type == 1`: `passenger_count.sum()` 0.46 s, 510 MB
      → 0.10 s, 269 MB; `fare_amount.max()` 0.24 s, 511 MB → 0.09 s,
      234 MB; the answers are the same

## 3. The snapshot shows what is on screen

- [x] 3.1 D80: a row range the grid gives, or a snapshot that holds the
      frame and formats a cell as it is shown. Both: a paged sheet, by size
- [x] 3.2 The snapshot formats the rows the grid shows, and the grid asks
      again as it scrolls. `doc.rs`'s `a_paged_table_shows_what_a_held_one_does`
      holds a paged table and a view of it to the held ones: blocks, the
      summary, a filter and a sort
- [ ] 3.3 The timing harness again: the snapshot was 1,604.8 ms of a
      4,138.6 ms rebuild at a million rows, and 819.5 of 1,836.0 where
      every column lowers. Both figures are `add-columnar-core` task 7.3's,
      taken on this machine, and this is what they are measured against
- [x] 3.4 Scrolling a million-row table in the app, which is the one test
      of this that is not a number
      Done by hand, 2026-11-04 (owner).

## 4. Verification

- [x] 4.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`
      Checked and ticked 2026-11-08: all three clean (two clippy faults
      fixed on the way: a literal `true` in an `assert_eq!`, and an item
      after a test module)
- [x] 4.2 Every `examples/*.expected` byte-identical, under `Never`, `Auto`
      and `Always`: nothing here changes what a file answers
      Checked and ticked 2026-11-08: `every_example_matches_its_expected_output_either_way`
      and the CLI's `examples_match_their_expected_output` pass
- [x] 4.3 The corpus coverage figure unchanged — 61 offered, 18 lowered, 42
      refused, 1 failed as this change begins — since none of this is about
      what lowers
      Checked and ticked 2026-11-08, with the baseline restated: the harness
      `how_much_of_the_corpus_lowers` now holds **102 offered, 27 lowered, 74
      refused, 1 failed**. The first three moved because the corpus grew
      (Complex's 19 columns, all refused; the type lattice; new examples),
      none of it from this change, which touches how rows are read and not
      what lowers. The one failure is the same fault as the baseline's, in a
      different column: `types.omx`'s `Joined.Whole := Fractions * 3` (the
      baseline's was `Mixes.Doubled := Yield * 2`, `add-columnar-core` 8.3),
      a fraction held as an exact decimal having no room for the integer
      part of a product. Polars returns null, D67's guard discards the plan,
      and the row path answers; `types.expected` is byte-identical

## 5. Moved here

- [x] 5.1 The release binary size and the `cargo build` time, before and
      after Polars, per the risk in `add-dataframe-interop`'s `design.md`.
      Moved from `add-dataframe-interop` task 1.2 (owner, 2026-10-30)
      Checked by hand and approved 2026-11-08 (owner)
- [x] 5.2 `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
      -- -D warnings`, `cargo test --workspace --no-fail-fast` over what
      `add-dataframe-interop` built. Moved from its task 8.1 (owner,
      2026-10-30)
      Checked and ticked 2026-11-08, with 4.1: the same three commands, and
      `--no-fail-fast` shows no failure in any crate
- [x] 5.3 Open a real Parquet file, and an unknown one, and confirm the
      schema drives the displayed types. A source names one file (3.6
      there), so not several. Moved from `add-dataframe-interop` task 8.2
      (owner, 2026-10-30)
      Done by hand, 2026-11-04 (owner).
