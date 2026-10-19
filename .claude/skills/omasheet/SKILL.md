---
name: omasheet
description: Read, write, check and convert Omasheet files — plain-text `.omx` spreadsheets with named, typed sheets and formulas in OMX, the Omasheet expression language. Use this whenever the user mentions omasheet, an `.omx` file, OMX formulas, or `omx` blocks and `{{ }}` values in Markdown; whenever they want a spreadsheet, budget, ledger or sheet of calculations kept as text or in Git; and whenever they want to bring an `.xlsx` or `.csv` into a file, export one, or render a Markdown report from one. Use it before writing or editing any `.omx` text, even a small change: the syntax is not Excel's and is easy to guess wrong.
---

# Omasheet

Omasheet is a text-native spreadsheet: what Markdown is to a word processor.
An `.omx` file holds named sheets with typed columns, and formulas
are written in OMX, a small array language with exact arithmetic.

It is not Excel in text. There are no cell addresses like `B2`: formulas
name columns and rows. Most mistakes come from writing Excel habits into it,
so read the rules below before writing, and always run the file afterwards.

## Running it

The command is `omasheet`. If it is not on the PATH:

- in the omasheet repository, use `cargo run -q --` in its
  place (or `target/debug/omasheet` once built);
- anywhere else, it is not installed. Tell the user, and offer
  `cargo install --git https://github.com/librasteve/omasheet omasheet-ui`
  (it needs Qt 6 to build and to run).
  Without it a file can be written but not checked, so say that it was not.

```sh
omasheet print file.omx                   # calculate and print every sheet
omasheet lint file.omx                    # check without calculating
omasheet eval file.omx 'Orders.Net.sum()' # one expression against a file
omasheet eval '1/3 + 1/6'                  # or on its own
omasheet eval --exact file.omx '...'      # every number in full, not 1.8446744073709551…e+19
```

Errors come as `file:line:col: error: message` with the line and a marker,
often with a `help:` line that says what to write instead. Read the help.

## The loop

1. Write or edit the `.omx` text.
2. `omasheet lint` it. Fix every error; they are all reported at once.
3. `omasheet` it and read the calculated sheets. A cell showing `#ERROR`
   failed, and the reason is printed after the sheets.
4. Spot-check a number or two with `omasheet eval`.

Do not report a file as working until step 3 has run clean. The language is
young and still changing, so the tool is the authority, not this page. If the
repository's `docs/format.md` and `docs/expressions.md` are there, they are
newer than this page: read them when something here does not work.

## A whole file

```omx
# Orders for a small shop.
const VatRate = 20%

# The price with VAT added.
fn WithVat(x) = x * (1 + VatRate)

sheet Orders

Placed : Date

| Item   | Placed     | Qty | Price | Net | Gross | Running |
| ------ | ---------- | --- | ----- | --- | ----- | ------- |
| Tea    | 2025-01-31 | 3   | 4.50  | *   | *     | *       |
| Coffee | 2025-02-01 | 2   | 7.25  | *   | *     | *       |
| Mugs   | 2025-02-03 | 6   | 9     | *   | *     | *       |

Net     := Qty * Price
Gross   := WithVat(Net)
Running := [Net; 0..*].sum()

sheet Summary

| Label    | Value                                    |
| -------- | ---------------------------------------- |
| Total    | = Orders.Gross.sum()                     |
| Dearest  | = Orders[; Net == Orders.Net.max()].Item |
| Big ones | = Orders[; Qty > 2].Item.count()         |
```

## The format

- `# comment` on a line of its own. The comment above an `fn` is its
  description.
- `const Name = expression` and `fn Name(a, b) = expression`, anywhere. A
  function sees its parameters, constants and sheets, not the row it was
  called from: pass the row's values in.
- `zone Europe/London` before the first sheet, if date-times need a zone.
- `sheet Name`, a blank line, optional `Column : Type` lines, then the sheet
  as a Markdown table: the header row, a `| --- |` line, and the data rows,
  each line between outer pipes, `| Jan | 100 |`. Line the pipes up; it is
  what makes the file readable and the diffs small. An empty cell is just
  spaces between pipes, `|     |`, a sheet of one column too.
- Older files leave out the outer pipes and the `---` line, `Jan | 100`; they
  still read. There a `|` at the start or end of a row is an empty cell.
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
- Bare text is Markdown where a file is shown, in the app and in a
  rendered document: `a **slow** start`, `[lease](lease.pdf)`. Quote text
  whose `*`, `_`, `[` or backticks are meant as they are: `"2 * 3 * 4"`.

## Types

`Int`, `Decimal`, `Fraction`, `Percent`, `Num`, `Text`, `Bool`, `Date`,
`Time`, `DateTime`.
A column with no type line takes its type from its cells, which is usually
what you want; declare one when the cells could be read another way.

| Type | Written |
|---|---|
| `Int` | `42`, `1_000_000` |
| `Decimal` | `19.99` — exact |
| `Fraction` | `1/7`, `22/7`, `2 3/4` — exact, shown as a fraction |
| `Percent` | `20%` — exact, shown as a percentage |
| `Num` | `1.5e3` — floating point, only when asked for; `Inf`, `-Inf`, `NaN` (as text, `"Inf"`), which is what a division by zero gives |
| `Bool` | `true`, `false` |
| `Date`, `Time`, `DateTime` | `2025-01-31`, `09:30`, `2025-01-31T09:30` — always ISO |

An `Int` is a `Decimal` and a `Decimal` is a `Fraction`. Sums and products of
decimals are `Decimal`; a division or an average is a `Fraction`, since it may
not end. So a column declared `Decimal` rejects `= Total / 3`: leave the
column undeclared, or write `(Total / 3).Decimal` if it is known to end.

A `Fraction` is shown as a fraction, never a decimal, and one of more than
one as a whole number and a part: `11/4` shows as `2 3/4`, and may be
written `2 3/4` in a cell. A `Fraction` column shows `2.75` that way too, so
declare money and measurements `Decimal`, not `Fraction`. Anything worked
out from a fraction is a fraction; `x.Decimal` gives the decimal.

A `Percent` is the number it stands for, so `Net * (1 + VatRate)` works as
written and gives a plain number. Only percentages added to percentages stay
a `Percent`. To show a share as a percentage, declare the column, as in
`Margin : Percent` over `Margin := Profit / Revenue`, or write `x.Percent`;
do not multiply by 100. It is shown rounded to two digits, `41.67%`.

Arithmetic is exact: `0.1 + 0.2 == 0.3` is true, `1/3 * 3` is `1`, and
integers do not overflow. An exact number is shown in full: a division
shows `0.125` if it ends as a decimal, `7 5/7` if it does not. A fraction in a result is correct,
not a fault: do not "fix" it with `.Num`, and use `round(x)` only when the
user wants a rounded number. A number is shown in at most 23 characters:
from `1e15` up, or below `1e-15`, it is in e notation, `1e+60`, and digits
that do not fit are cut and marked `…`. Only the display is cut, and
`--exact` shows it in full.

## Expressions

| Written | Means |
|---|---|
| `Revenue` | in a sheet's own formula: this row's value |
| `Sales.Revenue` | the whole column, a vector |
| `Sales[Revenue; 1]` | one cell. Rows count from 0; `-1` is the last |
| `Sales[; Revenue > 100]` | the rows where it is true, every column |
| `Sales[Revenue; *-1]` | the row before this one; `*` is this row |
| `[Revenue; 0..*]` | from the top to this row; the sheet's name can be dropped inside it |
| `a // b` | `b` when `a` is empty: `[Revenue; *-1] // 0` |

- Selection is `[columns; rows]`, with `;` between them. One column only: for
  several, pipe through `select`:
  `Sales |> filter(Profit > 0) |> select(Month, Profit)`.
- Ranges: `0..2` includes both ends, `0..^2` leaves out the last, `^2` is
  the first two.
- Equality is `==`. A single `=` only starts a formula cell or binds a
  `const`.
- `and`, `or`, `not`, `in`, and `if c then a else b`. There is no `IF()`.
- A computed column may be a sequence, `terms ... limit`: `1, 3 ... Inf`
  and `1, 2, 4 ... Inf` take their step from the terms, as a difference or
  as a ratio (two terms are read as a difference, so a ratio needs three);
  `0, 1, $-1 + $-2 ... Inf` gives its own rule, where `$-1` is the term
  before this one. `Inf` fills the column, and any other limit stops there
  and leaves the rest empty. The days of the week and the months of the
  year are known by name, in full or in three letters, and go round:
  `"Monday", "Tuesday" ... Inf`, `"Jan", "Feb" ... Inf`, and
  `"Jan-2026", "Apr-2026" ... Inf` for quarters. Any other text counts up
  as letters and digits do, `"A", "B" ... Inf` and `"AY" ... "BA"`. Dates
  step by whole days and times by seconds, except that the same day of
  each month steps by months: `2025-01-01, 2025-02-01 ... Inf`, and
  `2025-01-31, 2025-02-28 ... Inf` for the end of each month.
- Functions are called `sum(x)`, `x.sum()` or `x |> sum()`:
  `sum avg min max count`, `abs round floor ceil sqrt` and the usual maths,
  `today() now() year month day weekday`, `succ` for the next value, and
  the type names as conversions,
  `x.Int`, `x.Num`, `x.Percent`, `x.Text`.
- A date plus a whole number is days later; two dates subtract to days.
  `n.day`, `n.week`, `n.month` and `n.year` next to a date say which unit:
  `Placed + 1.month`, which keeps the day of the month or clamps it to the
  last day of a short one. `eom(x)` is the last day of a date's month.
- A lookup that finds one row fills a cell with its value; none gives empty,
  so add `// "none"`; several is an error.

## Workbooks, CSV, JSON and Markdown

```sh
omasheet import book.xlsx -o book.omx    # a sheet for each worksheet
omasheet import sales.csv                # one sheet, printed
omasheet export file.omx --xlsx          # file.xlsx
omasheet export file.omx --csv --sheet Orders -o orders.csv
omasheet import orders.json              # a list of row objects, or {"Sheet": [rows]}
omasheet export file.omx --json          # file.json: {"Sheet": [{"Col": value}]}
omasheet render report.md --html -o report.html
```

- The `.omx` file is the original; the others are ways in and out. An `.xlsx`
  export writes OMX formulas as workbook formulas where a workbook can say
  them, each with its value; the rest go out as values, and a `note:` says
  which columns and why. CSV and JSON hold values only. Import and export
  are a helper, not a full conversion: say so if the user expects otherwise.
- An `.xlsx` import translates the workbook's formulas to OMX where it can,
  and a column filled down with one formula becomes a computed column. Each
  is checked against the value the workbook held; one that cannot be
  translated, or does not agree, comes in as its value, and a `note:` says
  which cells and why. CSV has no formulas.
- Import and export print `note:` lines for what was renamed, rounded or left
  behind. Pass them on to the user; they are the lossy parts.
- After a CSV import the file is plain data. Turning repeated arithmetic
  back into computed columns is worth offering, and is done by hand; so is
  rewriting a workbook formula that was left as a value.
- In Markdown, `files: [budget.omx]` in the front matter names files, a
  fenced `omx` block is a file shown as its sheets, and
  `{{ Sales.Revenue.sum() }}` in the text is replaced by its value.
  `omasheet render` reports mistakes at their line in the document.

## In the omasheet repository

- `examples/*.omx` each have an `.expected` beside them, compared by the
  tests. After changing an example, regenerate it with
  `omasheet print examples/name.omx > examples/name.expected` and read the
  diff; do not edit the `.expected` by hand.
- `omasheet` with no command, or with only a file, opens the desktop app:
  never run it that way to read a file. It opens a real
  window on the user's desktop and building it loads every core, so say so
  before doing either.
