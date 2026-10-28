# Design: hold less of a big file

## Context

Source: the conversation of 2026-10-28, at the end of `add-columnar-core`.
D/P/Q numbers continue from that change, whose last is D77. On the
`polars-core` branch.

The question that started it: opening a big Parquet should not cost its
size in memory. Opening one already does not — `table::read` takes the
schema and the row count from the footer and reads no row group — so the
cost is in what the first question does, and three things do it. They are
written up in the proposal; this is what has been measured about them and
what the measurement does not yet say.

## What is measured, and what is not

`add-columnar-core` task 10.3 pins the first: a table over five row groups
of a thousand rows, asked for its first five rows, reads 5,000 values. The
same test says a sum reads 5,000 and opening reads 0, so it is the limit
alone that does not reach the scan, and `Engine::rows_read` is the number
that would move.

Its task 7.3 pins the third: at a million rows the snapshot is 1,604.8 ms
of a 4,138.6 ms rebuild, and 819.5 of 1,836.0 where every column lowers —
two fifths of the work, for rows no one is looking at.

The second is **not measured**, and that is the first task here. The cost
of `Value::Vector` over a Polars column is a boxed value per row against a
packed buffer, which is an argument and not a figure; `add-columnar-core`
task 11.1 made the work conditional on a measurement that its task 7.3
never took, because 7.3 timed the phases of a rebuild rather than the
width of a value. A count over a column at 10⁵ and 10⁶ rows, timed and
measured for peak memory, is what decides whether `Value::Series` is worth
a variant.

## Decisions

| | | |
|---|---|---|
| D78 | Open | Whether a row range reaches the scan by a sliced materialisation — `pl_column` taking the rows wanted — or by a selection that stays lazy until it is read. The first is the smaller change and keeps the column cache; the second is what a filter would want later |
| D79 | Open | Whether `Value::Series` is worth its variant, which D78's sibling measurement decides. If it is not, a column read keeps building a vector and this change is two items rather than three |
| D80 | Open | What the snapshot asks for: a row range given by the grid, or a snapshot that holds the frame and formats a cell as it is shown. The second is nearer D75's column-wise pass and further from what the app does today |

## Risks

- **A sliced read and a cached column are the same cache.** A column read
  for rows 0..5 and then wanted whole has to be read again or filled in,
  and a cache that answers the wrong rows is a wrong answer rather than a
  slow one. Whatever D78 settles, the differential over the corpus and the
  kernel differential are what hold it to the row path's answers
- **The snapshot is what the app draws from.** Formatting fewer rows means
  the grid asks again as it scrolls, so a stale or missing row is a visible
  defect where today there is none
