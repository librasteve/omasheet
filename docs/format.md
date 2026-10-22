# The file format

An `.omx` file is plain text: constants, functions and sheets, in any order.
A line that starts with `#` is a comment.

```omx
# Optional: the time zone of the file.
zone Europe/London

# A named value, and a function.
const TaxRate = 20%
fn WithTax(x) = x * (1 + TaxRate)

sheet Sales

# Optional: the type of a column.
Revenue : Decimal

# A cell that starts with = is a formula.
| Month | Revenue | Cost         | Profit | Gross |
| ----- | ------- | ------------ | ------ | ----- |
| Jan   | 10000   | 6000         | *      | *     |
| Feb   | 12000   | = 7000 + 500 | *      | *     |

# A computed column: one formula, every row. The header row names it where
# it goes, and its cells are *.
Profit := Revenue - Cost
Gross  := WithTax(Revenue)
```

## Sheets

- `sheet Name`, then a header row and data rows, written as a Markdown
  table: each line between outer pipes, `| a | b |`, cells separated by `|`,
  and a `| --- | --- |` line under the header row. A sheet is then a table
  wherever the file is shown as Markdown, a sheet of one column too.
- The outer pipes and the `---` line may be left out, as in older files:
  `a | b`. A header row that starts with `|` says the sheet has outer pipes.
  Without them, a `|` at the start or end of a row is an empty cell.
- The app writes every sheet it changes as a Markdown table, lined up.
- `Name := expression` after the rows is the formula of a computed column.
  The header row names the column, anywhere among the others, and each of
  its cells is `*`.
- A computed column may be written as a
  [sequence](expressions.md#sequences-with-), `Odd := 1, 3 ... Inf`, which
  fills it from the terms given.
- An empty cell is `Null`, and shown as nothing. Arithmetic with it gives
  `Null`, comparison with it is neither true nor false, and an aggregate
  passes over it rather than counting it as zero.

```omx
sheet Sales

| Month | Revenue | Profit | Cost | Tax |
| ----- | ------- | ------ | ---- | --- |
| Jan   | 10000   | *      | 6000 | *   |
| Feb   | 12000   | *      | 7000 | *   |

Profit := Revenue - Cost
Tax    := Profit * 20%
```

`Profit` is the third column and `Tax` the fifth. A formula with no column
of its name in the header row is an error, and so is anything but `*` in a
cell of a computed column.

## Tables: a sheet whose rows are in Parquet

A sheet may read its rows from a Parquet file instead of holding them:

```omx
table Sales := dataframe["data/sales.parquet"]

Month   : String
Revenue : Int
Cost    : Int

Profit := Revenue - Cost
```

`table` takes the place of `sheet` and closes the sheet before it. The
string names one Parquet file, read from beside the `.omx` file. No row is
written in the file, and the row count comes from the Parquet.

A column whose name in the file is not a name a formula could write — one
with a space or a slash in it, which Parquet allows and omasheet does not —
is given a name here with a `from` clause:

```omx
SiblingsSpousesAboard : Int from "Siblings/Spouses Aboard"
```

The quoted name is the file's, used to find the column and to check it; the
name on the left is what formulas use. Form it from the file's own, dropping
whatever a name has no room for, rather than shortening it: `Siblings` would
say something this column does not. A column the file holds and no schema
line names is reported with such a name in the help. Nothing in the file is
renamed, and a sheet that holds its own rows has no use for a `from`, so one
there is reported.

The schema lines are the columns, in the order they are written, and they
say what the file is expected to hold: a column the file holds under another
type, or holds and the schema does not mention, is reported rather than
quietly taken from either side. A table with no schema lines at all takes
its columns from the file, which is how a source is read before its schema
has been written down.

Otherwise the sheet is a sheet: its columns are selected and indexed as any
other, a `:=` column over them calculates for every row, and an aggregate
reads the whole column. What it cannot do is hold an edited cell — there is
no text behind a cell whose row lives in Parquet — so the app shows such a
sheet without editing it, and says so if you try: the way to edit the data
is `omasheet import sales.parquet`, which makes an `.omx` holding the rows
itself.

`examples/analysis.omx` is a working one, over `examples/titanic.parquet`.

## Sheets of formulas alone

A sheet written as `:=` lines with no table of its own has no cells to
write:

```omx
sheet PerHead

Class       := Passengers[Pclass; *]
Fare        := Passengers[Fare; *].Decimal.round(0.01)
GroupSize   := Passengers[Family; *] + 1
FarePerHead := (Fare / GroupSize).round(0.01)
```

Its columns are those lines, in the order they are written, and there is no
header row and no `*` to write for a cell. Its length is the length of the
sheets its formulas pick a row of with `*`: `PerHead` has a row for every
row of `Passengers`, whether those rows are in the text or in a Parquet
file. Where several are read, the longest says how many rows there are, and
a row past the end of a shorter one is empty, as any row outside a sheet is.

An aggregate reads a whole column rather than a row, so a sheet of
aggregates is one row long:

```omx
sheet Summary

Aboard   := Passengers.Survived.count()
Lived    := Passengers.Survived.sum()
MeanFare := Passengers.Fare.avg().round(0.01)
```

Otherwise the sheet is a sheet: its columns are selected and indexed as any
other, and an aggregate reads the whole column. What it cannot hold is a
written cell: there is no text behind one, and no row of its own to add or
delete, so the app refuses those, and a data column with them. Its formulas
are its text, so those are edited as any sheet's — typing into one of its
cells changes that column's formula, a column may be renamed, and another
`:=` column added after it.

Exporting one writes its rows like any other sheet's; importing them back
gives the header row and the `*` cells, since nothing in a worksheet says a
sheet's rows were derived.

## Types

`Int`, `Decimal`, `Fraction`, `Percent`, `Float`, `String`, `Boolean`,
`Date`, `Time`, `DateTime`.
A column with no `Name : Type` line takes its type from its cells.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Decimal` | `19.99`, `1/7`, `2 3/4` — exact |
| `Fraction` | `1/7`, `2 3/4`, `0.125` — exact, shown as a fraction |
| `Percent` | `20%`, `12.5%` — exact, shown as a percentage |
| `Float` | `1.5e3`, `Inf`, `-Inf`, `NaN` — floating point |
| `String` | `hi` or `"hi"`  (escape \|) |
| `Boolean` | `true`, `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15` |
| `DateTime` | `2025-01-31T09:30` |

A cell's written form carries its type where that form is distinctive:
`true`, `2025-01-31`, `09:30`, `20%` and `1.5e3` read as a `Boolean`, a
`Date`, a `Time`, a `Percent` and a `Float` wherever they stand. A
`Name : Type` line is what makes the type the whole column's, and is needed
where the written values would not carry it: a `Float` column of `22` and
`7.25`, which read as an `Int` and a `Decimal` on their own, or a `Fraction`
or `Percent` column over numbers written plainly. Nothing is lost by leaving
a line out where the cells already say it — the app shows a type
[muted](app.md#the-app) where it comes from the cells and plain where it is
declared.

Text written bare may hold Markdown: `a **slow** start`,
`see the [lease](lease.pdf)`. The app shows it as that, and so does a
[rendered document](interop.md#markdown). Text in quotes is as it is:
`"2 * 3 * 4"`. So is the text a formula makes, as it is written in quotes
there; a value read from a cell is Markdown or not as that cell is.
`omasheet print` shows every cell as it is written.

A `Decimal`, a `Fraction` and a `Percent` hold the same exact numbers, and a
column of one takes the others, showing what it holds its own way: `1/7` is
`0.14285714285714285714…` in a `Decimal` column, `1/7` in a `Fraction`
column, and `14.29%` in a `Percent` column.

A `Percent` column takes any exact number and shows it as a percentage: `0.4`
there is `40%`, and so is `= Profit / Revenue` when it comes to two fifths.
A `20%` in a `Decimal` column is the number it stands for, `0.2`.

A `Fraction` column takes any exact number and shows it as a fraction of
whole numbers, with a whole part where there is one: `0.125` there is `1/8`
and `2.75` is `2 3/4`. The fraction is worked out from the number rather
than remembered from the cell, so one nobody wrote is found too — a seventh
added to a seventh shows as `2/7`. Only a fraction whose denominator is
10000 or less is looked for; past that the number is shown as a decimal.
Anything worked out from a fraction is a fraction, so a `:=` column over one
shows fractions without being declared.

A fraction of two whole numbers is an exact `Decimal`: `1/7` and `22/7` are
both read that way, and one may be written with a space, `2 3/4` for `11/4`.
Writing `1/7` in a cell does not make the column a `Fraction`; declare it.

See `examples/types.omx` for every type at work.

Dates and times are always written this way in the file. The app shows and
accepts them in your locale's form.

## Constants, functions, zone

- `const Name = expression`
- `fn Name(a, b) = expression` — sees its parameters, constants and sheets,
  not the row it is called from. The comment above it is its description.
- `zone Area/City` before the first sheet says which time zone the file's
  date-times are in. Without it, they are in the zone of the machine.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
