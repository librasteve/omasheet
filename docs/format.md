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
- An empty cell is empty: there is no `null`.

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

## Types

`Int`, `Decimal`, `Fraction`, `Percent`, `Num`, `Complex`, `Text`, `Bool`,
`Date`, `Time`, `DateTime`.
A column with no `Name : Type` line takes its type from its cells.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Decimal` | `19.99` — exact |
| `Fraction` | `1/7`, `22/7`, `2 3/4` — exact, shown as a fraction |
| `Percent` | `20%`, `12.5%` — exact, shown as a percentage |
| `Num` | `1.5e3`, `Inf`, `-Inf`, `NaN` — floating point |
| `Text` | `hi` or `"hi"`  (escape \|) |
| `Bool` | `true`, `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15` |
| `DateTime` | `2025-01-31T09:30` |

Text written bare may hold Markdown: `a **slow** start`,
`see the [lease](lease.pdf)`. The app shows it as that, and so does a
[rendered document](interop.md#markdown). Text in quotes is as it is:
`"2 * 3 * 4"`. So is the text a formula makes, as it is written in quotes
there; a value read from a cell is Markdown or not as that cell is.
`omasheet print` shows every cell as it is written.

A `Decimal`, a `Fraction` and a `Percent` hold the same exact numbers, and a
column of one takes the others, showing them its own way: a `Decimal` column
shows `1/7` as `0.14285714285714285714…`, and a `Fraction` column `0.25` as
`1/4`.

A `Percent` column takes any exact number and shows it as a percentage: `0.4`
there is `40%`, and so is `= Profit / Revenue` when it comes to two fifths.
A `20%` in a `Decimal` or `Fraction` column is the number it stands for, `0.2`
or `1/5`.

A `Fraction` column shows every number as a fraction, and one of more than
one as a whole number and a part: `2.75` and `11/4` there are both `2 3/4`.
A fraction may be written that way too, with a space: `2 3/4` is `11/4`, and
the app keeps it in the file as `11/4`.

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
