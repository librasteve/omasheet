# Omasheet — Project Context

## Purpose

Omasheet is a text-native spreadsheet for the Omarchy platform: what Markdown is
to a word processor, Omasheet is to a spreadsheet. A workbook is a plain-text
`.omx` file that is human-readable, diff-friendly, Git-friendly and easy for
both people and LLMs to write.

It is deliberately *not* "Excel in text". Authored files have no A1 cell
coordinates. A sheet is a set of named, column-typed tables, and formulas are
written in **OMX** (Omasheet Expressions), a small Raku-inspired array language
with exact arithmetic, compiled to a Rust engine. Units and uncertainty are part
of the design and arrive in later phases (see Roadmap).

> Exact by default; approximate by explicit choice.

Origin: `coinvent-omasheet-format-chatgpt.md` (design conversation, saved
2026-10-07). Working title in that conversation was "Sheetdown"; the product
name is **Omasheet** and the expression language is **OMX** (called "SDX" in
that conversation).

## Design Principles

1. **Tables, not grids.** Named tables and named columns; A1 notation exists
   only at the XLSX import/export boundary.
2. **One indexing model.** `[]` selects, `;` separates dimensions, `..` builds
   ranges, `*` is the cursor, an empty slot is "all". Scalars, vectors, tables
   and N-d arrays all use it.
3. **Exact numerics.** `Int` is arbitrary precision, `Rational` is an
   arbitrary-precision rational. Floating point (`Num`) only on request.
4. **Meaning lives in types.** Units, currencies, percentages and uncertainty
   are part of a value's type, declared per column, checked before execution.
   (Base types in Phase 1; units in Phase 4; uncertainty in Phase 5.)
5. **Value is not format.** Display (currency symbol, decimal places, display
   unit) never changes the stored value.
6. **OMX stands alone.** The expression language is usable outside an `.omx`
   file (`omasheet eval`, REPL).
7. **Markdown is presentation, Omasheet is computation.** They are peers.
8. **Expressive syntax, boring runtime.** OMX is compiled (never
   string-evaluated) into ordinary Rust data structures.

## Roadmap

Delivery is phased. Each phase is one OpenSpec change under `changes/`, built,
verified and archived before the next begins.

| Phase | Change | Delivers | Done when |
|-------|--------|----------|-----------|
| 1 | `add-omasheet-core` | Numerics, OMX, the `.omx` format, compile / check / evaluate, `omasheet eval`, `lint` and the read-only view | An `.omx` with computed columns, cursor references, filters and lookups evaluates exactly from the CLI, with no dataframe dependency |
| 2 | `add-table-operations` | Group, join, the `group` pipe stage, partitioned cursor offsets, incremental recalculation | `Sales \|> group(Region) \|> sum(Revenue)` and a left join give exact results, on Omasheet's own engine |
| 3 | `add-interop` | XLSX import and export; Markdown `{{ }}` and `omx` blocks; `import`, `export`, `render` | A workbook imports to `.omx` and exports back; `omasheet render report.md` produces HTML |
| 4 | `add-units` | Unit literals, dimension checking, currencies, column units, user-defined units, display units | `Revenue + Weight` is rejected by `lint`; unit columns export to XLSX |
| 5 | `add-uncertainty` | `±` values, propagation, `Uncertain<…>` columns, display | `10 ± 0.1 m` propagates through arithmetic and aggregation |
| 6 | `add-matrix-multiplication` | Matrix values and literals, tables as matrices, the matrix product `@`, `transpose`, `inverse`, `det` | `[[1, 2], [3, 4]] @ [[5, 6], [7, 8]]` gives `[[19, 22], [43, 50]]` and `M @ M.inverse()` the identity, both exactly; a mismatch of shapes is rejected by `lint` |

Alongside the phases, `add-interactive-app` adds the desktop window: an editable
grid over the same engine. It needs only Phase 1 and gains each later phase's
features as they land.

Dependencies: every phase needs Phase 1. Phase 3 does not need Phase 2. Phase 4
adds deltas to Phases 2 and 3 (units through group/join, units on export), and
Phase 5 builds on Phase 4. Phase 6 needs only Phase 1; units and uncertainty
through a matrix product are later deltas to Phases 4 and 5.

Not yet in any change: REPL, LSP server, Neovim plugin, TUI viewer, charts,
pivots, formatting syntax.

## Tech Stack

- Language: Rust, shipped as a single static binary `omasheet`
- Parsing: hand-written lexer and recursive-descent parser (no parser library)
- Numerics: `num-bigint` / `num-rational` (`BigInt`, `BigRational`)
- Table operations (group, join, sort): implemented natively in
  `omasheet-engine` from Phase 2. Polars and other dataframe libraries are out
  of scope — none has an arbitrary-precision rational type
- XLSX (Phase 3): `calamine` (read), `rust_xlsxwriter` (write)
- CLI: `clap`
- App: Qt 6 Quick (QML) with `cxx-qt`; needs `qt6-base` and `qt6-declarative`
- Later: `tower-lsp` (LSP), Neovim plugin

## Workspace Layout (planned)

```
omasheet/
├── omasheet-omx      lexer, parser, AST, type checker, dependency analysis
├── omasheet-engine   values, numerics, execution (Phase 1); table operations,
│                     recalc (2); units (4); uncertainty (5); matrices (6)
├── omasheet-xlsx     XLSX import/export (Phase 3)
├── omasheet-cli      the `omasheet` binary
├── omasheet-ui       the interactive window: Qt Quick, driven from Rust (cxx-qt)
└── omasheet-lsp      language server (later)
```

## Conventions

- Native file extension: `.omx` — the same letters as OMX, the expression
  language; an `.omx` file is a whole sheet (tables, schemas, constants), not a
  single expression
- Fenced code block language tag in Markdown: `omx`
- Row and column positions are 0-based
- Identifiers for tables and columns are case-sensitive, conventionally
  `PascalCase`

## Glossary

| Term | Meaning |
|------|---------|
| Sheet | An `.omx` file: declarations plus one or more tables |
| Table | Named, ordered collection of rows with a typed column schema |
| Computed column | A column defined once by an OMX expression (`Name := expr`) |
| OMX | Omasheet Expressions — the formula language |
| Cursor (`*`) | The current row while an expression is evaluated in row context |
| Matrix | A rectangle of numbers addressed by position, without column names (Phase 6) |
| Quantity | A numeric magnitude with a unit (Phase 4) |
| Uncertain | A quantity with an attached measurement uncertainty (`±`) (Phase 5) |
