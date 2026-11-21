# Views over big files

A sheet normally holds its rows in the `.omx` file. A **view** reads them from a Parquet or CSV file instead, so the data stays where it is and the file holds only the schema and the formulas.

```omx
view Sales := dataframe['data/sales.parquet']

Month   : String
Revenue : Integer
Cost    : Integer

Profit := Revenue - Cost
```

`view` takes the place of `sheet`. The string names one Parquet or CSV file, read from beside the `.omx` file. No row is written in the file, and the row count comes from the Parquet.

## Why

- The `.omx` file stays small and diffs cleanly, whatever the size of the data.
- A table of millions of rows is not read into memory whole: the row count comes from the Parquet, and the file is scanned lazily by Polars. How long a large file takes to open is the subject of `examples/showcase/benchmarks.md`.
- Calculated columns and queries over the view run on Polars. See [Queries on Polars](queries.md).

## Schema lines

The schema lines are the columns, in the order they are written, and they say what the file is expected to hold. A column the file holds under another type, or holds and the schema does not mention, is reported rather than quietly taken from either side. A view with no schema lines at all takes its columns from the file, which is how a source is read before its schema has been written down.

A column whose name in the file is not one a formula could write, such as `Siblings/Spouses Aboard`, is given a name with a `from` clause:

```omx
SiblingsSpousesAboard : Integer from 'Siblings/Spouses Aboard'
```

The quoted name is the file's, and the name on the left is what formulas use. Form it from the file's own, dropping whatever a name has no room for, rather than shortening it.

## A CSV

A CSV is named the same way, `dataframe['data/sales.csv']`, and is read through a Parquet copy kept in the cache. Its column types are not written in the file, so they are worked out from its first 10,000 rows, and a schema line must say what was found. A column of plain numbers with a point, `1.5`, `2.25` and `4`, is `Decimal`, read exactly, with as many places as the most any value has (up to 18). A column of whole numbers is `Integer`. A column with an exponent (`1.5e3`), a `NaN` or `Inf`, or more than 18 places is `Float`.

Parquet carries its types and is read as they are. A Parquet column stored as `Float`, a price say, can be read as exact decimals all the same: give its schema line `Decimal`, as in `Price : Decimal`, and it is converted as it is read.

## What a view cannot do

It cannot hold an edited cell, because there is no text behind a cell whose row lives in Parquet, so the app shows such a sheet without editing it. The way to edit the data is `omasheet import sales.parquet`, which makes an `.omx` holding the rows itself.

## A working example

`examples/analysis.omx` is a view over `examples/titanic.parquet`, with sheets made from it by `filter`, `sort` and `group_by`.

## Where next

[The file format](format.md#views-a-sheet-whose-rows-are-in-parquet-or-csv) is the reference for views, and [Parquet](interop.md#parquet) says how Omasheet reads and writes the types, including keeping a file's formulas in the Parquet metadata with `--meta`.
