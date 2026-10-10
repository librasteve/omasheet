# Proposal: XLSX, CSV and Markdown interop (Phase 2)

## Why

A text-native spreadsheet is only adoptable if existing workbooks can be brought
in and results can be handed to people who use Excel, and it is most useful when
its results can be quoted in documents. XLSX is the compatibility format for
workbooks; CSV is the one every other tool reads and writes; Markdown is the
presentation format.

## What Changes

- **xlsx-interop** — ADDED: XLSX as a lossy compatibility format in both
  directions, with exact numbers mapped to the nearest double
- **csv-interop** — ADDED: CSV for one table of values in both directions, read
  and written as text so that terminating decimals and large integers stay exact
- **markdown-integration** — ADDED: `{{ expr }}` interpolation and `omx`
  blocks in Markdown, linking `.omx` files, HTML and terminal output
- **cli** — ADDED: `omasheet import`, `omasheet export --xlsx | --csv`,
  `omasheet render`

The XLSX mappings for units and uncertainty are added by `add-units` and
`add-uncertainty`, since those value kinds do not exist yet.

## Impact

- Depends on `add-omasheet-core`; independent of `add-table-operations`
- New crate `omasheet-md` (pulldown-cmark) for the Markdown half
- New crate `omasheet-interop` (calamine to read XLSX, rust_xlsxwriter to write
  it, `csv` for CSV). XLSX and CSV share the path from a grid of cells to `.omx`
  source: naming, column types, cell quoting and notices
- The Markdown half shares no code with the others and can be built in either
  order

## Out of Scope

- Translating Excel formulas to OMX on import (cached values are imported)
- Markdown tables as OMX data sources (`import "report.md"`)
- Other delimiters and encodings for CSV (TSV, `;`, Latin-1)
- Parquet and Polars integration (left to `add-dataframe-interop`, design D25)
- Charts
