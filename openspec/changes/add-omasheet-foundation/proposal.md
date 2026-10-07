# Proposal: Omasheet foundation

## Why

Spreadsheets have no plain-text source format. XLSX is opaque to editors, diffs,
Git and LLMs, and its formula language is built around grid coordinates
(`$B$2:B2`), which are fragile under edits and hide intent. Its numeric model is
IEEE floating point (`0.1 + 0.2 ≠ 0.3`) and it has no notion of units, so
`Revenue + Weight` is legal.

Omarchy has small focused text-first apps (Omawrite, Omacalc) but nothing for
tabular calculation. Omasheet fills that gap.

## What Changes

This change establishes the whole product baseline. Nothing exists yet, so every
requirement is ADDED.

- **sheet-format** — the `.sheet` plain-text file format: tables, column
  schemas, computed columns, constants, unit declarations
- **omx-expressions** — the OMX formula language: references, `[ ; ]` indexing,
  ranges, slices, the `*` cursor, filtering, aggregation, conditionals, pipes
- **numerics** — `Int` / `Rat` / `Num` / `Complex` tower, exact by default
- **units** — unit literals, dimensional analysis, currencies and `%` as units,
  column-level units, user-defined units
- **uncertainty** — `±` values with automatic propagation
- **evaluation** — compile pipeline, static checking before execution,
  dependency-ordered calculation, table operations (group, join)
- **cli** — the `omasheet` command: view, eval, lint, import, export, render
- **xlsx-interop** — XLSX as a lossy compatibility format, both directions
- **markdown-integration** — `{{ expr }}` interpolation and `sheet` blocks in
  Markdown, rendered by `omasheet render`

## Impact

- New Rust workspace (`omasheet-omx`, `omasheet-engine`, `omasheet-xlsx`,
  `omasheet-cli`); see `project.md`
- New file type `.sheet`
- No existing code or users affected

## Out of Scope (for this change)

Discussed in the source conversation but deliberately deferred:

- Charts and pivot-table declarations
- Sheet directives from the early brainstorm (`@format`, `@validate`, `@freeze`,
  `@filter`) — superseded in part by column types; formatting syntax is an open
  question
- A1 / `$A$1` references in authored files (import/export boundary only)
- Probabilistic values (`Normal(...)`, `Uniform(...)`)
- Interactive editing UI, TUI, LSP server, Neovim plugin
- Markdown tables as OMX data sources (`import "report.md"`)
- SQLite / DuckDB / DataFusion backends (Polars was chosen instead)

## Status of Decisions

The source is a brainstorm, so requirements fall into three classes. `design.md`
records which is which:

- **Decided** — stated or confirmed by the project owner
- **Provisional** — proposed in the conversation and not objected to, or chosen
  here to make the spec coherent
- **Open** — unresolved or internally contradictory; listed as open questions
  and kept out of normative requirements where possible
