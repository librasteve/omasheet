# Your first sheet

This chapter builds a small sheet from nothing, checks it, and reads values back out of it. It is the Benchmark sheet from the front page: how long it takes to show the first screen of a big file, for a few kinds of file, against a target.

## Write it

An `.omx` file is plain text. Make `benchmark.omx`:

```omx
const Target = 50

sheet Benchmark

| Run             | FileSize | Rows     | Cold | Warm | Change | Beats_50ms |
| --------------- | -------- | -------- | ---- | ---- | ------ | ---------- |
| CSV, large      | 1060     | 20800000 | 57   | 27   | *      | *          |
| XLSX, large     | 42       | 1048575  |      | 23   | *      | *          |
| Parquet, medium | 236      | 20800000 | 12.8 | 10.8 | *      | *          |
| Parquet, large  | 610      | 23900000 | 45.6 | 37   | *      | *          |

Change := (Warm/Cold-1).Percent
Beats_50ms := Cold <= Target
```

Three things are going on:

- `const Target = 50` is a constant, a name for a value, usable anywhere in the file.
- `sheet Benchmark` starts a sheet, and the table under it holds its data. A cell with a `*` has no value written in it: the column's formula gives it one.
- `Change := …` and `Beats_50ms := …` are formulas. A formula is written once, for the whole column, and applies down every row. The empty `Cold` cell for the workbook is a missing value, and it stays missing in the columns that read it.

## Check it

```sh
omasheet lint benchmark.omx
```

`lint` reads the file without calculating it and reports every mistake at once, each with its line and, usually, a `help:` line saying what to write instead. No output means the file is sound.

## Calculate it

```sh
omasheet print benchmark.omx
```

```text
sheet Benchmark

Run             | FileSize | Rows     | Cold | Warm | Change  | Beats_50ms
----------------|----------|----------|------|------|---------|-----------
CSV, large      |     1060 | 20800000 |   57 |   27 | -52.63% | false
XLSX, large     |       42 |  1048575 |      |   23 |         |
Parquet, medium |      236 | 20800000 | 12.8 | 10.8 | -15.63% | true
Parquet, large  |      610 | 23900000 | 45.6 |   37 | -18.86% | true
```

The `*` cells are filled in. `Change` is a `Percent` because the formula says `.Percent`, and the workbook row has no `Change` and no `Beats_50ms`, since there is no `Cold` figure to work from.

## Ask it questions

`omasheet eval` works out one expression, against the file if you name it:

```sh
omasheet eval benchmark.omx 'Benchmark[Cold; 0]'            # 57
omasheet eval benchmark.omx 'Benchmark.Cold.min()'          # 12.8
omasheet eval benchmark.omx 'Benchmark[Run; Cold > 50]'     # ['CSV, large']
```

`Benchmark[Cold; 0]` is the `Cold` column, row 0: columns are named, not lettered, and rows count from zero. `Benchmark.Cold.min()` is a method on the column. [Expressions](expressions.md#selecting-with-columns-rows) has the whole of selecting.

## Open it

`omasheet benchmark.omx` opens the same file in the app, where you can edit a cell, add a row, and watch the formula columns follow. The file on disk stays the text you wrote, so it diffs cleanly in Git.

## Where next

- Giving columns a type, with units and formats, is in [the file format](format.md#types), and a tour of the number types is in [Choosing a number type](number-types.md).
- Numbers with units, such as `Duration[ms]` for the times above, are in [Units and dimensions](units.md).
- Putting the sheet in a report is in [Markdown reports](reports.md).
