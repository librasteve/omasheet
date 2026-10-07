# Proposal: Omasheet core (Phase 1)

## Why

Spreadsheets have no plain-text source format. XLSX is opaque to editors, diffs,
Git and LLMs, and its formula language is built around grid coordinates
(`$B$2:B2`), which are fragile under edits and hide intent. Its numeric model is
IEEE floating point (`0.1 + 0.2 ≠ 0.3`).

Omarchy has small focused text-first apps (Omawrite, Omacalc) but nothing for
tabular calculation. Omasheet fills that gap.

This is Phase 1 of the roadmap in `project.md`: the smallest useful tool — write
an `.omx` file in a text editor, check it and evaluate it exactly from the
command line. It has no dataframe, XLSX or Markdown dependency.

## What Changes

Nothing exists yet, so every requirement is ADDED.

- **sheet-format** — the `.omx` plain-text file format: tables, column
  schemas over the base types, computed columns, constants
- **omx-expressions** — the OMX formula language: references, `[ ; ]` indexing,
  ranges, slices, the `*` cursor, filtering, aggregation, conditionals, pipes
  with `filter` and `select`
- **numerics** — `Int` / `Rat` / `Num` / `Complex` tower, exact by default
- **evaluation** — compile pipeline, static checking of types and shapes before
  execution, dependency-ordered calculation, cycle detection
- **cli** — the `omasheet` command: read-only view, `eval`, `lint`, diagnostics

## Impact

- New Rust workspace (`omasheet-omx`, `omasheet-engine`, `omasheet-cli`); see
  `project.md`
- New file type `.omx`
- No existing code or users affected

## Out of Scope (for this change)

Delivered by later changes, in roadmap order:

- `add-table-operations` (Phase 2) — group, join, the `group` pipe stage,
  partitioned cursor offsets, incremental recalculation
- `add-interop` (Phase 3) — XLSX import/export and Markdown integration
- `add-units` (Phase 4) — units, currencies, dimensional analysis
- `add-uncertainty` (Phase 5) — `±` values and propagation

Discussed in the source conversation and not yet in any change:

- Charts and pivot-table declarations
- Sheet directives from the early brainstorm (`@format`, `@validate`, `@freeze`,
  `@filter`) — superseded in part by column types; formatting syntax is an open
  question
- A1 / `$A$1` references in authored files (import/export boundary only)
- Probabilistic values (`Normal(...)`, `Uniform(...)`)
- Interactive editing UI, TUI, LSP server, Neovim plugin
- Markdown tables as OMX data sources (`import "report.md"`)
- Polars, SQLite, DuckDB or DataFusion backends

## Status of Decisions

The source is a brainstorm, so requirements fall into three classes. `design.md`
records which is which:

- **Decided** — stated or confirmed by the project owner
- **Provisional** — proposed in the conversation and not objected to, or chosen
  here to make the spec coherent
- **Open** — unresolved or internally contradictory; listed as open questions
  and kept out of normative requirements where possible
