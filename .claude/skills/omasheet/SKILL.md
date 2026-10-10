---
name: omasheet
description: Read, write, check and convert Omasheet sheets — plain-text `.omx` spreadsheets with named, typed tables and formulas in OMX, the Omasheet expression language. Use this whenever the user mentions omasheet, an `.omx` file, OMX formulas, or `omx` blocks and `{{ }}` values in Markdown; whenever they want a spreadsheet, budget, ledger or table of calculations kept as text or in Git; and whenever they want to bring an `.xlsx` or `.csv` into a sheet, export one, or render a Markdown report from one. Use it before writing or editing any `.omx` text, even a small change: the syntax is not Excel's and is easy to guess wrong.
---

# Omasheet

Omasheet is a text-native spreadsheet: what Markdown is to a word processor.
A sheet is an `.omx` file of named tables with typed columns, and formulas
are written in OMX, a small array language with exact arithmetic.

It is not Excel in text. There are no cell addresses like `B2`: formulas
name columns and rows. Most mistakes come from writing Excel habits into it,
so read the rules below before writing, and always run the sheet afterwards.

## Running it

The command is `omasheet`. If it is not on the PATH:

- in the omasheet repository, use `cargo run -q -p omasheet-cli --` in its
  place (or `target/debug/omasheet` once built);
- anywhere else, it is not installed. Tell the user, and offer
  `cargo install --git https://github.com/librasteve/omasheet omasheet-cli`.
  Without it a sheet can be written but not checked, so say that it was not.

```sh
omasheet sheet.omx                         # calculate and print every table
omasheet lint sheet.omx                    # check without calculating
omasheet eval sheet.omx 'Orders.Net.sum()' # one expression against a sheet
omasheet eval '1/3 + 1/6'                  # or on its own
omasheet eval --exact sheet.omx '...'      # a Num in full, not 1.41421…
```

Errors come as `file:line:col: error: message` with the line and a marker,
often with a `help:` line that says what to write instead. Read the help.

## The loop

1. Write or edit the `.omx` text.
2. `omasheet lint` it. Fix every error; they are all reported at once.
3. `omasheet` it and read the calculated tables. A cell showing `#ERROR`
   failed, and the reason is printed after the tables.
4. Spot-check a number or two with `omasheet eval`.

Do not report a sheet as working until step 3 has run clean. The language is
young and still changing, so the tool is the authority, not this page. If the
repository's `docs/format.md` and `docs/expressions.md` are there, they are
newer than this page: read them when something here does not work.

## A whole sheet

```omx
# Orders for a small shop.
const VatRate = 20%

# The price with VAT added.
fn WithVat(x) = x * (1 + VatRate)

table Orders

Placed : Date

Item   | Placed     | Qty | Price | Net | Gross | Running
Tea    | 2025-01-31 | 3   | 4.50  | *   | *     | *
Coffee | 2025-02-01 | 2   | 7.25  | *   | *     | *
Mugs   | 2025-02-03 | 6   | 9     | *   | *     | *

Net     := Qty * Price
Gross   := WithVat(Net)
Running := [Net; 0..*].sum()

table Summary

Label    | Value
Total    | = Orders.Gross.sum()
Dearest  | = Orders[; Net == Orders.Net.max()].Item
Big ones | = Orders[; Qty > 2].Item.count()
```

## The format

- `# comment` on a line of its own. The comment above an `fn` is its
  description.
- `const Name = expression` and `fn Name(a, b) = expression`, anywhere. A
  function sees its parameters, constants and tables, not the row it was
  called from: pass the row's values in.
- `zone Europe/London` before the first table, if date-times need a zone.
- `table Name`, a blank line, optional `Column : Type` lines, then the header
  row and the data rows, cells separated by `|`. Line the pipes up; it is
  what makes the file readable and the diffs small.
- Names are identifiers: letters, digits and `_`, no spaces. `UnitPrice`, not
  `Unit Price`.
- **A computed column** has one formula for every row. Name it in the header
  row where it should appear, put `*` in each of its cells, and write
  `Name := expression` after the rows. Leaving it out of the header, or
  putting anything but `*` in its cells, is an error.
- **A formula cell** starts with `=`. Everything else is a literal, so
  `Revenue - Cost` in a cell is text. A `:=` line and a `const` take no `=`.
- An empty cell is empty. There is no `null` or `NA`; leave it blank.
- Text is written bare, `Jan`. Quote it when it would read as something
  else or holds a `|`: `"42"`, `"a|b"`, `"=not a formula"`.

## Types

`Int`, `Decimal`, `Ratio`, `Percent`, `Num`, `Complex`, `Text`, `Bool`,
`Date`, `Time`, `DateTime`.
A column with no type line takes its type from its cells, which is usually
what you want; declare one when the cells could be read another way.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Decimal` | `19.99` — exact |
| `Ratio` | `1/7` — exact |
| `Percent` | `20%` — exact, shown as a percentage |
| `Num` | `1.5e3` — floating point, only when asked for |
| `Bool` | `true`, `false` |
| `Date`, `Time`, `DateTime` | `2025-01-31`, `09:30`, `2025-01-31T09:30` — always ISO |

An `Int` is a `Decimal` and a `Decimal` is a `Ratio`. Sums and products of
decimals are `Decimal`; a division or an average is a `Ratio`, since it may
not end. So a column declared `Decimal` rejects `= Total / 3`: leave the
column undeclared, declare it `Ratio`, or write `(Total / 3).Decimal` if it
is known to end.

A `Percent` is the number it stands for, so `Net * (1 + VatRate)` works as
written and gives a plain number. Only percentages added to percentages stay
a `Percent`. To show a share as a percentage, declare the column, as in
`Margin : Percent` over `Margin := Profit / Revenue`, or write `x.Percent`;
do not multiply by 100. One that does not end shows as `41.67…%`.

Arithmetic is exact: `0.1 + 0.2 == 0.3` is true, `1/3 * 3` is `1`, and
integers do not overflow. An exact number is shown in full: `0.125` if it
ends as a decimal, `54/7` if it does not. A fraction in a result is correct,
not a fault: do not "fix" it with `.Num`, and use `round(x)` only when the
user wants a rounded number. Only a `Num` is shortened, to five digits with
a `…` mark, and `--exact` shows it in full.

## Expressions

| Written | Means |
|---|---|
| `Revenue` | in a table's own formula: this row's value |
| `Sales.Revenue` | the whole column, a vector |
| `Sales[Revenue; 1]` | one cell. Rows count from 0; `-1` is the last |
| `Sales[; Revenue > 100]` | the rows where it is true, every column |
| `Sales[Revenue; *-1]` | the row before this one; `*` is this row |
| `[Revenue; 0..*]` | from the top to this row; the table's name can be dropped inside it |
| `a // b` | `b` when `a` is empty: `[Revenue; *-1] // 0` |

- Selection is `[columns; rows]`, with `;` between them. One column only: for
  several, pipe through `select`:
  `Sales |> filter(Profit > 0) |> select(Month, Profit)`.
- Ranges: `0..2` includes both ends, `0..^2` leaves out the last, `^2` is
  the first two.
- Equality is `==`. A single `=` only starts a formula cell or binds a
  `const`.
- `and`, `or`, `not`, `in`, and `if c then a else b`. There is no `IF()`.
- Functions are called `sum(x)`, `x.sum()` or `x |> sum()`:
  `sum avg min max count`, `abs round floor ceil sqrt` and the usual maths,
  `today() now() year month day weekday`, and the type names as conversions,
  `x.Int`, `x.Num`, `x.Percent`, `x.Text`.
- A date plus a whole number is days later; two dates subtract to days.
- A lookup that finds one row fills a cell with its value; none gives empty,
  so add `// "none"`; several is an error.

## Workbooks, CSV and Markdown

```sh
omasheet import book.xlsx -o book.omx    # a table for each worksheet
omasheet import sales.csv                # one table, printed
omasheet export sheet.omx --xlsx         # sheet.xlsx
omasheet export sheet.omx --csv --table Orders -o orders.csv
omasheet render report.md --html -o report.html
```

- The `.omx` file is the sheet; the others are ways in and out. Only values
  cross: Excel formulas come in as the numbers they last showed, and OMX
  formulas go out as their results. Say so if the user expects otherwise.
- Import and export print `note:` lines for what was renamed, rounded or left
  behind. Pass them on to the user; they are the lossy parts.
- After an import, the sheet is plain data. Turning repeated arithmetic back
  into computed columns is worth offering, and is done by hand.
- In Markdown, `sheets: [budget.omx]` in the front matter names sheets, a
  fenced `omx` block is a sheet shown as its tables, and
  `{{ Sales.Revenue.sum() }}` in the text is replaced by its value.
  `omasheet render` reports mistakes at their line in the document.

## In the omasheet repository

- `examples/*.omx` each have an `.expected` beside them, compared by the
  tests. After changing an example, regenerate it with
  `omasheet examples/name.omx > examples/name.expected` and read the diff;
  do not edit the `.expected` by hand.
- The desktop app is `omasheet-ui`. It opens a real window on the user's
  desktop and building it loads every core, so say so before doing either.
