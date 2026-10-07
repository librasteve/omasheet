# Proposal: XLSX and Markdown interop (Phase 3)

## Why

A text-native spreadsheet is only adoptable if existing workbooks can be brought
in and results can be handed to people who use Excel, and it is most useful when
its results can be quoted in documents. XLSX is the compatibility format;
Markdown is the presentation format.

## What Changes

- **xlsx-interop** — ADDED: XLSX as a lossy compatibility format in both
  directions, with exact numbers mapped to the nearest double
- **markdown-integration** — ADDED: `{{ expr }}` interpolation and `omx`
  blocks in Markdown, linking `.omx` files, HTML and terminal output
- **cli** — ADDED: `omasheet import`, `omasheet export --xlsx`,
  `omasheet render`

The XLSX mappings for units and uncertainty are added by `add-units` and
`add-uncertainty`, since those value kinds do not exist yet.

## Impact

- Depends on `add-omasheet-core`; independent of `add-table-operations`
- New crate `omasheet-xlsx` (calamine to read, rust_xlsxwriter to write)
- The XLSX and Markdown halves share no code and can be built in either order

## Out of Scope

- Translating Excel formulas to OMX on import (cached values are imported)
- Markdown tables as OMX data sources (`import "report.md"`)
- Charts
