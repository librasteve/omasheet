# Workbooks, CSV, JSON and Markdown

The `.omx` file is the original. A workbook, a CSV file or JSON is a way in or out,
and each command says what it could not carry across.

Import and export are a helper, not a full conversion. A file and a
workbook are built differently: named, typed sheets and exact numbers on one
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
- A number is read exactly: `19.99` is `1999/100`. A workbook holds every
  number as floating point, so it is read from the shortest decimal that
  gives the same number; CSV is read as it is written, every digit of it.
- A column of dates, times, date-times or `true` / `false` is given its
  type. A column of numbers and text together has none: each cell is what
  it reads as.
- A formula in a workbook is translated to OMX where OMX can say it: see
  [Formulas](#formulas). Where it cannot, the value is imported, and the
  cells are listed with the reason.
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
  `Decimal`, and `1.5e3` a `Num`. `true` and `false` are `Bool`, and `null`
  is an empty cell.
- JSON has no dates, so a string that is a date, a time or a date-time is
  one. So is `"Inf"`, `"-Inf"` or `"NaN"`. Any other string is text, a
  number in quotes too: `"42"`.
- A list or an object inside a row is kept as text, and reported.

### Formulas

A worksheet becomes a sheet whose first row is the header, so a cell of a
formula becomes a column and a row:

| In the workbook, on row 3 | In the file |
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
  such as `VLOOKUP`, `DATE(y, m, d)`, names defined in the workbook, lists
  in `{ }`, and a cell in the header row or outside the sheet.
- Every translation is checked. The file is worked out, and a formula that
  does not give the value the workbook holds for its cell is dropped for
  that value. The commonest cause is an empty cell: a workbook adds it up as
  nothing, and in OMX what is worked out from an empty cell is empty.
  `TODAY()` and `NOW()` are not checked, as they give another answer each
  day.
- A workbook counts the days of the week from Sunday and OMX from Monday, so
  `WEEKDAY(A2)=1` becomes `weekday(Date) == 7`.
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

A workbook is written with the formulas of the file, and each with its
calculated value, so it reads the same before anything works it out again.
CSV and JSON hold the calculated values alone. Constants are not exported: a
formula that reads one is written with what the constant is.

A sheet becomes a worksheet whose first row is the header, so a column and a
row become a cell. Each formula is written for the row it is on:

| In the file, on row 1 of `Sales` | In the workbook |
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
  `if`; `sum`, `avg`, `min`, `max` and `count` of a column, a range of rows
  or a list; `sum`, `avg` and `count` of the rows where columns equal
  values; a row found where one column equals a value; `abs`, `round`,
  `floor`, `ceil`, `sqrt`, the logarithms and the trigonometry; `today`,
  `now`, `year` to `second`, `weekday`, `date` and `time`; `x.Int`, and a
  function the file defines.
- These are not, and the value is written: a calculation on a whole column
  at once, `Sales.Revenue * 2`; rows picked by any other condition, by a
  list or by a range value; `in` and ranges as values; `//` after a
  calculation; text conversions; time zones; complex numbers. Each is
  reported with the columns it happened in.
- A cell that comes to nothing, or that failed, is left empty, with no
  formula.
- A workbook counts a time in days and a file in seconds, so `At - 09:00`
  is written `=ROUND((C3-TIME(9,0,0))*86400,0)`.
- A sheet exported alone with `--sheet` cannot read another: those formulas
  are written as values.
- A formula is not checked in a workbook, as nothing here works one out. It
  is exact in the file and floating point there, so the last digits may
  differ.
- Importing the workbook again brings back most formulas, but not all: the
  import has no translation for `ISBLANK` or `IFERROR`, and takes those
  cells as values.

| | XLSX | CSV |
|---|---|---|
| `Int`, `Decimal` | the nearest floating-point number | in full; one that does not end, such as `1/3`, as `0.3333333333333333` |
| `Percent` | that number, formatted as a percentage | `40%`; one that does not end as `33.33333333333333%` |
| `Date`, `Time`, `DateTime` | a date or time cell | `2025-01-31`, `09:30`, `2025-01-31T09:30` |
| a cell that failed | empty | empty |

JSON is written as it is read: an object holding each sheet by name as a
list of rows, a row to a line, and each row an object of values by column
name. With `--sheet` it is the rows of that sheet alone.

| | JSON |
|---|---|
| `Int`, `Decimal` | a number, in full; one that does not end, such as `1/3`, as `0.3333333333333333` |
| `Percent` | the number it stands for: `20%` is `0.2` |
| `Num` | a number; `Inf`, `-Inf` and `NaN` as strings |
| `Text`, `Bool` | a string; `true` or `false` |
| `Date`, `Time`, `DateTime` | a string, as a file writes it: `"2025-01-31"` |
| an empty cell, or one that failed | `null` |

Anything written less exactly than the file holds it is reported, with the
columns it happened in.

## In the app

- Ctrl+O opens a workbook, a CSV file or JSON as well as an `.omx` file. It is
  imported as a new file, not yet saved.
- Ctrl+E exports, and asks what as: every sheet as a workbook (X) or as
  JSON (J), or the sheet shown as CSV (C). It then asks where.
- What was changed, rounded or left behind is listed, and stays under
  `notes` in the footer.

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
