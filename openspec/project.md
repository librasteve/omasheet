# Omasheet — Project Context

## Purpose

Omasheet is a text-native spreadsheet for the Omarchy platform: what Markdown is
to a word processor, Omasheet is to a spreadsheet. A workbook is a plain-text
`.omx` file that is human-readable, diff-friendly, Git-friendly and easy for
both people and LLMs to write.

It is deliberately *not* "Excel in text". Authored files have no A1 cell
coordinates. A file is a set of named, column-typed sheets, and formulas are
written in **OMX** (Omasheet Expressions), a small Raku-inspired array language
with exact arithmetic, compiled to a Rust engine. Units and uncertainty are part
of the design and arrive in later phases (see Roadmap).

> Exact by default; approximate by explicit choice.

Origin: `coinvent-omasheet-format-chatgpt.md` (design conversation, saved
2026-10-07). Working title in that conversation was "Sheetdown"; the product
name is **Omasheet** and the expression language is **OMX** (called "SDX" in
that conversation).

## Design Principles

1. **Sheets, not grids.** Named sheets and named columns; A1 notation exists
   only at the XLSX import/export boundary.
2. **One indexing model.** `[]` selects, `;` separates dimensions, `..` builds
   ranges, `*` is the cursor, an empty slot is "all". Scalars, vectors, sheets
   and N-d arrays all use it.
3. **Exact numerics.** An omasheet type is a name for a Polars dtype, so
   `Int` is a 64-bit integer and `Decimal` a 38-digit fixed-point decimal,
   exact where the result fits and reported where it does not. Its scale is
   signed and wider than a Polars decimal's, so a number past 38 digits
   keeps them and moves the point, and such a column stays on the engine. Floating
   point (`Float`) only on request. (`add-polars-dtypes`, D14-D16;
   arbitrary-precision `Int` and the rational `Fraction` were the earlier
   reading.)
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

Delivery is phased. Each phase is one OpenSpec change under
`openspec/changes/`, built, verified and archived before the next begins.
Archived means moved to `changes/archive/`, with a note at the top of its
proposal saying whether it was built or superseded, and by what where it was
superseded. So `changes/` holds what is live — under way, or written and not
yet begun — and the archive holds the rest. Phases 1 and 2 are there, and
`add-sheet-operations` with them as the first change to be superseded rather
than built.

| Phase | Change | Delivers | Done when |
|-------|--------|----------|-----------|
| 1 | `add-omasheet-core` | Numerics, OMX, the `.omx` format, compile / check / evaluate, `omasheet eval`, `lint` and the read-only view | An `.omx` with computed columns, cursor references, filters and lookups evaluates exactly from the CLI, with no dataframe dependency |
| 2 | `add-interop` | XLSX and CSV import and export; Markdown `{{ }}` and `omx` blocks; `import`, `export`, `render` | A workbook or CSV file imports to `.omx` and exports back; `omasheet render report.md` produces HTML |
| 3 | `add-sheet-operations-polars` | Group, join, sort, the `group_by` pipe stage, partitioned cursor offsets — Polars' own, over the columnar core | `Sales \|> group_by(Region) \|> agg(Revenue.sum())` and a left join give exact results |
| 4 | `add-units` | Unit literals, dimension checking, currencies, column units, user-defined units, display units | `Revenue + Weight` is rejected by `lint`; unit columns export to XLSX |
| 5 | `add-uncertainty` | `±` values, propagation, `Uncertain<…>` columns, display | `10 ± 0.1 m` propagates through arithmetic and aggregation |

Alongside the phases, `add-interactive-app` adds the desktop window: an editable
grid over the same engine. It needs only Phase 1 and gains each later phase's
features as they land.

Also outside the numbered phases, `add-polars-dtypes` makes an omasheet type
a name for a Polars dtype, and `add-dataframe-interop` then adds Parquet and
dataframe-backed tables. The first needs Phase 1, the second needs Phase 2
and the first.

Then the owner's decision of 2026-10-23 put the dataframe at the centre, and
three changes on the `polars-core` branch carry it: `align-polars-vocabulary`
takes Polars' vocabulary as OMX's, `add-columnar-core` holds a sheet's data as
Polars columns and lowers to Polars every column it can — the lowering that
`add-dataframe-interop` had carried, which moves here with it — and
`add-sheet-operations-polars` is Phase 3 on top of that. `add-sheet-operations`,
which had Phase 3 on a kernel of omasheet's own, is superseded by the three.
After them comes `hold-less-of-a-big-file`, the snagging change: the three
places a big Parquet is materialised whole where a part of it would do, each
measured by the change that left it.

Dependencies: every phase needs Phase 1. Phase 3 now needs Phase 2, through
the columnar core it is built on. Phase 4 adds deltas to Phases 2 and 3 (units
on export, units through group/join), and Phase 5 builds on Phase 4.

Not yet in any change: matrices and the matrix product, REPL, LSP server, Neovim plugin, TUI viewer, charts,
pivots, formatting syntax.

## Tech Stack

- Language: Rust, shipped as a single static binary `omasheet`
- Parsing: hand-written lexer and recursive-descent parser (no parser library)
- Numerics: the Polars dtypes, held as `i64` and a 38-digit fixed-point
  `i128` with an `i32` scale in `omasheet-omx::decimal`. Aligning with the dtypes does not
  need the Polars crate — `omasheet-omx` has no dependency at all — and
  everything that holds or calculates a column does
- Evaluation: Polars, from `add-columnar-core`. A sheet's data is held as
  Polars columns and a column that can be calculated in one step over the
  whole of them is; the engine keeps what Polars cannot do — a row-recursive
  column, the exact decimal edges, a value with no dtype — and the two are
  held to the same answer by a differential test. Sheet operations (group,
  join, sort) are Polars' too, from Phase 3. Polars was out of scope while
  the exact numbers were rationals (D11), and native sheet operations were
  the reading that followed from that; `add-polars-dtypes` gives the
  rationals up, `add-dataframe-interop` brings Polars in for Parquet and for
  dataframe-backed tables, and the decision of 2026-10-23 brings it into
  evaluation
- XLSX (Phase 2): `calamine` (read), `rust_xlsxwriter` (write)
- CSV (Phase 2): `csv`
- Markdown (Phase 2): `pulldown-cmark`
- CLI: `clap`
- App: Qt 6 Quick (QML) with `cxx-qt`; needs `qt6-base` and `qt6-declarative`
- Later: `tower-lsp` (LSP), Neovim plugin

## Workspace Layout (planned)

```
omasheet/
├── omasheet-omx      lexer, parser, AST, type checker, dependency analysis
├── omasheet-engine   values, numerics, execution (Phase 1); the Polars
│                     columns a sheet is held in (`add-columnar-core`);
│                     sheet operations (3); units (4); uncertainty (5)
├── omasheet-interop  XLSX and CSV import/export (Phase 2)
├── omasheet-md       Markdown: `{{ }}`, `omx` blocks, render (Phase 2)
├── omasheet-cli      the commands of `omasheet`: print, eval, lint, import, export, render
├── omasheet-ui       the `omasheet` binary, and the interactive window it opens: Qt Quick, driven from Rust (cxx-qt)
└── omasheet-lsp      language server (later)
```

## Conventions

- Native file extension: `.omx` — the same letters as OMX, the expression
  language; an `.omx` file is a whole file (sheets, schemas, constants), not a
  single expression
- Fenced code block language tag in Markdown: `omx`
- Row and column positions are 0-based
- Identifiers for sheets and columns are case-sensitive, conventionally
  `PascalCase`

## Glossary

| Term | Meaning |
|------|---------|
| File | An `.omx` file: declarations plus one or more sheets |
| Sheet | Named, ordered collection of rows with a typed column schema |
| Computed column | A column defined once by an OMX expression (`Name := expr`) |
| OMX | Omasheet Expressions — the formula language |
| Cursor (`*`) | The current row while an expression is evaluated in row context |
| Quantity | A numeric magnitude with a unit (Phase 4) |
| Uncertain | A quantity with an attached measurement uncertainty (`±`) (Phase 5) |
