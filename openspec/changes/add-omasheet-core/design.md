# Design: Omasheet core (Phase 1)

## Context

Source: `coinvent-omasheet-format-chatgpt.md`. The conversation explored many
alternatives; this document records where it landed, what is still provisional,
and what does not yet hold together, for the Phase 1 core. Decisions and open
questions that belong to a later phase live in that phase's `design.md`. The
D/P/Q numbers are shared across all five changes, so gaps here are intentional.

## Decisions

### Decided (by the project owner)

| # | Decision | Notes |
|---|----------|-------|
| D1 | Product name **Omasheet**, for the Omarchy platform | Follows Omawrite / Omacalc |
| D2 | Relative row references use the cursor form `Sales[*-1]` | Chosen over `@prev` |
| D3 | OMX is Raku-like: ranges, slices, 2-D arrays | Raku-inspired, not embedded Raku |
| D4 | OMX is a fully custom language compiled to a Rust engine | No embedded Lua/JS/Python/SQL |
| D5 | `;` is the dimension separator inside `[]` | As in Raku: `A[2; 3]` |
| D6 | Numeric design lifted from Raku, centred on rationals | |
| D7 | `Int` is arbitrary precision (BigInt); `Rat` is arbitrary precision (FatRat) | No fixed-width integers at language level |
| D8 | Units and uncertainty are in scope | Later phases — see `add-units`, `add-uncertainty` |
| D9 | Types are expressed over an entire column | Column-typed first, cell-typed second; units per column arrive in `add-units` |
| D10 | XLSX via existing crates | Later phase — see `add-interop`. The dataframe-library half of this decision was withdrawn: see D11 in `add-table-operations` |
| D12 | File extension is `.omx` | Owner decision, 2026-10-07. Replaces the provisional `.sheet`; also seen: `.sd`, `.sheetdown`, `.omasheet` |
| D13 | `*` is always the cursor (current row); the whole dimension is an empty slot: `A[; 3]`, `A[2; ]` | Owner decision, 2026-10-07. Bare `*` is an error with no row context. Unlike Raku, `[*-1]` is the previous row; the last row is `[-1]` |
| D14 | A leading `=` marks a formula cell; every other cell is a literal | Owner decision, 2026-10-07. Replaces the provisional "no leading `=`". Computed columns (`Name := expr`) and `const` take no marker |
| D15 | Fallback operator is `//` (Raku defined-or) | Owner decision, 2026-10-07. Chosen over `??`. `//` therefore cannot mean integer division or begin a comment |

### Provisional (proposed, not objected to, or chosen here for coherence)

| # | Choice | Alternative(s) seen |
|---|--------|---------------------|
| P2 | No A1 references in authored files | Excel-style `=B2-C2` |
| P3 | Equality is `==`; `=` is binding (`const`) and the formula-cell marker (D14), never comparison | Early examples used `Region="UK"` |
| P4 | Bare column name in a predicate or cell means "this row's value" | `.Revenue`, `*.Revenue` |
| P5 | `.Field` and `; Field` are equivalent: `T[r].C` ≡ `T[r; C]` | Only one of the two |
| P7 | Conditional is `if … then … else …` | `c ? a : b` ternary |
| P8 | Aggregations are methods: `.sum() .avg() .min() .max() .count()` | Function form `sum(x)` |
| P9 | Pipe operator `|>` for multi-step table transforms | Method chaining only |
| P10 | Computed column syntax `Name := expr` | Per-cell repeated formulas |
| P15 | First app is a read-only viewer; editing stays in the text editor | Superseded by D16 in `add-interactive-app`: the app is an interactive grid |
| P17 | Lexical choices made while building: `#` comments to end of line; `"…"` strings with `\" \\ \n \t`; `true` / `false`; dates as `2025-01-01` and date-times as `2025-01-01T09:30` or `2025-01-01T09:30:15` (no time zone); a `const` or `:=` expression continues onto following indented lines, or while a bracket is open | Not discussed in the source — was Q13. No `null` literal: an empty cell is the only way to write "empty" |
| P18 | Inside a cell, `|` is not a separator when it is inside a quoted string or is the `|>` operator | Not discussed in the source — was Q14. An escape such as `\|` |
| P19 | A lookup that yields a vector of one value fills a cell with that value; no match gives empty; several matches is an error | Relates to Q10 in `add-table-operations` |

## Architecture

```
 .omx source ──┐
                 ▼
   lexer → parser → AST → semantic analysis (types, shapes)
                                 │
                        typed IR → dependency graph
                                 │
                         execution plan → Rust runtime
                                 │
                      exact scalar/array kernel
```

- File syntax and OMX are separate grammars sharing one AST and one type system.
  The OMX parser does not know whether an expression came from a cell, a
  computed column, `omasheet eval`, or a Markdown `{{ }}`.
- References are resolved at compile time: `Sales[*-1; Revenue]` becomes
  `LoadCell { table_id, row_offset: -1, col_id }`, not a runtime name lookup.
- Storage is column-oriented. MVP may use plain `Vec<T>` per column with
  hand-written slicing; views should not copy.
- Later phases extend this pipeline without replacing it: table operations
  beside the kernel (Phase 2), XLSX and Markdown at the edges (Phase 3), units
  and uncertainty in semantic analysis and the value model (Phases 4 and 5).

## Implementation Notes (as built)

- **Parser.** Hand-written recursive descent rather than `chumsky`: the grammar
  is small, the cursor and empty-slot forms need context-dependent handling, and
  it keeps the dependency list to `num-*` and `clap`. A syntax error stops that
  one expression; other declarations are still checked.
- **Checking is gradual.** Types and shapes are checked statically wherever they
  are known. Where they are not (a column that reads itself, mixed-type columns)
  the type is `Any` and the check happens during evaluation, with the same
  located diagnostic. Vector lengths are compared statically only when both are
  known (literals and whole columns).
- **Cycles.** A cycle is reported only if a cell could reach itself: a column
  that reads its own earlier rows (`[*-1]`, `[0..*-1]`) is not circular.
- **Evaluation.** Cells are calculated once, on demand, and visited in
  dependency order, row by row, so a running balance over 200,000 rows does not
  recurse deeply. A cell that fails shows `#ERROR`; cells that read it also
  show `#ERROR` without a second diagnostic.

Not built in this phase, though the specs mention them:

- **A distinct matrix shape.** `Sales[2..5; 3..7]` returns a table (rows and
  named columns) with those dimensions rather than an unnamed matrix; there is
  no matrix literal.
- **Complex literals.** `Complex` values exist and do arithmetic, built with
  `Complex(re, im)`; there is no literal syntax and no `Complex` column type.
- **Date arithmetic.** Dates and date-times can be compared with their own
  kind, not added or subtracted, and a `Date` does not mix with a `DateTime`.

## Open Questions

**Q8 — Rat growth.** Long chains of exact operations can grow denominators
without bound. Decided: never convert silently. Not decided: whether to warn
(lint) past a size threshold.

**Q12 — Descending ranges and the full operator set.** `10...1` for descending
ranges, `in` / `∈` membership, `%` as modulo vs percent, and `.map/.filter/
.reduce/.sort/.unique` were all listed without detail. Membership (`in`) is
specified because an example depends on it; the rest are deferred.

## Risks

- **Scope.** Exact numerics plus a new language is already a large surface. The
  roadmap in `project.md` keeps units, uncertainty, table operations and
  XLSX out of this phase so that a useful tool (parse, evaluate, print) exists
  first.
- **Ambiguity debt.** The three grammar-shaping questions are settled (D13–D15).
  P17 and P18 were chosen during implementation and still need the owner's
  confirmation; changing them later means changing the lexer and the cell
  splitter, not the engine.
- **Later phases reopen the type system.** Units and uncertainty will extend
  column types and static checking. Keep the type representation open to
  parameters (`Rat<unit>`) even though Phase 1 has none.
