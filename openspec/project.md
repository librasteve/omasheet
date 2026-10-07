# Omasheet — Project Context

## Purpose

Omasheet is a text-native spreadsheet for the Omarchy platform: what Markdown is
to a word processor, Omasheet is to a spreadsheet. A workbook is a plain-text
`.sheet` file that is human-readable, diff-friendly, Git-friendly and easy for
both people and LLMs to write.

It is deliberately *not* "Excel in text". Authored files have no A1 cell
coordinates. A sheet is a set of named, column-typed tables, and formulas are
written in **OMX** (Omasheet Expressions), a small Raku-inspired array language
with exact arithmetic, units and uncertainty, compiled to a Rust engine.

> Exact by default; approximate by explicit choice.

Origin: `coinvent-omasheet-format-chatgpt.md` (design conversation, saved
2026-10-07). Working title in that conversation was "Sheetdown"; the product
name is **Omasheet** and the expression language is **OMX** (called "SDX" in
that conversation).

## Design Principles

1. **Tables, not grids.** Named tables and named columns; A1 notation exists
   only at the XLSX import/export boundary.
2. **One indexing model.** `[]` selects, `;` separates dimensions, `..` builds
   ranges, `*` is the cursor. Scalars, vectors, tables and N-d arrays all use it.
3. **Exact numerics.** `Int` is arbitrary precision, `Rat` is an
   arbitrary-precision rational. Floating point (`Num`) only on request.
4. **Meaning lives in types.** Units, currencies, percentages and uncertainty
   are part of a value's type, declared per column, checked before execution.
5. **Value is not format.** Display (currency symbol, decimal places, display
   unit) never changes the stored value.
6. **OMX stands alone.** The expression language is usable outside a `.sheet`
   file (`omasheet eval`, REPL).
7. **Markdown is presentation, Omasheet is computation.** They are peers.
8. **Expressive syntax, boring runtime.** OMX is compiled (never
   string-evaluated) into ordinary Rust data structures.

## Tech Stack

- Language: Rust, shipped as a single static binary `omasheet`
- Parsing: `chumsky` (candidate; `winnow` acceptable)
- Numerics: `num-bigint` / `num-rational` (`BigInt`, `BigRational`)
- Dataframe operations (group, join, pivot, sort, window): Polars — see
  `design.md` open question on exact numerics
- XLSX: `calamine` (read), `rust_xlsxwriter` / `polars_excel_writer` (write)
- CLI: `clap`
- Later: `tower-lsp` (LSP), `ratatui` (viewer), Neovim plugin

## Workspace Layout (planned)

```
omasheet/
├── omasheet-omx      lexer, parser, AST, type checker, dependency analysis
├── omasheet-engine   values, numerics, units, uncertainty, execution, recalc
├── omasheet-xlsx     XLSX import/export
├── omasheet-cli      the `omasheet` binary
├── omasheet-ui       viewer (later)
└── omasheet-lsp      language server (later)
```

## Conventions

- Native file extension: `.sheet`
- Fenced code block language tag in Markdown: `sheet`
- Row and column positions are 0-based
- Identifiers for tables and columns are case-sensitive, conventionally
  `PascalCase`

## Glossary

| Term | Meaning |
|------|---------|
| Sheet | A `.sheet` file: declarations plus one or more tables |
| Table | Named, ordered collection of rows with a typed column schema |
| Computed column | A column defined once by an OMX expression (`Name := expr`) |
| OMX | Omasheet Expressions — the formula language |
| Cursor (`*`) | The current row (or current position in a dimension) while an expression is evaluated in row context |
| Quantity | A numeric magnitude with a unit |
| Uncertain | A quantity with an attached measurement uncertainty (`±`) |
