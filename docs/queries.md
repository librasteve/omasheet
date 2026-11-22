# Queries on Polars

A sheet can be asked questions that make another sheet: keep some rows, put them in order, pick columns, group and total. These are the operations of a dataframe library, and Omasheet runs them on [Polars](https://pola.rs), so they work on a table of millions of rows as readily as on a dozen.

## Stages

A sheet goes through stages with `|>`, each giving a sheet:

```omx
Passengers |> filter(Survived == 1) |> sort(Age) |> select(Name, Age) |> head(5)
```

| Stage | Keeps or makes |
|---|---|
| `filter(condition)` | the rows the condition is true of |
| `select(columns)` | those columns, in that order |
| `sort(columns)` | the rows in order, ascending, or descending with `desc` as the last word: `sort(Fare, desc)` |
| `head(n)`, `tail(n)` | the first or last `n` rows |
| `group_by(keys) \|> agg(aggregates)` | a row for each value of the key columns, with the aggregates worked out over the group |
| `rollup(keys) \|> agg(aggregates)` | the same as blocks: a head row for each group of the outer keys, the groups inside it, and a summary row at its foot |
| `group_by(keys) \|> apply(Fn)` | calls the function `Fn` on the rows of each group |
| `join(Other, keys)` | a row for every pair of rows whose key columns are equal; `join(Other, keys, left)` keeps a left row that matches nothing |

Ties in `sort` keep the order they came in, and an empty cell sorts first ascending and last descending.

## A sheet from a query

`group_by`, `apply` and `join` make rows of their own, so each is the whole of a sheet's formula, and a sheet is made of one with `sheet Name := …`. What comes after it is written over that sheet by name:

```omx
sheet ByClass := Passengers |> group_by(Pclass) |> agg(Aboard := Name.len(), Lived := Survived.sum(), MeanFare := Fare.mean())

sheet Dearest := Passengers |> sort(Fare, desc) |> select(Name, Fare) |> head(3)
```

Over the Titanic table in `examples/titanic.parquet` that prints:

```text
sheet ByClass

Pclass | Aboard | Lived | MeanFare
-------|--------|-------|---------
     1 |    216 |   136 |  84.155·
     2 |    184 |    87 |  20.662·
     3 |    487 |   119 |  13.708·
```

In `agg` a column's name is that column over the group, so it needs an aggregate: `Fare.mean()`, not `Fare`. An aggregate of one column is named for it, `Revenue_sum`; `Name := …` names one, and an aggregate of anything else must be named. A stage that makes its own rows cannot be followed by another in the same formula, and the error says to name the sheet:

```text
error: `agg` makes a sheet of its own, so it ends a sheet's formula
  = help: write it as `sheet ByRegion := Sales |> group_by(Region) |> agg(Revenue.sum())`, and use the sheet by its name from there
```

## Functions per group

`group_by(keys) |> apply(Fn)` calls a function once on the rows of each group:

```omx
fn Takings(rows) = (rows.Units * rows.Price).sum()
```

What the function gives sets what the group makes: a value makes one row, a sheet makes its rows. `Fn` may take the group's best rows, `rows |> sort(Units, desc) |> head(2) |> select(Rep, Units)`, or look up rows of another sheet by the group's key, as a join.

## Lazy, and streaming

The work is Polars', which plans a query before it runs it. A view over a Parquet or CSV file is scanned as a lazy frame, not read into memory whole, and the stages of a query run on lazy frames too. A CSV is read once into a Parquet copy, which Polars' streaming engine writes without holding the whole file in memory, and the copy is kept in the cache for the next open (see [Views over big files](big-files.md)).

A formula column over a table big enough to be worth it is calculated by Polars in one step over the whole column when every part of the formula is one it can do. The answer is the one the engine would give a row at a time, to the digit. A formula with a part Polars cannot do is calculated by the engine, whole, and gives the same answer more slowly. [Expressions](expressions.md#calculated-in-polars) lists what each is.

## Reading the results

A query's result is a sheet, so it can be selected from, aggregated and shown in a report like any other: `ByClass[MeanFare; 0]`, or `{{ ByClass }}` in a Markdown document. See [Markdown reports](reports.md).
