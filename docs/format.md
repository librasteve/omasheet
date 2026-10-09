# The sheet format

An `.omx` file is plain text: constants, functions and tables, in any order.
A line that starts with `#` is a comment.

```omx
# Optional: the time zone of the sheet.
zone Europe/London

# A named value, and a function.
const TaxRate = 20%
fn WithTax(x) = x * (1 + TaxRate)

table Sales

# Optional: the type of a column.
Revenue : Ratio

# A cell that starts with = is a formula.
Month | Revenue | Cost         | Profit | Gross
Jan   | 10000   | 6000         | *      | *
Feb   | 12000   | = 7000 + 500 | *      | *

# A computed column: one formula, every row. The header row names it where
# it goes, and its cells are *.
Profit := Revenue - Cost
Gross  := WithTax(Revenue)
```

## Tables

- `table Name`, then a header row and data rows, cells separated by `|`.
- A `---|---` line under the header is allowed and ignored.
- `Name := expression` after the rows is the formula of a computed column.
  The header row names the column, anywhere among the others, and each of
  its cells is `*`.
- An empty cell is empty: there is no `null`.

```omx
table Sales

Month | Revenue | Profit | Cost | Tax
Jan   | 10000   | *      | 6000 | *
Feb   | 12000   | *      | 7000 | *

Profit := Revenue - Cost
Tax    := Profit * 20%
```

`Profit` is the third column and `Tax` the fifth. A formula with no column
of its name in the header row is an error, and so is anything but `*` in a
cell of a computed column.

## Types

`Int`, `Ratio`, `Num`, `Complex`, `Text`, `Bool`, `Date`, `Time`, `DateTime`.
A column with no `Name : Type` line takes its type from its cells.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Ratio` | `19.99`, `20%`, `1/7` — exact; shown to five digits, `0.14286…` |
| `Num` | `1.5e3` — floating point |
| `Text` | `hi` or `"hi"`  (escape \|) |
| `Bool` | `true`, `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15` |
| `DateTime` | `2025-01-31T09:30` |

Dates and times are always written this way in the file. The app shows and
accepts them in your locale's form.

## Constants, functions, zone

- `const Name = expression`
- `fn Name(a, b) = expression` — sees its parameters, constants and tables,
  not the row it is called from. The comment above it is its description.
- `zone Area/City` before the first table says which time zone the sheet's
  date-times are in. Without it, they are in the zone of the machine.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
