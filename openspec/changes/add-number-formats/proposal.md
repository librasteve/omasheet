# Proposal: Number formats

## Why

A number column is shown by the adaptive rules of the numerics spec: plain
from `1e-15` to `1e15`, e notation outside that, grouped from five digits,
cut and marked `…` past 23 characters. That suits a value of unknown size,
but a column of prices or counts wants a fixed shape: two places, a sign,
padded to a width. There is no way to say so.

## What Changes

- **file-format** — MODIFIED: a column of `Integer`, `Decimal` or `Float` may
  carry a format after its type in brackets, in the flags, width, precision
  and conversion of a C `printf` conversion without its `%`:
  `Price : Decimal['012.2f]`. A format that does not read is an error.
- **numerics** — ADDED: a column's format shows each of its numbers in place
  of the adaptive rules, with the point and the separator of digit groups the
  locale's
- **cli** / **markdown-integration** — MODIFIED: `print` and `{{ }}` show
  a number by its column's format
- **app** — MODIFIED: a format can be set from the app; the heading of a column with a format shows the format
  after its type: `Decimal['012.2f]`

## Impact

- `omasheet-omx`: the schema line reads the bracket; `SchemaLine` holds the
  format; a format is parsed and checked as the file is read
- `omasheet-engine`: `format_value` shows a number by its column's format
  where there is one; files round-trip the format
- `omasheet-ui`: column headings, and setting a format
- `omasheet-cli`, `omasheet-md`: rendering by the format
- A file with no formats reads and shows as before

## Out of Scope

- `Fraction` and `Percent` columns
- How a value is written in the file, and in the cell editor: always in full, as now
- XLSX, CSV and JSON exports
- A format for a single cell, or for a computed column's result type
- Formats beyond `printf`'s (Excel `#,##0.00`, currency symbols, colours)
