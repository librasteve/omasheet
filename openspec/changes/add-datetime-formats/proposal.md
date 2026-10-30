# Proposal: Date and time formats

## Why

A `Date`, `Time` or `Datetime` is shown in the locale's form: its order of
day, month and year, its separator, and its clock. That suits a value read
at a glance, but a column of invoice dates wants `31 Jan 2025`, a log wants
`2025-01-31 09:30:15.123`, a rota wants `Fri 09:30`. There is no way to say
so. Number formats (`add-number-formats`) give a number column a C `printf`
conversion; this change gives a date or time column a C `strftime` format
the same way.

## What Changes

- **file-format** — MODIFIED: a column of `Date`, `Time` or `Datetime` may
  carry a format after its type in brackets, as a C `strftime` format:
  `Due : Date[%d %b %Y]`. A format that does not read is an error.
- **datetime** — ADDED: a column's format shows each of its values in place
  of the locale's form
- **cli** / **markdown-integration** — MODIFIED: `print` and `{{ }}` show
  a date or time by its column's format
- **app** — MODIFIED: `Set format…` takes date and time columns; the heading
  of a column with a format shows it after its type: `Date[%d %b %Y]`

## Impact

- `omasheet-omx`: the bracket on a schema line takes a `strftime` format
  for the three types; parsed and checked as the file is read
- `omasheet-engine`: `Engine::show_in` shows a date or time by its column's
  format; files round-trip it
- `omasheet-ui`: headings, and `Set format…` for the three types
- `omasheet-cli`, `omasheet-md`: rendering by the format
- A file with no formats reads and shows as before

## Out of Scope

- How a value is written in the file: always ISO, as now
- Reading typed text by the format (the cell editor reads the locale's form, as now)
- XLSX, CSV and JSON exports
- A format for a single cell, or for a computed column's result type
- Formats beyond `strftime`'s (Excel `dd-mmm-yy`, ICU `yyyy-MM-dd`)
