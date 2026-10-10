# Design: XLSX, CSV and Markdown interop (Phase 2)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. Builds on `add-omasheet-core`;
D/P/Q numbers continue that change's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D10 | Decided | XLSX via existing crates | calamine (read), rust_xlsxwriter (write) |
| P14 | Provisional | Markdown model: `.md` = prose, `.omx` = computation, linked by `{{ expr }}` | Alternatives seen: literate `.omx` with prose; full notebook |
| D24 | Decided | CSV import and export, one table per file, via the `csv` crate | Owner request, 2026-10-09. Fields are read and written as text, never through a double |
| D26 | Decided | A Markdown document names its sheets in YAML front matter: `sheets: [Sales.omx]` | Owner decision, 2026-10-09. Chosen over `!sheet Sales.omx` and `{{ include("Sales.omx") }}`: a tool that does not know Omasheet hides it, and it adds nothing to OMX |
| P27 | Provisional | Choices made while building the Markdown half: a `{{ }}` whose value is a table is shown as a table, and one whose value is several values as a list, `Feb, Mar`; a block with no table shows nothing; `\{{` is not an interpolation; a `zone` line in any of a document's sheets is the zone of them all | Not discussed in the source |
| D25 | Decided | Polars is not used in this phase. Parquet and any Polars integration are left to a later change, `add-dataframe-interop` | Owner decision, 2026-10-09. See "Why not Polars here". Consistent with D11 in `add-table-operations` |

## Architecture

XLSX and CSV sit at the edge of the pipeline: import produces `.omx` source,
which is then compiled like any other; export reads evaluated tables. Neither
touches the internal representation. Both are in the `omasheet-interop` crate.

Both imports reduce their file to the same thing — a named grid of cells, each
a number, text, boolean, date, time or empty — and one writer turns that into
`.omx` source: it makes names into identifiers, infers column types, and quotes
text that would otherwise read as something else. Column types may also be
given to the writer, so that a later format with a schema (Parquet) can reuse
it. Both exports walk the evaluated tables and report the lossy rules they
applied as notices, which do not fail the command.

## Why not Polars here

- **It does not read or write XLSX in Rust.** The Excel support people know is
  in the Python package, and is itself calamine and xlsxwriter underneath. The
  Rust crate would still need both.
- **Its CSV reader infers `f64` and `i64`.** `0.12345678901234567890123` and
  `2 ** 100` would be rounded or rejected before Omasheet saw them. Avoiding
  that means reading every column as a string and parsing it ourselves, at
  which point Polars is doing only what the `csv` crate does.
- **Its frames cannot hold the values.** There is no arbitrary-precision
  integer or rational column (D11), so export would convert every exact column
  to `f64` or to text first.
- **Its size.** Several hundred crates and tens of megabytes in a binary that
  is otherwise small and quick to build.

What dataframe users need from Omasheet is typed tables in a format their tools
read, which is Parquet. That is drafted as `add-dataframe-interop` and left out
of this phase: the Parquet crates alone take the `omasheet` binary from about
1.7 MB to about 8-10 MB (measured 2026-10-09 with a read/write probe).

A Markdown document, the `.omx` files its front matter names and its `omx`
blocks are one sheet. `omasheet-md` joins their text and compiles that, so the
checker and the engine see nothing new; it keeps where each part came from, and
puts every diagnostic back at its line in the document or the file. Rendering
is two steps: the document with its values and tables in place, which is still
Markdown and is the terminal output, and that as HTML.

## Open Questions

None.

## Risks

- **Lossy XLSX round trip.** Exporting loses exactness (and later units and
  uncertainty); importing cannot recover table structure from a free-form grid.
  Users must not treat XLSX as a save format.
- **CSV carries values only.** Formulas, computed columns, types and constants
  do not survive a CSV round trip, and CSV has no standard: files from other
  tools may use another delimiter, encoding or date form, and will then import
  as text.
