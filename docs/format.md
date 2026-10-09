# The sheet format

An `.omx` file is plain text: constants, functions and tables, in any order.
A line that starts with `#` is a comment.

```omx
# Optional: the time zone of the sheet.
zone Europe/London

# A named value, and a function.
const TaxRate = 20%
func WithTax(x) = x * (1 + TaxRate)

table Sales

# Optional: the type of a column.
Revenue : Ratio

# A cell that starts with = is a formula.
Month | Revenue | Cost
Jan   | 10000   | 6000
Feb   | 12000   | = 7000 + 500

# A computed column: one formula, every row.
Profit := Revenue - Cost
Gross  := WithTax(Revenue)
```

## Tables

- `table Name`, then a header row and data rows, cells separated by `|`.
- A `---|---` line under the header is allowed and ignored.
- `Name := expression` after the rows adds a computed column.
- An empty cell is empty: there is no `null`.

## Types

`Int`, `Ratio`, `Num`, `Complex`, `Text`, `Bool`, `Date`, `Time`, `DateTime`.
A column with no `Name : Type` line takes its type from its cells.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Ratio` | `19.99`, `20%`, `1/7` — exact |
| `Num` | `1.5e3` — floating point |
| `Text` | `hi` or `"hi"`  (use `\``|`) |
| `Bool` | `true`, `false` |
| `Date` | `2025-01-31` |
| `Time` | `09:30`, `09:30:15` |
| `DateTime` | `2025-01-31T09:30` |

Dates and times are always written this way in the file. The app shows and
accepts them in your locale's form.

## Constants, functions, zone

- `const Name = expression`
- `func Name(a, b) = expression` — sees its parameters, constants and tables,
  not the row it is called from. The comment above it is its description.
- `zone Area/City` before the first table says which time zone the sheet's
  date-times are in. Without it, they are in the zone of the machine.
