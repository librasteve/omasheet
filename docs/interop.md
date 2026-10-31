# XLSX Workbooks, CSV, JSON, Parquet and Markdown

The `.omx` file is the original. An XLSX Workbook, a CSV file, JSON or Parquet
is a way in or out, and each command says what it could not carry across.

Import and export are a helper, not a full conversion. A file and an XLSX
Workbook are built differently: named, typed sheets and exact numbers on one
side, a grid of cells and floating point on the other. Those differences
keep either direction from covering everything, so check what the command
reports.

## Import

```sh
omasheet import budget.xlsx -o budget.omx    # a sheet for each worksheet
omasheet import sales.csv                    # one sheet, named after the file
omasheet import orders.json                  # a sheet for each list of rows
```

- The first row is the column names. A name that is not an identifier is
  made one: `Unit Price` becomes `UnitPrice`.
- A number is read exactly: `19.99` is `1999/100`. An XLSX Workbook holds
  every number as floating point, so it is read from the shortest decimal
  that gives the same number; CSV is read as it is written, every digit of it.
- A column of dates, times, date-times, `true` / `false` or floating-point
  numbers is declared, because the written cells alone would not give that
  type back: a `Float` column of `22` and `7.25` would read as an `Integer` and
  a `Decimal` without the line. A column of exact numbers, or of text, is
  left undeclared, as its cells read as that type anyway, and the app shows
  such a type muted. A column of numbers and text together has none either:
  each cell is what it reads as. See
  [Types](format.md#types).
- A formula in an XLSX Workbook is translated to OMX where OMX can say it:
  see [Formulas](#formulas). Where it cannot, the value is imported, and
  the cells are listed with the reason.
- Text that looks like a formula, `=SUM(A1:A3)`, stays text.
- Text is imported as it is, not as Markdown: `*new*` is written in quotes,
  `"*new*"`, so its asterisks are kept.

### JSON

JSON is read as values: a list of rows is a sheet named after the file, and
an object of such lists is a sheet for each, named by its key.

```json
{
  "Sales": [
    {"Month": "Jan", "Revenue": 10000, "Day": "2025-01-31"},
    {"Month": "Feb", "Revenue": 12000, "Day": "2025-02-28"}
  ]
}
```

- A row is an object of values by column name. The columns are in the order
  they are first met, and one that a row does not have is empty there. A
  row may be a list of values instead, with the names as the first row.
- A number is read as it is written, every digit of it: `19.99` is a
  `Decimal`, and `1.5e3` a `Float`. `true` and `false` are `Boolean`, and `null`
  is an empty cell.
- JSON has no dates, so a string that is a date, a time or a date-time is
  one. So is `"Inf"`, `"-Inf"` or `"NaN"`. Any other string is text, a
  number in quotes too: `"42"`.
- A list or an object inside a row is kept as text, and reported.

### Formulas

A worksheet becomes a sheet whose first row is the header, so a cell of a
formula becomes a column and a row:

| In the XLSX Workbook, on row 3 | In the file |
|---|---|
| `=B3-C3` | `= Revenue - Cost` |
| `=B2` | `= [Revenue; *-1]`, the row above |
| `=B$2` | `= [Revenue; 0]`, the first row whichever row asks |
| `=SUM(B2:B10)`, all the rows | `= Sales.Revenue.sum()` |
| `=SUM(B$2:B3)` | `= [Revenue; 0..*].sum()`, a running total |
| `=Rates!B2*B3` | `= Rates[Rate; 0] * Revenue` |
| `=IF(B3>C3,"up","down")` | `= if Revenue > Cost then "up" else "down"` |

- A column filled down with one formula becomes a computed column, with `*`
  in its cells and `Profit := Revenue - Cost` after the rows.
- These are translated: `+ - * / ^ %` and the comparisons; `SUM`, `AVERAGE`,
  `MIN`, `MAX`, `COUNT`, `COUNTA`; `IF`, `IFS`, `SWITCH`, `AND`, `OR`, `NOT`;
  `ROUND`, `INT`, `TRUNC`, `ABS`, `SIGN`, `SQRT`, `POWER`, `EXP`, `LN`,
  `LOG10`, `PI` and the trigonometry; `TODAY`, `NOW`, `YEAR`, `MONTH`, `DAY`,
  `HOUR`, `MINUTE`, `SECOND`, `WEEKDAY`, `DATEVALUE`.
- These are not, and the value is imported: joining text with `&`, lookups
  such as `VLOOKUP`, `DATE(y, m, d)`, names defined in the XLSX Workbook,
  lists in `{ }`, and a cell in the header row or outside the sheet.
- Every translation is checked. The file is worked out, and a formula that
  does not give the value the XLSX Workbook holds for its cell is dropped
  for that value. The commonest cause is an empty cell: an XLSX Workbook
  adds it up as nothing, and in OMX what is worked out from an empty cell
  is empty.
  `TODAY()` and `NOW()` are not checked, as they give another answer each
  day.
- An XLSX Workbook counts the days of the week from Sunday and OMX from
  Monday, so `WEEKDAY(A2)=1` becomes `weekday(Date) == 7`.
- A `SWITCH` or an `IFS` with nothing for when no case fits takes its last
  case for that.

## Export

```sh
omasheet export budget.omx --xlsx            # budget.xlsx, a worksheet for each sheet
omasheet export budget.omx --csv             # budget.Sales.csv, budget.Summary.csv
omasheet export budget.omx --csv --sheet Sales -o sales.csv
omasheet export budget.omx --json            # budget.json, every sheet
omasheet export budget.omx --json --sheet Sales   # the rows of Sales alone
```

An XLSX Workbook is written with the formulas of the file, and each with its
calculated value, so it reads the same before anything works it out again.
CSV and JSON hold the calculated values alone. Constants are not exported: a
formula that reads one is written with what the constant is.

A sheet becomes a worksheet whose first row is the header, so a column and a
row become a cell. Each formula is written for the row it is on:

| In the file, on row 1 of `Sales` | In the XLSX Workbook |
|---|---|
| `Revenue - Cost` | `=B3-C3` |
| `[Revenue; *-1]`, the row above | `=B2` |
| `[Revenue; 0]`, the first row | `=$B$2` |
| `Sales.Revenue.sum()`, from another sheet | `=SUM('Sales'!$B$2:$B$13)` |
| `sum([Revenue; 0..*])`, a running total | `=SUM($B$2:B3)` |
| `[Revenue; *-1] // 0` | `=IF(ISBLANK(B2),0,B2)`, and `0` on the first row |
| `Rates[Fee; Code == Sales[Code; *]] // 1` | `=IFERROR(INDEX('Rates'!$B$2:$B$9,MATCH(A3,'Rates'!$A$2:$A$9,0)),1)` |
| `count(Orders[; Region == Name])` | `=COUNTIFS('Orders'!$F$2:$F$40,A3)` |
| `if Revenue > Cost then "up" else "down"` | `=IF(B3>C3,"up","down")` |

- These are written: `+ - * / **` and the comparisons; `and`, `or`, `not`,
  `if`; `sum`, `mean`, `min`, `max`, `count` and `len` of a column, a range
  of rows or a list; `sum`, `mean` and `len` of the rows where columns equal
  values; a row found where one column equals a value; `abs`, `round`,
  `floor`, `ceil`, `sqrt`, the logarithms and the trigonometry; `today`,
  `now`, `year` to `second`, `weekday`, `date` and `time`; `x.Integer`, and a
  function the file defines.
- These are not, and the value is written: a calculation on a whole column
  at once, `Sales.Revenue * 2`; rows picked by any other condition, by a
  list or by a range value; `in` and ranges as values; `//` after a
  calculation; text conversions; time zones; complex numbers. Each is
  reported with the columns it happened in.
- A cell that comes to nothing, or that failed, is left empty, with no
  formula.
- An XLSX Workbook counts a time in days and a file in seconds, so
  `At - 09:00` is written `=ROUND((C3-TIME(9,0,0))*86400,0)`.
- A sheet exported alone with `--sheet` cannot read another: those formulas
  are written as values.
- A formula is not checked in an XLSX Workbook, as nothing here works one
  out. It is exact in the file and floating point there, so the last digits
  may differ.
- Importing the XLSX Workbook again brings back most formulas, but not all:
  the import has no translation for `ISBLANK` or `IFERROR`, and takes those
  cells as values.

| | XLSX | CSV |
|---|---|---|
| `Integer`, `Decimal` | the nearest floating-point number | in full, to the 30 digits it shows; one that does not end, such as `1/3`, as `0.333333333333333333333333333333` |
| `Fraction` | the nearest floating-point number | the decimal it holds, not `1/7`: a CSV is read by anything, and a fraction is not a number elsewhere |
| `Percent` | that number, formatted as a percentage | `40%`; one that does not end as `33.3333333333333333333333333333%` |
| `Date`, `Time`, `Datetime` | a date or time cell | `2025-01-31`, `09:30`, `2025-01-31T09:30` |
| a cell that failed | empty | empty |

JSON is written as it is read: an object holding each sheet by name as a
list of rows, a row to a line, and each row an object of values by column
name. With `--sheet` it is the rows of that sheet alone.

| | JSON |
|---|---|
| `Integer`, `Decimal`, `Fraction` | a number, in full; one that does not end, such as `1/3`, as `0.333333333333333333333333333333` |
| `Percent` | the number it stands for: `20%` is `0.2` |
| `Float` | a number; `Inf`, `-Inf` and `NaN` as strings |
| `String`, `Boolean` | a string; `true` or `false` |
| `Date`, `Time`, `Datetime` | a string, as a file writes it: `"2025-01-31"` |
| an empty cell, or one that failed | `null` |

Anything written less exactly than the file holds it is reported, with the
columns it happened in.

## In the app

- Ctrl+O opens an XLSX Workbook, a CSV file or JSON as well as an `.omx`
  file. It is imported as a new file, not yet saved.
- Ctrl+E exports, and asks what as: every sheet as an XLSX Workbook (X) or
  as JSON (J), or the sheet shown as CSV (C). It then asks where.
- What was changed, rounded or left behind is listed, and stays under
  `notes` in the footer.

## Parquet

Parquet carries column types where CSV carries only text, so it is the one
format that needs no guessing in either direction.

```sh
omasheet import sales.parquet            # one sheet, named after the file
omasheet export file.omx --parquet       # a file for each sheet
omasheet export file.omx --parquet --sheet Sales -o sales.parquet
omasheet export file.omx --parquet --meta  # with the formulas kept as well
```

An omasheet type *is* a Polars dtype, so a column is written as the Parquet
type its own name stands for rather than converted into something near it.

| | Parquet |
|---|---|
| `Integer` | `int64` |
| `Decimal`, `Percent` | `decimal(38, s)`, at the widest scale the column needs |
| `Float` | `double` |
| `Boolean` | `boolean` |
| `String` | UTF-8 `byte_array` |
| `Date` | `date32` |
| `Time` | `time64`, in nanoseconds |
| `Datetime` | `timestamp`, in microseconds |
| a blank cell | a null |
| a cell that failed | a null, and it is reported |

On the way in, each column's type is taken from the file's schema and never
guessed from its values, so a column of `"42"` and `"7"` held as text stays
`String`. The `.omx` written from it declares that type only where the cells
would not read back as it: a `double` column becomes `Age : Float`, while an
`int64` or a text column needs no line and shows muted in the app — muted
there is still the schema's type, not a guess. A column that is all nulls,
or whose values do not go together, has no line to hold it and reads as its
cells do. A column Parquet holds in a shape a sheet cannot render — a list,
a struct, a map — is left out and named in a notice.

A `Percent` is written as the decimal it holds: `20%` is `0.2`. Its display
is no part of its value, so it comes back as a `Decimal`.

### Keeping the formulas: `--meta`

A Parquet file holds values. With `--meta`, an export also keeps what no
format carries, in the Parquet file's key-value metadata under the key
`omasheet`: the file's `zone`, its constants and functions, and the
sheet's type lines and formulas. It is kept as `.omx` source, the sheet
with no rows:

```omx
const TaxRate = 20%

sheet Sales

Margin : Percent

| Month | Revenue | Cost | Profit | Tax | Margin |
| ----- | ------- | ---- | ------ | --- | ------ |

Profit := Revenue - Cost
Tax    := Profit * TaxRate
Margin := Profit / Revenue
```

Importing a file written this way gives that source back with the file's
rows put under its header: a computed column is a formula again, its cells
`*`, and the values the file holds for it are passed over. Its types are
the ones the sheet declared, a `Percent` included. A tool that knows
nothing of the metadata sees an ordinary Parquet file. A formula that
reads another sheet, which the Parquet file does not hold, is kept as it
is, and reports that sheet missing until it is there again. A table's or a
computed sheet's columns are kept as a sheet of their own rows.

### What a table reads

An import copies the rows into the `.omx`. A `table` leaves them where they
are — `table Sales := dataframe["sales.parquet"]`, written up in
[the format guide](format.md) — and then reads as little of the file as the
question needs.

Opening one reads the footer and nothing else: the schema, which is where
the columns and their types come from, and the row count, which is the
sheet's length. A file of a million rows and twenty columns costs that much
to open, whatever is asked of it afterwards.

Each column is then read on its own, the first time something asks for it,
and kept from then on. A column no formula names and no view shows is never
read at all: summing one column of the eight in `examples/titanic.parquet`
reads one of them. What is read stays a column — a formula over it is
calculated over the whole column at once wherever Polars has the same
answer as the engine, rather than a row at a time.

A column is read whole, though, so `head(5)` over a file of many row groups
reads every row group of the columns it shows. It is the rows that are not
narrowed, never the columns.

## Markdown

```sh
omasheet render report.md                    # Markdown, with the values in place
omasheet render report.md --html -o report.html
```

A Markdown document can quote a file:

````markdown
---
files: [budget.omx]
---

Revenue was {{ Sales.Revenue.sum() }}, best in {{ Sales[; Profit == Sales.Profit.max()].Month }}.

```omx
sheet Targets

| Month | Goal | Over |
| ----- | ---- | ---- |
| Jan   | 4200 | *    |

Over := Sales[Profit; *] - Goal
```
````

- `files:` in the front matter names `.omx` files, from where the document
  is. `files: budget.omx` and a list of `- budget.omx` lines work too.
- A fenced `omx` block is a file, shown as its sheets. The files and all
  the blocks of a document are one file, so each can read the others.
- `{{ expression }}` is replaced by its value. Several values read as a
  list, `Feb, Mar`; a sheet is shown as a Markdown table.
- `{{` in code is left alone, and so is `\{{`.
- Text written bare in a cell may hold Markdown, and is read as that in the
  document: `**bold**`, `[a link](there.md)`. Text in quotes, or made by a
  formula, is shown as it is. HTML in a cell is shown as text.
- A mistake is reported at its line in the document, or in the file it is
  in, and nothing is written.

A tool that does not know Omasheet shows the block as code and the braces
as they are. [`examples/report.md`](../examples/report.md) is a worked
document, with what it renders as beside it.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
