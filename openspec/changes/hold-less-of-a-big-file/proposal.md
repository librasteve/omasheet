# Proposal: hold less of a big file

## Why

Opening a big Parquet is cheap, and nothing after it is. `add-columnar-core`
left a table's rows where they are: the footer gives the schema and the row
count, each column is a plan of its own, and a column no formula names is
never read (its task 6.2). What that change did not do is bound what happens
once something *is* asked, and three places then materialise the whole file
where they need a part of it.

- **A column is read whole.** The projection reaches the scan and the limit
  does not, so `head(5)` over a file of five row groups reads five of them
  where the answer is in the first. Measured, and pinned by a test:
  `add-columnar-core` task 10.3, `omasheet-engine/tests/rowgroups.rs`.
- **A whole-column read is expanded into one `Value` per row.** The only
  value that holds a column is `Value::Vector(Rc<Vec<Value>>)`, so a count
  over a column builds a boxed value for every row of it before counting
  them — of a column that is sitting in the frame store as Arrow.
  `Passengers[; Pclass == 1].Name.len()` reads `Name` for that reason, which
  `spec.rs` records.
- **The snapshot formats every cell.** It is 1.60 s of a 4.14 s rebuild at a
  million rows, and 45% of one where every column lowers
  (`add-columnar-core` task 7.3), and the grid shows thirty rows of it.

Each is a snag left by a change that was about speed rather than size, and
together they are why a file too big for memory is too big for omasheet even
though Parquet is read a row group at a time.

## What Changes

- **evaluation** — MODIFIED: a row range selected from a sheet whose rows
  come from a source SHALL reach the scan, so that only the row groups the
  answer needs are read. ADDED: a whole column may be held as the Polars
  column itself and turned into a vector of values only where something that
  cannot read a column asks for one
- **app** — MODIFIED: the snapshot formats the rows the grid shows rather
  than every row of the sheet

## Impact

- `omasheet-engine`: `table.rs` (a plan that carries a row range),
  `eval.rs` (`pl_column`, and the generic paths that would meet a series),
  `value.rs` (the new variant and its `materialised()`), `doc.rs` (the
  snapshot)
- `omasheet-ui`: the grid asks for the rows it shows, and asks again as it
  scrolls
- No change to the `.omx` format, to what any expression means, or to a
  number any of them answers. Every figure in `add-columnar-core` tasks 7.3
  and 10.3 is the *before* of this change, and the tests that hold those
  figures are what would say it worked

## Out of Scope

- Anything that changes what a file means. This change is about what is read
  and what is held, and a differential against the row path is the check
- Row-group statistics and predicate pushdown — reading only the row groups
  a `filter` can match. It is the same family and a change of its own, and
  it needs the statistics a writer may not have written
- Spilling to disk, or any evaluation that does not fit in memory at all

## Depends on

`add-columnar-core`, whose frame store and plans this bounds. It is the
snagging change after the current set: `add-sheet-operations-polars` is
Phase 3 and comes first, since group and join over a big file would
otherwise be written twice.
