# Design: XLSX and Markdown interop (Phase 3)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. Builds on `add-omasheet-core`;
D/P/Q numbers continue that change's numbering.

## Decisions

| # | Status | Decision | Notes |
|---|--------|----------|-------|
| D10 | Decided | XLSX via existing crates | calamine (read), rust_xlsxwriter (write) |
| P14 | Provisional | Markdown model: `.md` = prose, `.omx` = computation, linked by `{{ expr }}` | Alternatives seen: literate `.omx` with prose; full notebook |

## Architecture

XLSX sits at the edge of the pipeline: import produces `.omx` source, which is
then compiled like any other; export reads evaluated tables. Neither touches the
internal representation.

## Open Questions

- The syntax for linking an `.omx` file from Markdown is not settled
  (`!sheet Sales.omx` and `{{ include("Sales.omx") }}` were both floated).

## Risks

- **Lossy XLSX round trip.** Exporting loses exactness (and later units and
  uncertainty); importing cannot recover table structure from a free-form grid.
  Users must not treat XLSX as a save format.
