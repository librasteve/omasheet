# Tasks: hold less of a big file

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

- [ ] 1.1 D78: a sliced materialisation, or a selection that stays lazy
      until it is read. The first is the smaller change and keeps the
      column cache; the second is what a row-group filter would want later
- [ ] 1.2 The row range reaches `scan_parquet`, so `head(5)` over a file of
      many row groups reads one
- [ ] 1.3 `omasheet-engine/tests/rowgroups.rs` is the before and the after:
      it asserts 5,000 values and 5 row groups today, and says in its
      message that the day the limit lands it fails with the new figure.
      The new figure is 5 values and 1 row group
- [ ] 1.4 A column read for a range and then wanted whole: read again,
      filled in, or kept as two reads. Whatever it is, the corpus
      differential and the kernel differential say the answers did not move

## 2. A column need not become a value per row

- [ ] 2.1 Only if 0.1 says so. `Value::Series(Rc<PlColumn>)`, with a
      `materialised()` that turns it into a `Vector` at the top of each
      generic path, so that a column read hands back what the frame store
      already holds
- [ ] 2.2 `Passengers[; Pclass == 1].Name.len()` counts without reading
      `Name`'s values, which `spec.rs`'s `unselected_columns_are_not_read`
      records as reading them today
- [ ] 2.3 A sum over a selection taken by Polars — `add-columnar-core`
      task 6.3's last bullet, which left it on the engine because a
      selection answers with a vector

## 3. The snapshot shows what is on screen

- [ ] 3.1 D80: a row range the grid gives, or a snapshot that holds the
      frame and formats a cell as it is shown
- [ ] 3.2 The snapshot formats the rows the grid shows, and the grid asks
      again as it scrolls
- [ ] 3.3 The timing harness again: the snapshot was 1,604.8 ms of a
      4,138.6 ms rebuild at a million rows, and 819.5 of 1,836.0 where
      every column lowers. Both figures are `add-columnar-core` task 7.3's,
      taken on this machine, and this is what they are measured against
- [ ] 3.4 Scrolling a million-row table in the app, which is the one test
      of this that is not a number

## 4. Verification

- [ ] 4.1 `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `cargo test --workspace`
- [ ] 4.2 Every `examples/*.expected` byte-identical, under `Never`, `Auto`
      and `Always`: nothing here changes what a file answers
- [ ] 4.3 The corpus coverage figure unchanged — 61 offered, 18 lowered, 42
      refused, 1 failed as this change begins — since none of this is about
      what lowers
