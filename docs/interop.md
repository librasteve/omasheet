# Workbooks, CSV and Markdown

The `.omx` file is the sheet. A workbook or a CSV file is a way in or out,
and each command says what it could not carry across.

## Import

```sh
omasheet import budget.xlsx -o budget.omx    # a table for each worksheet
omasheet import sales.csv                    # one table, named after the file
```

- The first row is the column names. A name that is not an identifier is
  made one: `Unit Price` becomes `UnitPrice`.
- A number is read exactly: `19.99` is `1999/100`. A workbook holds every
  number as floating point, so it is read from the shortest decimal that
  gives the same number; CSV is read as it is written, every digit of it.
- A column of dates, times, date-times or `true` / `false` is given its
  type. A column of numbers and text together has none: each cell is what
  it reads as.
- A formula in a workbook is not translated: its value is imported, and the
  cells are listed.
- Text that looks like a formula, `=SUM(A1:A3)`, stays text.

## Export

```sh
omasheet export budget.omx --xlsx            # budget.xlsx, a worksheet for each table
omasheet export budget.omx --csv             # budget.Sales.csv, budget.Summary.csv
omasheet export budget.omx --csv --table Sales -o sales.csv
```

What is written is the calculated values: no formulas, and no constants.

| | XLSX | CSV |
|---|---|---|
| `Int`, `Ratio` | the nearest floating-point number | in full; one that does not end, such as `1/3`, as `0.3333333333333333` |
| `Date`, `Time`, `DateTime` | a date or time cell | `2025-01-31`, `09:30`, `2025-01-31T09:30` |
| `Complex` | text | `3+4i` |
| a cell that failed | empty | empty |

Anything written less exactly than the sheet holds it is reported, with the
columns it happened in.

## In the app

- Ctrl+O opens a workbook or a CSV file as well as a sheet. It is imported
  as a new sheet, not yet saved.
- Ctrl+E exports, and asks what as: every table as a workbook (X), or the
  table shown as CSV (C). It then asks where.
- What was changed, rounded or left behind is listed, and stays under
  `notes` in the footer.

## Markdown

```sh
omasheet render report.md                    # Markdown, with the values in place
omasheet render report.md --html -o report.html
```

A Markdown document can quote a sheet:

````markdown
---
sheets: [budget.omx]
---

Revenue was {{ Sales.Revenue.sum() }}, best in {{ Sales[; Profit == Sales.Profit.max()].Month }}.

```omx
table Targets

Month | Goal | Over
Jan   | 4200 | *

Over := Sales[Profit; *] - Goal
```
````

- `sheets:` in the front matter names `.omx` files, from where the document
  is. `sheets: budget.omx` and a list of `- budget.omx` lines work too.
- A fenced `omx` block is a sheet, shown as its tables. The files and all
  the blocks of a document are one sheet, so each can read the others.
- `{{ expression }}` is replaced by its value. Several values read as a
  list, `Feb, Mar`; a table is shown as a table.
- `{{` in code is left alone, and so is `\{{`.
- A mistake is reported at its line in the document, or in the sheet it is
  in, and nothing is written.

A tool that does not know Omasheet shows the block as code and the braces
as they are. [`examples/report.md`](../examples/report.md) is a worked
document, with what it renders as beside it.

---

Copyright (c) 2026 Stephen Roe. MIT licence.
